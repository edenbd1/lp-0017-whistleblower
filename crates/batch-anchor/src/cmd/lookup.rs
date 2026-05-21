//! `batch-anchor lookup <cid>` — read a single record from the registry.

use crate::cli::LookupArgs;
use crate::config::Config;
use crate::registry::ShellOutRegistry;
use indexing::RegistryClient;
use serde::Serialize;

#[derive(Serialize)]
struct LookupOutput<'a> {
    cid: &'a str,
    metadata_hash: String,
    anchor_timestamp: i64,
    anchored_by: String,
    version: u8,
}

pub async fn run(cfg: &Config, args: &LookupArgs) -> anyhow::Result<()> {
    let registry = ShellOutRegistry::from_config(&cfg.registry);
    match registry.lookup(&args.cid).await? {
        Some(rec) => {
            let out = LookupOutput {
                cid: &args.cid,
                metadata_hash: format!("v1:{}", hex::encode(rec.metadata_hash)),
                anchor_timestamp: rec.anchor_timestamp,
                anchored_by: hex::encode(rec.anchored_by),
                version: rec.version,
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            Ok(())
        }
        None => {
            tracing::warn!(cid = %args.cid, "not anchored");
            std::process::exit(1);
        }
    }
}
