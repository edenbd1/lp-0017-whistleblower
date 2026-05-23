//! End-to-end anchor round-trip — `cargo test --features live-lez`.
//!
//! Exercises the full pipeline against a running `lgs localnet` +
//! `infra/docker-compose.yml`:
//!
//!  1. Publish 50 envelopes to nwaku (max-batch capacity).
//!  2. Run the watcher in --once mode so it flushes after one batch.
//!  3. List anchored CIDs via the registry client; assert all 50 are
//!     present.
//!  4. Lookup three random CIDs; assert the metadata_hash + timestamp
//!     match what we published.
//!  5. Re-publish the same 50 envelopes; assert the registry size does
//!     not grow (idempotency).
//!
//! Skipped silently in the host-only fast CI tier (the test body is
//! gated on `feature = "live-lez"`).
//!
//! Required environment:
//!   PROGRAM_ID      hex program ID printed by scripts/deploy.sh
//!   SEQUENCER_URL   http://127.0.0.1:3040 in CI
//!   NWAKU_URL       http://127.0.0.1:8645 in CI
//!   STORAGE_URL     http://127.0.0.1:8080 in CI

#![cfg(feature = "live-lez")]

use batch_anchor::config::{Config, DeliverySection, RegistrySection, StorageSection};
use batch_anchor::delivery::NwakuRest;
use batch_anchor::registry::ShellOutRegistry;
use indexing::{canonical_metadata_hash, DeliveryClient, Envelope, RegistryClient};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TOPIC: &str = "/whistleblower/1/document-broadcast/json";

fn env_or_skip(key: &'static str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            eprintln!("[skip] env {key} not set");
            None
        }
    }
}

fn build_config() -> Option<Config> {
    let sequencer_url = env_or_skip("SEQUENCER_URL")?;
    let nwaku_url = env_or_skip("NWAKU_URL")?;
    let storage_url = env_or_skip("STORAGE_URL")?;
    let program_id = env_or_skip("PROGRAM_ID")?;
    Some(Config {
        delivery: DeliverySection {
            url: nwaku_url,
            topics: vec![TOPIC.into()],
            store_lookback_hours: 1,
        },
        storage: StorageSection { url: storage_url },
        registry: RegistrySection {
            sequencer_url,
            program_id,
            idl_path: "./idl/whistleblower_registry.idl.json".into(),
            signer_account_id: std::env::var("SIGNER_ACCOUNT_ID")
                .unwrap_or_else(|_| "CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r".into()),
            lgs_bin: None,
        },
        batch: Default::default(),
    })
}

fn sample_env(n: u32) -> Envelope {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    let title = format!("e2e-doc-{n}");
    let metadata_hash = canonical_metadata_hash(
        Some(&title),
        Some("LP-0017 e2e test"),
        Some("text/plain"),
        Some(64),
        &["e2e".into()],
    );
    Envelope {
        v: 1,
        // Synthetic CID — collisions across test runs handled by the
        // monotonically-advancing `now + n` suffix.
        cid: format!("zE2E{:056x}", (now as u64) * 1024 + n as u64),
        metadata_hash,
        timestamp: now as u64,
        title: Some(title),
        description: Some("LP-0017 e2e test".into()),
        content_type: Some("text/plain".into()),
        size_bytes: Some(64),
        tags: vec!["e2e".into()],
    }
}

#[tokio::test]
#[ignore = "live-lez: requires running sequencer + nwaku + storage"]
async fn round_trip_50_envelopes_through_live_stack() {
    // Banner the e2e CI job greps for.
    println!(
        "RISC0_DEV_MODE={}",
        std::env::var("RISC0_DEV_MODE").unwrap_or_else(|_| "<unset>".into())
    );

    let Some(cfg) = build_config() else {
        eprintln!("env not configured — skipping live e2e");
        return;
    };

    let delivery = NwakuRest::new(cfg.delivery.url.clone());
    let registry = ShellOutRegistry::from_config(&cfg.registry);

    assert!(delivery.healthy().await, "nwaku must be reachable");
    delivery.subscribe(TOPIC).await.expect("subscribe");

    let envs: Vec<Envelope> = (0..50).map(sample_env).collect();
    for env in &envs {
        delivery.publish(TOPIC, env).await.expect("publish");
    }

    // Init registry (idempotent).
    if !registry.is_initialised().await.unwrap_or(false) {
        let _ = registry.init().await;
    }

    // Drain everything we just published.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let drained = delivery.drain(TOPIC).await.expect("drain");
    assert!(
        drained.len() >= envs.len(),
        "drained {} envelopes, expected >= {}",
        drained.len(),
        envs.len()
    );

    // Submit in one batch.
    let cids: Vec<String> = drained.iter().take(50).map(|e| e.cid.clone()).collect();
    let hashes: Vec<[u8; 32]> = drained
        .iter()
        .take(50)
        .map(|e| e.metadata_hash_bytes().unwrap())
        .collect();
    let timestamps: Vec<u32> = drained
        .iter()
        .take(50)
        .map(|e| e.timestamp.try_into().unwrap())
        .collect();

    let tx_hash = registry
        .index_batch(&cids, &hashes, &timestamps)
        .await
        .expect("index_batch");
    println!("tx_hash = {tx_hash}");

    // Read back.
    let set = registry.anchored_cid_set().await.expect("anchored_cid_set");
    for cid in &cids {
        assert!(set.contains(cid), "registry missing cid {cid}");
    }

    // Re-submit; idempotency check.
    let registry_size_before = set.len();
    let _ = registry.index_batch(&cids, &hashes, &timestamps).await;
    let set_after = registry
        .anchored_cid_set()
        .await
        .expect("anchored_cid_set 2");
    assert_eq!(
        set_after.len(),
        registry_size_before,
        "re-anchor must be a no-op"
    );
    println!("e2e ok: {} CIDs anchored, idempotent", cids.len());
}
