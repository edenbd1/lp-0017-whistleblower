//! RegistryClient that drives the SPEL CLI by shelling out.
//!
//! Why shell-out and not direct linking: linking to `nssa_core` +
//! `spel-framework` pulls in multi-GB Risc0 circuits and a strict LEZ
//! tag pin. Shelling out keeps the binary portable.
//!
//! All real on-chain interaction goes through `spel --idl <FILE> -p
//! <BIN> -- <instruction> ...`. The wallet password (if any) is read
//! from `$WALLET_PASSWORD` and fed via a pty — see [`pty_spawn`].
//!
//! Note on the spel CLI's `Vec<String>` ABI: the `cli-vec-string` fork
//! pinned in `methods/guest/Cargo.toml` parses multiple `--cids` via
//! flag repetition (`--cids a --cids b --cids c`) rather than CSV.
//! We pass cids via repetition; `metadata_hashes` and
//! `anchor_timestamps` use the CSV form (`Vec<[u8;32]>` and
//! `Vec<u32>` are already parsed in CSV upstream).

use async_trait::async_trait;
use indexing::{IndexingError, IndexingResult, RegistryClient};
use registry_core::{CidRecord, Registry};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct ShellOutRegistry {
    pub sequencer_url: String,
    pub program_id_hex: String,
    pub program_bin: PathBuf,
    pub idl_path: PathBuf,
    pub signer_account_id: String,
    pub lgs_bin: String,
    pub wallet_password: String,
    pub spel_bin: String,
    pub wallet_bin: String,
}

impl ShellOutRegistry {
    pub fn from_config(cfg: &crate::config::RegistrySection) -> Self {
        let idl_path = PathBuf::from(&cfg.idl_path);
        // Convention: the SPEL guest binary sits next to the IDL we
        // committed. If the config doesn't override, fall back to the
        // canonical methods/guest target path.
        let program_bin = std::env::var("LP0017_GUEST_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(
                    "methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin",
                )
            });
        Self {
            sequencer_url: cfg.sequencer_url.clone(),
            program_id_hex: cfg.program_id.clone(),
            program_bin,
            idl_path,
            signer_account_id: cfg.signer_account_id.clone(),
            lgs_bin: cfg.lgs_bin.clone().unwrap_or_else(|| "lgs".into()),
            wallet_password: std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| "test".into()),
            spel_bin: "spel".into(),
            wallet_bin: "wallet".into(),
        }
    }

    /// Lazy registry PDA address: relies on `spel --idl <FILE> -p <BIN>
    /// pda registry` to compute it. Falls back to an empty string when
    /// the spel binary isn't on PATH (host-only test runs).
    ///
    /// The output format on the pinned spel fork is a bare line with
    /// the base58 PDA — no prefix. Older versions print
    /// `registry → <PDA>`. Accept both.
    pub fn pda_address(&self) -> IndexingResult<String> {
        let out = std::process::Command::new(&self.spel_bin)
            .arg("--idl")
            .arg(&self.idl_path)
            .arg("-p")
            .arg(&self.program_bin)
            .arg("--")
            .arg("pda")
            .arg("registry")
            .output();
        let stdout = match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            _ => return Ok(String::new()),
        };
        for line in stdout.lines() {
            let line = line.trim();
            if let Some(rest) = line.split("registry →").nth(1) {
                let cand = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                    .to_string();
                if !cand.is_empty() {
                    return Ok(cand);
                }
            }
            // Bare base58 line — current spel fork output format.
            if is_base58_likely(line) {
                return Ok(line.to_string());
            }
        }
        Ok(String::new())
    }

    /// Common subroutine: spawn an interactive process that may prompt
    /// for the wallet password, feed it in via stdin, and collect the
    /// output. The wallet/spel binaries actually read the password
    /// from a pty in normal operation, but in practice the standalone
    /// LEZ debug wallet at v0.2.0-rc3 does not re-prompt for subsequent
    /// commands once the storage is unlocked — so plain stdin works.
    async fn run_with_password(
        &self,
        bin: &str,
        args: &[String],
    ) -> IndexingResult<(bool, String, String)> {
        let mut cmd = Command::new(bin);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NSSA_WALLET_HOME_DIR", self.wallet_home())
            .env("RISC0_DEV_MODE", "0");
        let mut child = cmd
            .spawn()
            .map_err(|e| IndexingError::Transport(format!("spawn {bin}: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            // Feed the password just in case; harmless if not prompted.
            let _ = stdin
                .write_all(format!("{}\n", self.wallet_password).as_bytes())
                .await;
            drop(stdin);
        }
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let (out, err, status) =
            tokio::join!(collect_lines(stdout), collect_lines(stderr), child.wait(),);
        let status = status.map_err(|e| IndexingError::Transport(e.to_string()))?;
        Ok((status.success(), out, err))
    }

    fn wallet_home(&self) -> String {
        std::env::var("NSSA_WALLET_HOME_DIR").unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| format!("{h}/logos/src/logos-execution-zone/wallet/configs/debug"))
                .unwrap_or_default()
        })
    }
}

async fn collect_lines<R: tokio::io::AsyncRead + Unpin>(mut r: R) -> String {
    let mut buf = String::new();
    let _ = r.read_to_string(&mut buf).await;
    buf
}

/// Heuristic: a base58 string of LEZ-account length (typically 43–44
/// chars). Used to identify the bare-line PDA output from spel.
fn is_base58_likely(s: &str) -> bool {
    if s.len() < 32 || s.len() > 64 {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() && c != '0' && c != 'O' && c != 'I' && c != 'l')
}

#[async_trait]
impl RegistryClient for ShellOutRegistry {
    async fn is_initialised(&self) -> IndexingResult<bool> {
        let pda = self.pda_address()?;
        if pda.is_empty() {
            return Ok(false);
        }
        let out = Command::new(&self.wallet_bin)
            .args(["account", "get", "--account-id", &format!("Public/{pda}")])
            .env("NSSA_WALLET_HOME_DIR", self.wallet_home())
            .output()
            .await
            .map_err(|e| IndexingError::Transport(e.to_string()))?;
        if !out.status.success() {
            return Ok(false);
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Heuristic: the JSON line has a non-empty `data` field once
        // the registry is initialised, even if there are zero entries.
        Ok(stdout.contains("\"data\":\"") && !stdout.contains("\"data\":\"\""))
    }

    async fn init(&self) -> IndexingResult<String> {
        let args = vec![
            "--idl".into(),
            self.idl_path.to_string_lossy().into_owned(),
            "-p".into(),
            self.program_bin.to_string_lossy().into_owned(),
            "--".into(),
            "init-registry".into(),
            "--payer".into(),
            self.signer_account_id.clone(),
        ];
        let (ok, out, err) = self.run_with_password(&self.spel_bin, &args).await?;
        if !ok {
            return Err(IndexingError::Backend(format!(
                "spel init-registry failed:\nstdout:\n{out}\nstderr:\n{err}"
            )));
        }
        Ok(extract_tx_hash(&out).unwrap_or_else(|| "tx-hash-unknown".into()))
    }

    async fn index_batch(
        &self,
        cids: &[String],
        metadata_hashes: &[[u8; 32]],
        timestamps: &[u32],
    ) -> IndexingResult<String> {
        registry_core::validate_batch(cids, metadata_hashes, timestamps)
            .map_err(|e| IndexingError::RegistryCode(e.code()))?;
        let mut args: Vec<String> = vec![
            "--idl".into(),
            self.idl_path.to_string_lossy().into_owned(),
            "-p".into(),
            self.program_bin.to_string_lossy().into_owned(),
            "--".into(),
            "index-batch".into(),
        ];
        // Flag repetition is the Vec<String> parsing form on the pinned spel
        // fork (commit fbbffd3 — see methods/guest/Cargo.toml + BUGS_FILED.md).
        for cid in cids {
            args.push("--cids".into());
            args.push(cid.clone());
        }
        args.push("--metadata-hashes".into());
        args.push(
            metadata_hashes
                .iter()
                .map(hex::encode)
                .collect::<Vec<_>>()
                .join(","),
        );
        args.push("--anchor-timestamps".into());
        args.push(
            timestamps
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        args.push("--anchorer".into());
        args.push(self.signer_account_id.clone());

        let (ok, out, err) = self.run_with_password(&self.spel_bin, &args).await?;
        if !ok {
            return Err(IndexingError::Backend(format!(
                "spel index-batch failed:\nstdout:\n{out}\nstderr:\n{err}"
            )));
        }
        Ok(extract_tx_hash(&out).unwrap_or_else(|| "tx-hash-unknown".into()))
    }

    async fn lookup(&self, cid: &str) -> IndexingResult<Option<CidRecord>> {
        let registry = self.fetch_registry().await?;
        Ok(registry.entries.get(cid).cloned())
    }

    async fn anchored_cid_set(&self) -> IndexingResult<HashSet<String>> {
        let registry = self.fetch_registry().await?;
        Ok(registry.entries.keys().cloned().collect())
    }
}

impl ShellOutRegistry {
    async fn fetch_registry(&self) -> IndexingResult<Registry> {
        let pda = self.pda_address()?;
        if pda.is_empty() {
            return Ok(Registry::default());
        }
        // `wallet account get` prints a header line "Account" followed
        // by a JSON line. We scan for the JSON line specifically.
        let out = Command::new(&self.wallet_bin)
            .args(["account", "get", "--account-id", &format!("Public/{pda}")])
            .env("NSSA_WALLET_HOME_DIR", self.wallet_home())
            .output()
            .await
            .map_err(|e| IndexingError::Transport(e.to_string()))?;
        if !out.status.success() {
            return Ok(Registry::default());
        }
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let Some(json_line) = stdout.lines().find(|l| l.trim_start().starts_with('{')) else {
            return Ok(Registry::default());
        };
        let parsed: serde_json::Value = serde_json::from_str(json_line.trim())
            .map_err(|e| IndexingError::Unexpected(format!("account JSON: {e}")))?;
        let Some(hex_str) = parsed.get("data").and_then(|v| v.as_str()) else {
            return Ok(Registry::default());
        };
        if hex_str.is_empty() {
            return Ok(Registry::default());
        }
        let bytes = hex::decode(hex_str)
            .map_err(|e| IndexingError::Unexpected(format!("hex decode: {e}")))?;
        let registry: Registry = borsh::from_slice(&bytes)
            .map_err(|e| IndexingError::Unexpected(format!("borsh decode registry: {e}")))?;
        Ok(registry)
    }
}

/// Extract the `tx_hash:` field from spel/wallet stdout.
pub(crate) fn extract_tx_hash(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        for prefix in ["tx_hash:", "tx_hash =", "TxHash:", "txHash:"] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let cand = rest.trim().trim_matches(|c| matches!(c, '"' | ',' | ';'));
                if !cand.is_empty() {
                    return Some(cand.to_string());
                }
            }
        }
    }
    None
}

/// Legacy alias for the old parser name; kept until callers migrate.
#[allow(dead_code)]
pub(crate) fn parse_tx_hash(stdout: &str) -> Option<String> {
    extract_tx_hash(stdout)
}

/// Extract raw hex from `wallet account get --raw` style output. Kept
/// for the unit-test surface; the live path reads JSON via `fetch_registry`.
#[allow(dead_code)]
pub(crate) fn parse_raw_account_hex(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        for prefix in ["data:", "Account data:", "Data:"] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let cand = rest.trim().trim_matches('"').trim_start_matches("0x");
                if !cand.is_empty() && cand.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(cand.to_string());
                }
            }
        }
        let cand = trimmed.trim_start_matches("0x");
        if cand.len() > 8 && cand.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(cand.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tx_hash_picks_up_clean_line() {
        let out = "submitted\ntx_hash: 0xdeadbeefcafe1234\nok";
        assert_eq!(extract_tx_hash(out).as_deref(), Some("0xdeadbeefcafe1234"));
    }

    #[test]
    fn extract_tx_hash_strips_punctuation() {
        let out = r#"TxHash: "0xabc1234","#;
        assert_eq!(extract_tx_hash(out).as_deref(), Some("0xabc1234"));
    }

    #[test]
    fn extract_tx_hash_none_when_missing() {
        assert_eq!(extract_tx_hash("nothing here\n"), None);
    }

    #[test]
    fn parse_raw_account_hex_picks_prefixed_line() {
        let out = "Account data: 0xdeadbeef\n";
        assert_eq!(parse_raw_account_hex(out).as_deref(), Some("deadbeef"));
    }

    #[test]
    fn parse_raw_account_hex_bare_line() {
        let out = "header\n0102deadbeef\nfooter\n";
        assert_eq!(parse_raw_account_hex(out).as_deref(), Some("0102deadbeef"));
    }

    #[test]
    fn parse_raw_account_hex_none() {
        assert_eq!(parse_raw_account_hex("no hex\n"), None);
    }

    #[tokio::test]
    async fn index_batch_validates_input_before_calling_out() {
        let r = ShellOutRegistry {
            sequencer_url: "http://localhost:3040".into(),
            program_id_hex: "".into(),
            program_bin: PathBuf::from("/dev/null"),
            idl_path: PathBuf::from("/dev/null"),
            signer_account_id: "test".into(),
            lgs_bin: "lgs".into(),
            wallet_password: "test".into(),
            spel_bin: "spel".into(),
            wallet_bin: "wallet".into(),
        };
        let err = r
            .index_batch(&["cid".into()], &[], &[1u32])
            .await
            .unwrap_err();
        match err {
            IndexingError::RegistryCode(code) => {
                assert_eq!(code, registry_core::RegistryError::ArityMismatch.code());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
