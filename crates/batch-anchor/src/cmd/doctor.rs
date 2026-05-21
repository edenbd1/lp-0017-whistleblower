//! `batch-anchor doctor` — probe storage, delivery, and registry.

use crate::config::Config;
use crate::delivery::NwakuRest;
use crate::registry::ShellOutRegistry;
use crate::storage::CodexRest;
use indexing::{DeliveryClient, RegistryClient, StorageClient};

pub async fn run(cfg: &Config) -> anyhow::Result<()> {
    let storage = CodexRest::new(cfg.storage.url.clone());
    let delivery = NwakuRest::new(cfg.delivery.url.clone());
    let registry = ShellOutRegistry::from_config(&cfg.registry);

    let storage_ok = storage.healthy().await;
    let delivery_ok = delivery.healthy().await;
    let registry_ok = registry.is_initialised().await.unwrap_or(false);

    println!("storage   ({}): {}", cfg.storage.url, status(storage_ok));
    println!("delivery  ({}): {}", cfg.delivery.url, status(delivery_ok));
    println!(
        "registry  ({}): {}",
        cfg.registry.sequencer_url,
        status(registry_ok)
    );

    if storage_ok && delivery_ok && registry_ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn status(ok: bool) -> &'static str {
    if ok {
        "OK"
    } else {
        "DOWN"
    }
}
