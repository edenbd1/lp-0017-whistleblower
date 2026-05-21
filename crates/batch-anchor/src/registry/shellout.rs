//! RegistryClient that drives `lgs` + the SPEL CLI by shelling out.
//!
//! Rationale: the alternative (linking to `nssa_core` + `spel-framework`
//! directly) pulls in multi-GB Risc0 circuits and a strict LEZ tag pin.
//! Shelling out is portable, hermetic, and matches Thompson's pattern
//! at `/tmp/lp17-thompson/batch-anchor/src/registry/state.rs`.
//!
//! Pure-parsing helpers are unit-tested; the actual `Command::exec`
//! path is exercised by the `live-lez` integration tests.

use async_trait::async_trait;
use indexing::{IndexingError, IndexingResult, RegistryClient};
use registry_core::{CidRecord, Registry};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ShellOutRegistry {
    pub sequencer_url: String,
    pub program_id_hex: String,
    pub idl_path: PathBuf,
    pub signer_account_id: String,
    pub lgs_bin: String,
}

impl ShellOutRegistry {
    pub fn from_config(cfg: &crate::config::RegistrySection) -> Self {
        Self {
            sequencer_url: cfg.sequencer_url.clone(),
            program_id_hex: cfg.program_id.clone(),
            idl_path: cfg.idl_path.clone().into(),
            signer_account_id: cfg.signer_account_id.clone(),
            lgs_bin: cfg.lgs_bin.clone().unwrap_or_else(|| "lgs".into()),
        }
    }

    /// Derive the registry PDA address for the configured `program_id`.
    /// SPEL seed = `literal("registry")` (see methods/guest).
    pub fn pda_address(&self) -> IndexingResult<String> {
        // PDA address depends on the program_id and seed. The CLI
        // returns it via `lgs program pda --program-id <hex> --seed registry`;
        // until we have access to that subcommand we stash the derived
        // address in config or rely on `lgs wallet account get`
        // returning the canonical form.
        //
        // For tests: deterministic placeholder.
        if self.program_id_hex.is_empty() {
            return Err(IndexingError::Backend(
                "program_id_hex is empty; deploy first".into(),
            ));
        }
        Ok(format!("Public/registry@{}", &self.program_id_hex[..8]))
    }
}

#[async_trait]
impl RegistryClient for ShellOutRegistry {
    async fn is_initialised(&self) -> IndexingResult<bool> {
        // Conservative: returns false until a live `lgs wallet account
        // get --raw <pda>` returns non-empty data. Wired up in the
        // `live-lez` feature gate.
        #[cfg(feature = "live-lez")]
        {
            let pda = self.pda_address()?;
            let out = tokio::process::Command::new(&self.lgs_bin)
                .args(["wallet", "account", "get", "--raw", &pda])
                .output()
                .await
                .map_err(|e| IndexingError::Transport(e.to_string()))?;
            return Ok(out.status.success() && !out.stdout.is_empty());
        }
        #[allow(unreachable_code)]
        Ok(false)
    }

    async fn init(&self) -> IndexingResult<String> {
        #[cfg(feature = "live-lez")]
        {
            let out = tokio::process::Command::new("spel")
                .args([
                    "init_registry",
                    "--payer",
                    &self.signer_account_id,
                    "-i",
                    self.idl_path.to_str().unwrap_or(""),
                    "-p",
                    &self.program_id_hex,
                ])
                .env("SEQUENCER_URL", &self.sequencer_url)
                .output()
                .await
                .map_err(|e| IndexingError::Transport(e.to_string()))?;
            if !out.status.success() {
                return Err(IndexingError::Backend(format!(
                    "spel init_registry: {}",
                    String::from_utf8_lossy(&out.stderr)
                )));
            }
            return Ok(parse_tx_hash(&String::from_utf8_lossy(&out.stdout))
                .unwrap_or_else(|| "tx-hash-unknown".into()));
        }
        #[allow(unreachable_code)]
        Err(IndexingError::Unexpected(
            "live-lez feature not enabled; rebuild with --features live-lez".into(),
        ))
    }

    async fn index_batch(
        &self,
        cids: &[String],
        metadata_hashes: &[[u8; 32]],
        timestamps: &[u32],
    ) -> IndexingResult<String> {
        registry_core::validate_batch(cids, metadata_hashes, timestamps)
            .map_err(|e| IndexingError::RegistryCode(e.code()))?;
        #[cfg(feature = "live-lez")]
        {
            let cids_csv = cids.join(",");
            let hashes_csv = metadata_hashes
                .iter()
                .map(hex::encode)
                .collect::<Vec<_>>()
                .join(",");
            let ts_csv = timestamps
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let out = tokio::process::Command::new("spel")
                .args([
                    "index_batch",
                    "--cids",
                    &cids_csv,
                    "--metadata-hashes",
                    &hashes_csv,
                    "--anchor-timestamps",
                    &ts_csv,
                    "--payer",
                    &self.signer_account_id,
                    "-i",
                    self.idl_path.to_str().unwrap_or(""),
                    "-p",
                    &self.program_id_hex,
                ])
                .env("SEQUENCER_URL", &self.sequencer_url)
                .output()
                .await
                .map_err(|e| IndexingError::Transport(e.to_string()))?;
            if !out.status.success() {
                return Err(IndexingError::Backend(format!(
                    "spel index_batch: {}",
                    String::from_utf8_lossy(&out.stderr)
                )));
            }
            return Ok(parse_tx_hash(&String::from_utf8_lossy(&out.stdout))
                .unwrap_or_else(|| "tx-hash-unknown".into()));
        }
        #[allow(unreachable_code)]
        Err(IndexingError::Unexpected(
            "live-lez feature not enabled; rebuild with --features live-lez".into(),
        ))
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
        #[cfg(feature = "live-lez")]
        {
            let pda = self.pda_address()?;
            let out = tokio::process::Command::new(&self.lgs_bin)
                .args(["wallet", "account", "get", "--raw", &pda])
                .output()
                .await
                .map_err(|e| IndexingError::Transport(e.to_string()))?;
            if !out.status.success() {
                return Ok(Registry::default());
            }
            let hex_bytes = parse_raw_account_hex(&String::from_utf8_lossy(&out.stdout))
                .ok_or_else(|| {
                    IndexingError::Unexpected(
                        "could not extract account data hex from lgs output".into(),
                    )
                })?;
            let bytes = hex::decode(hex_bytes)
                .map_err(|e| IndexingError::Unexpected(format!("hex decode: {e}")))?;
            let registry: Registry = borsh::from_slice(&bytes)
                .map_err(|e| IndexingError::Unexpected(format!("borsh decode: {e}")))?;
            return Ok(registry);
        }
        #[allow(unreachable_code)]
        Ok(Registry::default())
    }
}

/// Extract the `tx_hash:` field from `spel`/`lgs` stdout. Returns None
/// if the line cannot be located — caller falls back to a placeholder.
#[allow(dead_code)]
pub(crate) fn parse_tx_hash(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let line = line.trim();
        for prefix in ["tx_hash:", "tx_hash =", "TxHash:", "txHash:"] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let candidate = rest.trim().trim_matches('"');
                if !candidate.is_empty() {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}

/// Extract the account-data hex string from `lgs wallet account get
/// --raw <pda>` stdout. Tolerates multiple emission shapes
/// (`data: <hex>`, `Account data: 0x<hex>`, JSON-ish single hex line).
#[allow(dead_code)]
pub(crate) fn parse_raw_account_hex(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let line = line.trim();
        for prefix in ["data:", "Account data:", "Data:"] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let candidate = rest.trim().trim_matches('"').trim_start_matches("0x");
                if candidate.chars().all(|c| c.is_ascii_hexdigit())
                    && !candidate.is_empty()
                {
                    return Some(candidate.to_string());
                }
            }
        }
        // Last-ditch: a single all-hex token on its own line.
        let candidate = line.trim_start_matches("0x");
        if candidate.len() > 8
            && candidate.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Some(candidate.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tx_hash_extracts_value() {
        let out = "ok\ntx_hash: 0xdeadbeef\nother line\n";
        assert_eq!(parse_tx_hash(out).as_deref(), Some("0xdeadbeef"));
    }

    #[test]
    fn parse_tx_hash_strips_quotes() {
        let out = "TxHash: \"0xabc\"";
        assert_eq!(parse_tx_hash(out).as_deref(), Some("0xabc"));
    }

    #[test]
    fn parse_tx_hash_returns_none_when_missing() {
        let out = "submitted instruction; pending confirmation\n";
        assert_eq!(parse_tx_hash(out), None);
    }

    #[test]
    fn parse_raw_account_hex_picks_up_prefixed_line() {
        let out = "Account data: 0xdeadbeefcafe1234\nother: stuff\n";
        assert_eq!(
            parse_raw_account_hex(out).as_deref(),
            Some("deadbeefcafe1234")
        );
    }

    #[test]
    fn parse_raw_account_hex_handles_bare_hex_line() {
        let out = "header\n01020304deadbeef\nfooter\n";
        assert_eq!(
            parse_raw_account_hex(out).as_deref(),
            Some("01020304deadbeef")
        );
    }

    #[test]
    fn parse_raw_account_hex_returns_none_when_no_hex() {
        let out = "nothing relevant here\nor here\n";
        assert_eq!(parse_raw_account_hex(out), None);
    }

    #[tokio::test]
    async fn index_batch_validates_input_before_calling_out() {
        let r = ShellOutRegistry {
            sequencer_url: "http://localhost:3040".into(),
            program_id_hex: "".into(),
            idl_path: "/dev/null".into(),
            signer_account_id: "test".into(),
            lgs_bin: "lgs".into(),
        };
        // Arity mismatch — should never reach the shell-out path.
        let err = r
            .index_batch(
                &["cid".into()],
                &[],
                &[1_u32],
            )
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
