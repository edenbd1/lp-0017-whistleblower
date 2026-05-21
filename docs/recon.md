# Competitive Recon — LP-0017

Snapshot date: **2026-05-22**. Two PRs are open on [`logos-co/lambda-prize`](https://github.com/logos-co/lambda-prize), neither reviewed yet. First-come-first-served: the first submission that meets **all** criteria wins. This file captures the gaps in both contenders so we know exactly what we have to beat.

## Open PRs

| PR | Author | Repo | Filed | Status |
|---|---|---|---|---|
| [#48](https://github.com/logos-co/lambda-prize/pull/48) | `Thompsonmina` | [WhistleBlower-Logos-](https://github.com/Thompsonmina/WhistleBlower-Logos-) | 2026-05-12 | Open; reviewer `weboko` assigned, said "review later this week" on 2026-05-19. |
| [#58](https://github.com/logos-co/lambda-prize/pull/58) | `Tranquil-Flow` | [lp-0017-whistleblower](https://github.com/Tranquil-Flow/lp-0017-whistleblower) | 2026-05-20 | Open; no reviewer assigned. |

## PR #58 — Tranquil-Flow (~5% acceptance)

Marketing-heavy PR body claims `RISC0_DEV_MODE=0`, real Basecamp, 50-CID batch, CU benchmarks. The code does not back the claims.

| Gap | Evidence |
|---|---|
| Batch CLI doesn't actually subscribe to Delivery. | `batch/src/main.rs:70-77`: `if !cli.mock_delivery { anyhow::bail!(...) }`. Author admits in `README.md:147-148`: *"The headless batch CLI remains `--mock-delivery` until a non-Qt Delivery [is wired]."* The demo script (`scripts/demo.sh:118`) embeds `--mock-delivery`. |
| CI red on `main`. | 4 consecutive failed runs since 2026-05-17 on `cargo fmt --all -- --check` (`ui/ffi/tests/anchor_one_live.rs`). Most recent commit `c3f7084` did not fix it. |
| Live LEZ CI gated to `workflow_dispatch` + `exit 1`. | `.github/workflows/ci.yml:59`, `:74`. The job cannot have ever passed. |
| No verifiable devnet program ID. | `DEPLOYMENT.md:63`: `devnet program_id: <copy from lgs deploy>` — literal placeholder. |
| Indexing module not agnostic. | `indexing/` crate imports `whistleblower_core::{…}` in `traits.rs`, `batch.rs`, `publisher.rs`. |
| CU benchmarks tagged TBD. | `BENCHMARKS.md:70`: *"Devnet TBD — pending credentials."* |

## PR #48 — Thompsonmina (~30-40% acceptance)

Substantively closer to spec than Tranquil. Real chronicle module, real nwaku integration, real batch CLI, anchor verifiable on-chain by PDA read-back. CI is green. Three surgical gaps remain.

| Gap | Evidence |
|---|---|
| **No e2e in CI.** | `.github/workflows/ci.yml:16-25` enumerates skipped components: `examples/`, `methods/guest`, `ffi/`, `nix flake`. Only `chronicle_registry_core` + `batch-anchor` unit tests + `cargo fmt`. No sequencer ever spawns. |
| **No `RISC0_DEV_MODE=0` in actual demo scripts.** | `grep -rn RISC0_DEV_MODE` over `scripts/`, `demo.sh`, `Makefile`, `flake.nix` returns 0 hits. Mentioned only in `README.md:90` (the compliance table) — narrative, not enforced. |
| **No public devnet deployment.** | Every `sequencer_url` is `127.0.0.1:3040` (`batch-anchor.toml:11`, `scaffold.toml:36`). `program_id` is the SHA of the guest binary — that's a build-time identity, not a deployment proof. |

## What we have to beat

To outscore both PRs we must deliver, **in addition to matching Thompson's functional baseline**:

1. CI job that spawns `lgs localnet`, deploys the guest, runs an anchor round-trip with `RISC0_DEV_MODE=0` — and stays green on `main`.
2. `scripts/demo.sh` that explicitly `export RISC0_DEV_MODE=0`, echoes the value, and runs end-to-end from a clean clone.
3. A real public devnet deployment with a published txhash + program_id + registry PDA address, verifiable by any third party.
4. A document-indexing module that is honestly agnostic — separate crate, no Whistleblower-specific imports — with a documented public API.
5. Batch anchor CLI with real Logos Delivery subscription (nwaku REST, no mocks) plus store-protocol catch-up.

Items 4 and 5 also need to beat Thompson on agnosticism and Tranquil on delivery-realness respectively. See [`design.md`](design.md) for how each delta is implemented.

## Race timing

Reviewer `weboko` flagged Thompson's PR for review "later this week" on 2026-05-19. Today (2026-05-22) is the back end of that window. We need a credible submission filed within 3 days to land before or in the same review batch as Thompson.
