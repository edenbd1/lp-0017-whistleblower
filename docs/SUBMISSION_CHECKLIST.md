# Submission checklist — LP-0017

Hand-off document for the user. Captures what's done, what's left, and the exact PR body to file on `logos-co/lambda-prize`.

## ✅ Done (autonomous build)

- [x] Workspace scaffold (4 crates + methods/guest + app/whistleblower + infra + scripts)
- [x] `registry-core` — Borsh types + 11 tests
- [x] `indexing` — 3 agnostic traits + Envelope + retry + 16 tests
- [x] `batch-anchor` — full CLI (watch, init, lookup, list, publish, doctor), real nwaku REST, BatchBuffer with on-chain-seeded dedup, ShellOutRegistry, 29 tests
- [x] `ffi` — JSON-in / JSON-out C ABI for the Basecamp plugin, 4 tests
- [x] `methods/guest` — SPEL `#[lez_program]` with `init_registry` + `index_batch` (parallel-vector args, MAX_BATCH=50, in-guest dedup)
- [x] `app/whistleblower` — Basecamp Qt6/QML plugin (manifest, plugin.cpp, backend.cpp wiring storage_module + delivery_module, Main.qml UI)
- [x] `infra/docker-compose.yml` — nwaku v0.38.0 + logos-storage-nim, healthchecks, logos.dev cluster
- [x] `scripts/demo.sh` — `RISC0_DEV_MODE=0` banner as first stdout line, end-to-end reproducible
- [x] `.github/workflows/ci.yml` — fast tier (fmt + clippy + tests). **GREEN on main.**
- [x] `.github/workflows/e2e.yml` — spawns nwaku + storage + lgs + risc0, runs live-lez gated round-trip
- [x] `.github/workflows/verify-deployment.yml` — nightly devnet read-only
- [x] All docs: README, recon, design, 3 ADRs, SPEC_COMPLIANCE, DEPLOYMENT, BENCHMARKS, BUGS_FILED
- [x] Makefile + .editorconfig
- [x] `batch-anchor.toml.example`
- [x] `idl/whistleblower_registry.json` placeholder
- [x] Dual MIT + Apache-2.0 licence

## ⏳ Manual steps required before filing the PR

These need either external coordination or human verification — I can't do them autonomously.

1. **Verify the e2e workflow ends green**, or at least past the cache step. Latest runs are at https://github.com/edenbd1/lp-0017-whistleblower/actions. If e2e is still failing, the most likely next blocker is the `lgs localnet` startup command — the workflow currently falls back to launching `sequencer_service` if `lgs --help` doesn't mention `localnet`, but the fallback path may need refinement.

2. **Devnet deployment** (per `docs/DEPLOYMENT.md`):
   - Ping `#builder-hub` on Logos Discord for a devnet sequencer URL.
   - Run `bash scripts/deploy.sh` against that URL (override `SEQUENCER_URL` env).
   - Capture `program_id`, deploy `tx_hash`, registry-init `tx_hash`.
   - Commit the filled-in `DEPLOYMENT.md`.

3. **CU benchmarks** (per `docs/BENCHMARKS.md`):
   - The e2e CI run produces an `e2e-anchor-log` artefact with the cycle counts.
   - Drop the numbers into the table at `docs/BENCHMARKS.md`.

4. **Narrated demo video** (S19 in `SPEC_COMPLIANCE.md`):
   - 5-10 minute walkthrough.
   - First frame: show `scripts/demo.sh`'s `RISC0_DEV_MODE=0` banner in the terminal.
   - Narrate: architecture overview → run the demo end-to-end → show the on-chain readback via `batch-anchor lookup <cid>`.
   - Upload to YouTube (unlisted is fine), link from README.

5. **File the submission PR on `logos-co/lambda-prize`**. Body template at the bottom of this file.

## 🔍 Where to verify the kill-criteria deltas

The three concrete differences against Thompson's PR #48 — pasted verbatim into the PR body below.

| Delta | Evidence |
|---|---|
| 1. E2E in CI with `RISC0_DEV_MODE=0` | [`.github/workflows/e2e.yml`](../.github/workflows/e2e.yml) spawns nwaku + storage + lgs, runs `cargo test --features live-lez --test e2e_anchor`, asserts the `RISC0_DEV_MODE=0` banner is in stdout. |
| 2. `RISC0_DEV_MODE=0` in demo script | [`scripts/demo.sh`](../scripts/demo.sh) lines 17-23: `export RISC0_DEV_MODE=0` + first banner line echoes the value. |
| 3. Verifiable public devnet deployment | [`docs/DEPLOYMENT.md`](DEPLOYMENT.md) — template with the four tx_hash fields. Filled once Discord coordination completes. |

## 📨 PR body template

Paste into `logos-co/lambda-prize` "New PR" once steps 1-4 above are done. Title: `Solution: LP-0017 — Whistleblower`.

```markdown
Implementation of LP-0017: censorship-resistant document upload + indexing on the Logos stack.

**Repository:** https://github.com/edenbd1/lp-0017-whistleblower (public, MIT + Apache-2.0)

**Narrated demo video:** <YouTube link>

## Highlights vs the spec

- **Real Delivery integration** — `batch-anchor` subscribes to a live nwaku node via the REST relay + store-protocol catch-up. No mock-delivery shortcuts; the binary refuses to start without a reachable nwaku endpoint.
- **Agnostic indexing module** — `crates/indexing/` is a standalone trait crate with zero `whistleblower::` imports. Any Logos application that needs the upload → broadcast → anchor pipeline can drop it in.
- **E2E in CI with `RISC0_DEV_MODE=0`** — `.github/workflows/e2e.yml` spawns nwaku v0.38.0, logos-storage-nim, and `lgs localnet` as job steps, deploys the SPEL guest with real proofs, and runs a 50-CID round-trip. The workflow asserts the `RISC0_DEV_MODE=0` banner is present in stdout before passing.
- **Reproducible demo** — `scripts/demo.sh` exports `RISC0_DEV_MODE=0` as its first non-comment line, echoes the value to stdout, and runs the full pipeline end-to-end from a clean clone.
- **Public devnet deployment** — see `docs/DEPLOYMENT.md` for program_id, deploy tx hash, registry-init tx hash, and a sample 50-CID `index_batch` tx hash. All verifiable via the read-only `verify-deployment.yml` workflow that runs nightly against the published address.

## Per-criterion compliance map

See [`docs/SPEC_COMPLIANCE.md`](https://github.com/edenbd1/lp-0017-whistleblower/blob/main/docs/SPEC_COMPLIANCE.md) for a row-by-row mapping of every brief criterion to the code and tests that satisfy it.

## Architecture

LEZ registry: single PDA holding a Borsh `Registry { entries: BTreeMap<String, CidRecord> }`. `index_batch` accepts three parallel vectors (cids, metadata_hashes, anchor_timestamps), in-guest dedup via `contains_key`, MAX_BATCH=50. Decision write-up: [ADR-001](https://github.com/edenbd1/lp-0017-whistleblower/blob/main/docs/decisions/001-registry-layout.md).

Envelope schema: shared verbatim with chronicle (interop). Wire form locked at v=1 with `cid`, `metadata_hash: "v1:<hex>"`, `timestamp`, plus optional discovery metadata. Decision: [ADR-002](https://github.com/edenbd1/lp-0017-whistleblower/blob/main/docs/decisions/002-envelope-schema.md).

CI strategy: fast tier on every push (<4 min), e2e tier on main + nightly with the full sequencer + nwaku stack. Decision: [ADR-003](https://github.com/edenbd1/lp-0017-whistleblower/blob/main/docs/decisions/003-ci-with-sequencer.md).
```
