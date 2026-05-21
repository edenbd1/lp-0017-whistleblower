//! `batch-anchor init` — claim the registry PDA on-chain.

use crate::config::Config;
use crate::registry::ShellOutRegistry;
use indexing::RegistryClient;

pub async fn run(cfg: &Config) -> anyhow::Result<()> {
    let registry = ShellOutRegistry::from_config(&cfg.registry);
    if registry.is_initialised().await? {
        tracing::info!("registry PDA already initialised — nothing to do");
        return Ok(());
    }
    let tx = registry.init().await?;
    tracing::info!(tx_hash = %tx, "registry initialised");
    println!("{tx}");
    Ok(())
}
