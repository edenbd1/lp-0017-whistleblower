# Design — LP-0017 Whistleblower

## Goal

Ship a submission that satisfies every line of the LP-0017 success criteria, with the on-chain registry deployed live on the public Logos Execution Zone testnet and the full pipeline verifiable end-to-end.

## Architecture overview

```
                ┌─────────────────────────┐
                │  Basecamp Qt6/QML app   │
                │  (app/whistleblower/)   │
                └────┬───────────┬────────┘
                     │           │
        publishFile()│           │anchor()
                     ▼           ▼
       ┌─────────────────────────────────────┐
       │   document-indexing module          │  ◄── agnostic
       │   (crates/indexing + ffi/cdylib)    │      — only depends on
       └────┬───────────┬──────────┬─────────┘      storage/delivery/registry
            │           │          │                traits
            ▼           ▼          ▼
   ┌────────────┐ ┌──────────┐ ┌──────────┐
   │ storage_mod│ │delivery_m│ │  LEZ     │
   │  (Codex)   │ │ (nwaku)  │ │ registry │
   └────────────┘ └──────────┘ └──────────┘

                ┌──────────────────────────┐
                │  batch-anchor CLI (Rust) │  ◄── permissionless
                │  subscribe → dedup → tx  │
                └──┬─────────────────────┬─┘
                   │                     │
                   ▼                     ▼
              nwaku REST           LEZ registry
            (delivery topic)        (PDA write)
```

The Basecamp app and the batch CLI consume the same `Envelope` schema over the same Logos Delivery topic. The on-chain registry is the dedup ground truth — both paths converge there.

## Workspace layout

| Crate / module | Purpose | Agnostic? |
|---|---|---|
| `crates/registry-core` | Borsh `Registry` + `CidRecord` shared between guest, FFI, batch CLI. | yes |
| `crates/indexing` | Trait crate: `StorageClient`, `DeliveryClient`, `RegistryClient`. Any app can drop it in. | **yes — no Whistleblower deps** |
| `crates/batch-anchor` | CLI binary. Real nwaku REST `DeliveryClient` impl. In-memory + on-chain-seeded dedup. | n/a (consumer) |
| `crates/ffi` | `cdylib` exposing `index_batch` / `init_registry` / `lookup` for the Qt module. | yes |
| `methods/guest` | SPEL `#[lez_program]`: `init_registry`, `index_batch(cids, hashes, timestamps)`. PDA per program (`literal("registry")`). HashMap layout, MAX_BATCH = 50. | yes |
| `app/whistleblower` | Basecamp Qt6/QML plugin (C++ UI App, not `ui_qml`). | n/a (consumer) |

## Design pillars

### 1 — End-to-end CI with a real sequencer

`.github/workflows/e2e.yml` exercises the full stack:

1. Cache: cargo + Risc0 + `lgs` binary (~1.5 GB).
2. Install `lgs` via release artefact.
3. `cargo risczero build` the guest with `RISC0_DEV_MODE=0`.
4. `lgs localnet start`, wait for `:3040/health`.
5. Bring up nwaku via `docker compose -f infra/docker-compose.yml up -d nwaku`.
6. `wallet deploy-program` the guest, capture `PROGRAM_ID`.
7. Run `cargo test -p batch-anchor --features live-lez -- --include-ignored` — round-trips through nwaku publish, batch CLI drain, on-chain `index_batch`, on-chain readback.
8. Assert `tx_hash` non-empty + at least 1 anchored CID.

A second job `verify-deployment` hits the deployed program PDA read-only and asserts entries > 0. Runs nightly.

### 2 — `RISC0_DEV_MODE=0` baked into the demo

```bash
#!/usr/bin/env bash
# scripts/demo.sh — header
set -euo pipefail
export RISC0_DEV_MODE=0
export LGS_NETWORK="${LGS_NETWORK:-localnet}"
echo "▶ RISC0_DEV_MODE=$RISC0_DEV_MODE"
echo "▶ LGS_NETWORK=$LGS_NETWORK"
```

The evaluator sees the value in stdout; the narrated video captures the terminal banner.

### 3 — Public devnet deployment with verifiable tx hashes

Deployed live on `https://testnet.lez.logos.co`. `docs/DEPLOYMENT.md` records:

- Network ID
- Sequencer URL
- `program_id` (hex)
- Deploy `tx_hash`
- Registry PDA address
- `init_registry` `tx_hash`
- `index_batch` `tx_hash` (single CID, real Logos Storage CID)
- `index_batch` `tx_hash` (50 CIDs — exercises the MAX_BATCH path)

Each hash is a link to the public explorer at `https://explorer.testnet.lez.logos.co`. A read-only CI job (`verify-deployment`) re-runs that query nightly and asserts the registry has ≥ 1 entry.

## Functional baseline

| Criterion | Implementation |
|---|---|
| F1 Upload to Logos Storage → CID | QML plugin → `storage_module.upload*()` via `LogosAPIClient`; headless CLI helper for tests uses Codex REST `/api/storage/v1/data`. |
| F2 Broadcast envelope to Delivery topic | Plugin → `delivery_module.send()`. Topic: `/whistleblower/1/document-broadcast/json`. Envelope: `v: 1`, `cid`, `metadata_hash: "v1:<hex>"`, `timestamp`, plus optional title / description / content_type / size_bytes / tags. |
| F3 Optional on-chain anchor | Plugin "Anchor" button → FFI `index_batch(vec![cid], ...)`. |
| F4 Permissionless batch CLI | `batch-anchor watch`: nwaku REST subscribe + store-protocol catch-up + on-chain dedup seed + idempotent batch submit. |
| F5 On-chain registry, ≥10 CIDs/batch | SPEL guest, single PDA holding `Registry { entries: HashMap<String, CidRecord> }`. `MAX_BATCH = 50`. Queryable by CID. |
| F6 Reusable indexing module | `crates/indexing/` — no Whistleblower imports; usage example in `examples/indexing-consumer/`. |
| Reliability: retries / dedup / resume | `with_retry` exp backoff on upload; envelope hash dedup; store-protocol 24h lookback for resume. |
| Performance: CU 1-CID + 50-CID | Captured by `verify-deployment` job, written to `docs/BENCHMARKS.md`. |
| Submission package | MIT + Apache-2.0 dual licence, README + DEPLOYMENT.md, narrated demo video, GitHub issues prepared in `docs/BUGS_FILED.md` for any Logos toolchain papercut hit. |

## Out of scope

- Search / discovery / feeds (per spec).
- Content moderation, blocklists (per spec — permissionless).
- E2E encryption of document content (per spec — optional, not required).

## ADRs

- [ADR-001 — Registry data layout](decisions/001-registry-layout.md): single-PDA HashMap vs PDA-per-CID.
- [ADR-002 — Envelope schema](decisions/002-envelope-schema.md): wire format details.
- [ADR-003 — CI with a live sequencer](decisions/003-ci-with-sequencer.md): how the e2e workflow is structured.
- [ADR-004 — LEZ program vs zone SDK](decisions/004-lez-program-vs-zone-sdk.md): on-chain anchoring approach.
