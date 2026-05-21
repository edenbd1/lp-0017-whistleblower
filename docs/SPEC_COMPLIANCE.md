# Spec compliance — LP-0017

Mapping every criterion from `prizes/LP-0017.md` to the code that satisfies it. ✅ = implemented + tested; 🟡 = scaffolded, pending live verification; ⏳ = pending (planned).

## Functionality

| # | Criterion | Status | Evidence |
|---|---|---|---|
| F1 | Upload file to Logos Storage, return CID | 🟡 | `crates/batch-anchor/src/storage/codex.rs` (REST), `app/whistleblower/src/backend.cpp::publish()` (Qt module path) |
| F2 | Broadcast envelope to Delivery topic | ✅ | Envelope shape: `crates/indexing/src/envelope.rs`. Topic: `/whistleblower/1/document-broadcast/json`. nwaku REST publish: `crates/batch-anchor/src/delivery/nwaku.rs::publish()` |
| F3 | Optional anchor on-chain action | ✅ | `app/whistleblower/qml/Main.qml` — distinct "Anchor on-chain" button. Code path: `backend.cpp::anchorLast()` → `lp0017_ffi::lp0017_index_batch` |
| F4 | Permissionless batch CLI | ✅ | `crates/batch-anchor/src/cmd/watch.rs`. Real nwaku REST subscribe + drain. No `--mock-delivery` flag. Idempotent (in-guest `contains_key`) + resumable (24h store-protocol catch-up + on-chain dedup seed) |
| F5 | On-chain registry, queryable by CID, ≥10 CIDs/batch | ✅ | `methods/guest/src/bin/whistleblower_registry.rs`. `MAX_BATCH = 50`. PDA seed = `literal("registry")`. See [ADR-001](decisions/001-registry-layout.md) |
| F6 | Document-indexing module — extracted, agnostic | ✅ | `crates/indexing/` — separate crate, zero `whistleblower::` imports, public API documented in lib.rs. `grep -rn whistleblower crates/indexing/src/` returns 0 |

## Usability

| # | Criterion | Status | Evidence |
|---|---|---|---|
| U7 | Basecamp app GUI loadable in Basecamp | 🟡 | `app/whistleblower/` — `metadata.json`, plain QQuickWidget plugin, CMake framework + manual paths |
| U8 | SDK with README | ✅ | `crates/indexing/` lib.rs doc-headers + `app/whistleblower/README.md` |
| U9 | IDL via SPEL | 🟡 | Generated via `spel generate-idl` once devnet deploy lands; placeholder at `idl/whistleblower_registry.json` |

## Reliability

| # | Criterion | Status | Evidence |
|---|---|---|---|
| R10 | Upload retries with backoff | ✅ | `crates/indexing/src/retry.rs::with_retry` + `RetryConfig`. 5 tests pin the exponential backoff curve and exhaustion semantics |
| R11 | Delivery broadcast deduplicated | ✅ | `crates/batch-anchor/src/batch/mod.rs::BatchBuffer::push` returns `false` silently on duplicate. 8 tests |
| R12 | Batch tool resumes after interrupt | ✅ | `crates/batch-anchor/src/cmd/watch.rs::catch_up_from_store` — 24h lookback window combined with on-chain dedup seed means kill-9-restart cannot double-anchor |

## Performance

| # | Criterion | Status | Evidence |
|---|---|---|---|
| P13 | CU cost for 1-CID + 50-CID batch on devnet/testnet | ⏳ | `docs/BENCHMARKS.md` — methodology documented, awaiting devnet credentials (Discord #builder-hub) |

## Supportability — the kill-criteria

These are the three deltas this submission targets against [`docs/recon.md`](recon.md).

| # | Criterion | Status | Evidence |
|---|---|---|---|
| S14 | Program deployed on LEZ devnet/testnet | ⏳ | Pending; see `docs/DEPLOYMENT.md` |
| S15 | **E2E tests against LEZ sequencer in CI** | ✅ | `.github/workflows/e2e.yml` — `lgs localnet` + nwaku + storage as job services, runs `cargo test --features live-lez --test e2e_anchor`. **Delta #1 vs Thompson's PR.** |
| S16 | CI green on default branch | ✅ | `.github/workflows/ci.yml` (fmt + clippy + tests) passes on `main` |
| S17 | **README documents end-to-end usage** | ✅ | `README.md` Quickstart section + `app/whistleblower/README.md` |
| S18 | **`RISC0_DEV_MODE=0` in reproducible demo** | ✅ | `scripts/demo.sh` line 17: `export RISC0_DEV_MODE=0`. First stdout line is the banner. **Delta #2 vs Thompson's PR.** |
| S19 | Narrated video showing terminal output | ⏳ | Recorded in final pre-submission pass |

## Submission requirements

| Requirement | Status | Evidence |
|---|---|---|
| Public repo MIT + Apache-2.0 | ✅ | `LICENSE-MIT`, `LICENSE-APACHE`, `Cargo.toml` workspace inherit |
| Deployed registry on LEZ devnet/testnet + documented program address | ⏳ | `docs/DEPLOYMENT.md`. **Delta #3 vs Thompson's PR.** |
| Demo video showing: upload + Delivery findability + batch anchor + on-chain registry confirmation | ⏳ | Final pass |
| CU benchmarks for single + 50-CID batch | ⏳ | `docs/BENCHMARKS.md` |
| GitHub issues filed for any Logos tooling problems | ⏳ | `docs/BUGS_FILED.md` — opened as encountered |

## Out of scope (per spec)

- Content moderation, access control, blocklists.
- Full-text search or semantic indexing.
- End-user authentication / identity binding.
- Cross-chain anchoring.
- Hosted relay or backend service.

The repo enforces these by construction: the registry program is permissionless (anyone can `index_batch`, no allowlist), the batch CLI subscribes to a public topic with no auth header, and there is no hosted service — everything runs against user-owned nwaku + Codex nodes.
