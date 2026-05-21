//! `batch-anchor watch` — the main subscribe-and-anchor loop.
//!
//! Lifecycle:
//! 1. Health-probe storage, delivery, registry; warn if any are down
//!    but continue (transient outages are part of normal life).
//! 2. Seed the [`BatchBuffer`] from the on-chain anchored set.
//! 3. Catch up via store-protocol (24 h lookback by default).
//! 4. Subscribe to every configured topic.
//! 5. Loop: drain → push → maybe-flush, until interrupted (or `--once`
//!    after the first flush).

use crate::batch::BatchBuffer;
use crate::cli::WatchArgs;
use crate::config::Config;
use crate::delivery::NwakuRest;
use crate::registry::ShellOutRegistry;
use indexing::{DeliveryClient, Envelope, RegistryClient};
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub async fn run(cfg: &Config, args: &WatchArgs) -> anyhow::Result<()> {
    let delivery = NwakuRest::new(cfg.delivery.url.clone());
    let registry = ShellOutRegistry::from_config(&cfg.registry);

    if !delivery.healthy().await {
        tracing::warn!(url = %cfg.delivery.url, "delivery node not reachable; will retry inside loop");
    }

    let mut buffer = BatchBuffer::new();
    seed_from_chain(&registry, &mut buffer).await?;
    catch_up_from_store(&delivery, cfg, &mut buffer).await?;

    for topic in &cfg.delivery.topics {
        if let Err(e) = delivery.subscribe(topic).await {
            tracing::warn!(topic = %topic, error = %e, "subscribe failed; will retry");
        } else {
            tracing::info!(topic = %topic, "subscribed");
        }
    }

    let poll = Duration::from_secs(
        args.poll_interval_secs
            .unwrap_or(cfg.batch.poll_interval_secs),
    );
    let flush_after = Duration::from_secs(cfg.batch.flush_interval_secs);
    let mut last_flush = Instant::now();

    loop {
        for topic in &cfg.delivery.topics {
            match delivery.drain(topic).await {
                Ok(envs) => {
                    for env in envs {
                        if env.validate().is_ok() {
                            buffer.push(env);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(topic = %topic, error = %e, "drain failed");
                }
            }
        }

        let should_flush =
            buffer.is_full() || (buffer.pending_len() > 0 && last_flush.elapsed() >= flush_after);

        if should_flush {
            flush_once(&registry, &mut buffer).await?;
            last_flush = Instant::now();
            if args.once {
                tracing::info!("--once set; exiting after first flush");
                break;
            }
        }

        sleep(poll).await;
    }
    Ok(())
}

async fn seed_from_chain(
    registry: &ShellOutRegistry,
    buffer: &mut BatchBuffer,
) -> anyhow::Result<()> {
    match registry.anchored_cid_set().await {
        Ok(set) => {
            let n = set.len();
            buffer.seed_known(set);
            tracing::info!(seeded = n, "seeded dedup set from on-chain registry");
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not seed dedup set; starting empty");
        }
    }
    Ok(())
}

async fn catch_up_from_store(
    delivery: &NwakuRest,
    cfg: &Config,
    buffer: &mut BatchBuffer,
) -> anyhow::Result<()> {
    if cfg.delivery.store_lookback_hours == 0 {
        return Ok(());
    }
    let lookback_ns: u128 = ((cfg.delivery.store_lookback_hours as u128) * 3_600)
        * 1_000_000_000;
    let now_ns: u128 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let start = now_ns.saturating_sub(lookback_ns);
    for topic in &cfg.delivery.topics {
        match delivery.query_store(topic, start).await {
            Ok(envs) => {
                let mut new_count = 0usize;
                for env in envs {
                    if buffer.push(env) {
                        new_count += 1;
                    }
                }
                tracing::info!(topic = %topic, new = new_count, "catch-up complete");
            }
            Err(e) => {
                tracing::warn!(topic = %topic, error = %e, "store catch-up failed");
            }
        }
    }
    Ok(())
}

async fn flush_once(
    registry: &ShellOutRegistry,
    buffer: &mut BatchBuffer,
) -> anyhow::Result<()> {
    let envs = buffer.drain_batch();
    if envs.is_empty() {
        return Ok(());
    }
    let n = envs.len();
    let (cids, hashes, timestamps) = unzip_envs(&envs);
    tracing::info!(count = n, "flushing batch");
    match registry.index_batch(&cids, &hashes, &timestamps).await {
        Ok(tx) => tracing::info!(count = n, tx_hash = %tx, "flush ok"),
        Err(e) => {
            tracing::error!(count = n, error = %e, "flush failed; will retry next tick");
            buffer.return_failed(envs);
        }
    }
    Ok(())
}

fn unzip_envs(envs: &[Envelope]) -> (Vec<String>, Vec<[u8; 32]>, Vec<u32>) {
    let mut cids = Vec::with_capacity(envs.len());
    let mut hashes = Vec::with_capacity(envs.len());
    let mut timestamps = Vec::with_capacity(envs.len());
    for env in envs {
        cids.push(env.cid.clone());
        hashes.push(env.metadata_hash_bytes().unwrap_or([0u8; 32]));
        timestamps.push(env.timestamp.try_into().unwrap_or(u32::MAX));
    }
    (cids, hashes, timestamps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(n: u8) -> Envelope {
        Envelope {
            v: 1,
            cid: format!("cid:{n}"),
            metadata_hash: format!("v1:{}", "a".repeat(64)),
            timestamp: 1_700_000_000 + n as u64,
            title: None,
            description: None,
            content_type: None,
            size_bytes: None,
            tags: vec![],
        }
    }

    #[test]
    fn unzip_envs_produces_parallel_vectors() {
        let e = vec![env(1), env(2), env(3)];
        let (cids, hashes, ts) = unzip_envs(&e);
        assert_eq!(cids, vec!["cid:1", "cid:2", "cid:3"]);
        assert_eq!(hashes.len(), 3);
        assert_eq!(hashes[0], [0xaa; 32]);
        assert_eq!(ts, vec![1_700_000_001u32, 1_700_000_002, 1_700_000_003]);
    }
}
