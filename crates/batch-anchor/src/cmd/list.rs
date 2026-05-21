//! `batch-anchor list` — list all anchored CIDs.

use crate::config::Config;
use crate::registry::ShellOutRegistry;
use indexing::RegistryClient;

pub async fn run(cfg: &Config) -> anyhow::Result<()> {
    let registry = ShellOutRegistry::from_config(&cfg.registry);
    let mut cids: Vec<String> = registry.anchored_cid_set().await?.into_iter().collect();
    cids.sort();
    for cid in &cids {
        println!("{cid}");
    }
    tracing::info!(count = cids.len(), "listed anchored CIDs");
    Ok(())
}
