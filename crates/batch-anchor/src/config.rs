//! TOML + env configuration.
//!
//! The CLI accepts `--config <path>` (defaults to `./batch-anchor.toml`).
//! Every field has a sensible default targetting a local `lgs localnet`
//! plus the `docker-compose.yml` shipped with this repo, so a fresh
//! clone that follows the README runs without editing config.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level config. Sections are independent so a CLI subcommand can
/// load only what it needs (e.g. `lookup` doesn't touch delivery).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub delivery: DeliverySection,
    #[serde(default)]
    pub storage: StorageSection,
    #[serde(default)]
    pub registry: RegistrySection,
    #[serde(default)]
    pub batch: BatchSection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliverySection {
    /// nwaku REST endpoint (no trailing slash).
    pub url: String,
    /// Topics to subscribe to. Multiple = interop with chronicle.
    pub topics: Vec<String>,
    /// Hours of store-protocol lookback at startup (resume window).
    pub store_lookback_hours: u32,
}

impl Default for DeliverySection {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8645".into(),
            topics: vec!["/whistleblower/1/document-broadcast/json".into()],
            store_lookback_hours: 24,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageSection {
    /// Codex / logos-storage REST endpoint.
    pub url: String,
}

impl Default for StorageSection {
    fn default() -> Self {
        Self {
            // 18080, not 8080: the shipped docker-compose remaps the
            // storage REST port so we don't collide with the
            // ubiquitous :8080 dev port. See infra/docker-compose.yml.
            url: "http://127.0.0.1:18080".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistrySection {
    /// LEZ sequencer endpoint.
    pub sequencer_url: String,
    /// Hex program_id (no 0x prefix, 64 chars).
    pub program_id: String,
    /// Path to the SPEL IDL JSON shipped with the repo.
    pub idl_path: String,
    /// Wallet account ID (base58 or hex) signing on-chain submissions.
    pub signer_account_id: String,
    /// Optional path override for `lgs` / `wallet` binaries on PATH.
    #[serde(default)]
    pub lgs_bin: Option<String>,
}

impl Default for RegistrySection {
    fn default() -> Self {
        Self {
            sequencer_url: "http://127.0.0.1:3040".into(),
            program_id: "".into(),
            idl_path: "./idl/whistleblower_registry.json".into(),
            signer_account_id: "".into(),
            lgs_bin: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchSection {
    /// Flush when this many CIDs accumulate.
    pub flush_size: usize,
    /// Or when this many seconds have elapsed since the last flush.
    pub flush_interval_secs: u64,
    /// Watch-loop poll interval (drain nwaku every N seconds).
    pub poll_interval_secs: u64,
}

impl Default for BatchSection {
    fn default() -> Self {
        Self {
            flush_size: registry_core::MAX_BATCH,
            flush_interval_secs: 30,
            poll_interval_secs: 2,
        }
    }
}

impl Config {
    /// Load from a TOML file. Missing file = use defaults.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        if let Some(p) = path {
            if p.exists() {
                let s = std::fs::read_to_string(p)?;
                let cfg: Config = toml::from_str(&s)?;
                return Ok(cfg);
            }
        }
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_populate_sensible_endpoints() {
        let c = Config::default();
        assert_eq!(c.delivery.url, "http://127.0.0.1:8645");
        assert_eq!(c.storage.url, "http://127.0.0.1:18080");
        assert_eq!(c.registry.sequencer_url, "http://127.0.0.1:3040");
        assert_eq!(c.batch.flush_size, registry_core::MAX_BATCH);
        assert_eq!(c.delivery.topics.len(), 1);
        assert!(c.delivery.topics[0].starts_with("/whistleblower/"));
    }

    #[test]
    fn missing_file_returns_defaults() {
        let cfg = Config::load(Some(Path::new("/nonexistent/batch-anchor.toml"))).unwrap();
        assert_eq!(cfg.delivery.url, "http://127.0.0.1:8645");
    }

    #[test]
    fn partial_toml_merges_with_defaults() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            r#"
[delivery]
url = "http://nwaku.example:9999"
topics = ["/custom/1/json"]
store_lookback_hours = 6
"#,
        )
        .unwrap();
        let cfg = Config::load(Some(tmp.path())).unwrap();
        assert_eq!(cfg.delivery.url, "http://nwaku.example:9999");
        assert_eq!(cfg.delivery.topics, vec!["/custom/1/json".to_string()]);
        // Other sections still come from defaults:
        assert_eq!(cfg.storage.url, "http://127.0.0.1:18080");
    }
}
