# Design — LP-0017 Whistleblower

## Goal

Ship a submission that scores **PASS** on every line of the LP-0017 success criteria, with deliberate margin on the 3 axes where competing PRs (see [`recon.md`](recon.md)) underperform.

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
       │   chronicle-style indexing module   │  ◄── this is agnostic
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
| `crates/batch-anchor` | CLI binary. Real nwaku REST `DeliveryClient` impl. sled-free in-memory + on-chain-seeded dedup. | n/a (consumer) |
| `crates/ffi` | `cdylib` exposing `index_batch` / `init_registry` / `lookup` for the Qt module. | yes |
| `methods/guest` | SPEL `#[lez_program]`: `init_registry`, `index_batch(cids, hashes, timestamps)`. PDA per program (`literal("registry")`). HashMap layout, MAX_BATCH = 50. | yes |
| `app/whistleblower` | Basecamp Qt6/QML plugin (C++ UI App, not `ui_qml`). | n/a (consumer) |

## Kill-criteria deltas vs Thompson

These are the three concrete differences that lift our acceptance probability from Thompson's ~30-40% to ~85%.

### Delta 1 — End-to-end CI with a real sequencer

Thompson's CI explicitly excludes `examples/`, `methods/guest`, `ffi/`, and the Nix flake (`.github/workflows/ci.yml:16-25`). Ours adds a job `e2e-anchor`:

1. Cache: cargo + Risc0 + `lgs` binary (~1.5 GB).
2. Install `lgs` via release artefact.
3. `cargo risczero build` the guest with `RISC0_DEV_MODE=0`.
4. `lgs localnet start`, wait for `:3040/health`.
5. Bring up nwaku via `docker compose -f infra/docker-compose.yml up -d nwaku`.
6. `wallet deploy-program` the guest, capture `PROGRAM_ID`.
7. Run `cargo test -p batch-anchor --features live-lez -- --include-ignored` — round-trips through nwaku publish, batch CLI drain, on-chain `index_batch`, on-chain readback.
8. Assert `tx_hash` non-empty + at least 1 anchored CID.

A second job `verify-deployment` hits a known devnet program PDA read-only and asserts entries > 0. Runs nightly.

### Delta 2 — `RISC0_DEV_MODE=0` baked into the demo

```bash
#!/usr/bin/env bash
# scripts/demo.sh — header
set -euo pipefail
export RISC0_DEV_MODE=0
export LGS_NETWORK="${LGS_NETWORK:-localnet}"
echo "▶ RISC0_DEV_MODE=$RISC0_DEV_MODE"
echo "▶ LGS_NETWORK=$LGS_NETWORK"
```

The evaluator sees the value in stdout; the narrated video captures the terminal banner. No "compliance table claims compliance" gap.

### Delta 3 — Public devnet deployment with verifiable txhash

We coordinate via Logos `#builder-hub` Discord, get a devnet sequencer endpoint, and deploy. We commit `docs/DEPLOYMENT.md`:

- Network ID
- Sequencer URL
- `program_id` (hex)
- Deploy `tx_hash`
- Registry PDA address
- `init_registry` `tx_hash`
- One sample `index_batch` `tx_hash` (single CID)
- One sample `index_batch` `tx_hash` (50 CIDs — exercises the MAX_BATCH path)

Each hash is a link to the public explorer or, if no explorer ships yet, the `lgs wallet account get --raw <PDA>` output as evidence. A read-only CI job (`verify-deployment`) re-runs that query nightly and asserts the registry has ≥ 1 entry.

## Functional baseline (matches Thompson)

| Criterion | Implementation |
|---|---|
| F1 Upload to Logos Storage → CID | QML plugin → `storage_module.upload*()` via `LogosAPIClient`; headless CLI helper for tests uses Codex REST `/data`. |
| F2 Broadcast envelope to Delivery topic | Plugin → `delivery_module.send()`. Topic: `/whistleblower/1/document-broadcast/json`. Envelope = Thompson's schema (`v: 1`, `cid`, `metadata_hash: "v1:<hex>"`, `timestamp`, plus title/desc/tags). |
| F3 Optional on-chain anchor | Plugin "Anchor" button → FFI `index_batch(vec![cid], ...)`. |
| F4 Permissionless batch CLI | `batch-anchor watch`: nwaku REST subscribe + store-protocol catch-up + on-chain dedup seed + idempotent batch submit. |
| F5 On-chain registry, ≥10 CIDs/batch | SPEL guest, single PDA holding `Registry { entries: HashMap<String, CidRecord> }`. `MAX_BATCH = 50`. Queryable by CID. |
| F6 Reusable indexing module | `crates/indexing/` — no Whistleblower imports; usage example in `examples/indexing-consumer/`. |
| Reliability: retries / dedup / resume | `with_retry` exp backoff on upload; envelope hash dedup; store-protocol 24h lookback for resume. |
| Performance: CU 1-CID + 50-CID | Captured by `verify-deployment` job, written to `docs/BENCHMARKS.md`. |
| Submission package | MIT + Apache-2.0 dual licence, README + DEPLOYMENT.md, narrated demo video, GitHub issues filed for any Logos toolchain papercut hit. |

## Out of scope

- Search / discovery / feeds (per spec).
- Content moderation, blocklists (per spec — permissionless).
- E2E encryption of document content (per spec — optional, not required).

## Open risks

1. **Basecamp/Qt6 plugin pipeline.** New territory. Mitigation: fork Tranquil's `ui/` skeleton (MIT-licensed) — it's structurally correct, just lacks the `metadata.json` filename Basecamp's `package_manager` reads.
2. **Devnet sequencer access.** Logos Discord coordination required. Fallback: document local-sequencer-as-devnet (Logos position per Discord 2026-05-11, also cited by Thompson) plus an additional `lgs basecamp` push to a community sequencer.
3. **Race timing.** Thompson's reviewer signalled a review window of 2026-05-19 → 2026-05-22+. We need to file before or concurrent with the review.

## ADRs

- [ADR-001 — Registry data layout](decisions/001-registry-layout.md): single-PDA HashMap vs PDA-per-CID.
- [ADR-002 — Envelope schema](decisions/002-envelope-schema.md): adopt Thompson's schema verbatim for interop.
- [ADR-003 — CI with a live sequencer](decisions/003-ci-with-sequencer.md): how to ship Delta 1.
