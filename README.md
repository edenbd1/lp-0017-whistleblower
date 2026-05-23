# LP-0017: Whistleblower

Censorship-resistant document upload and indexing on the Logos stack.

A Logos Basecamp app that uploads a document to **Logos Storage**, broadcasts the resulting CID over **Logos Delivery** so it is immediately discoverable, and optionally anchors the CID on-chain via a **LEZ registry program**. A permissionless **batch CLI** lets any third party gather broadcasted CIDs and commit them on-chain in a single transaction — with no coordination required from the original publisher.

> Submission for [LP-0017 on ns.com](https://ns.com/earn/lp-0017-whistleblower-censorship-resistant-document-upload-and-indexing-basecamp-app). Full brief at [`prizes/LP-0017.md`](https://github.com/logos-co/lambda-prize/blob/main/prizes/LP-0017.md).

## Status

**✅ Submission ready — deployed live on the public Logos Execution Zone testnet.**

- Sequencer: `https://testnet.lez.logos.co`
- Block explorer: `https://explorer.testnet.lez.logos.co`
- Registry PDA: [`A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH`](https://explorer.testnet.lez.logos.co/account/A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH) — holds 51 anchored CIDs in 6583 bytes (Borsh-encoded, exactly `4 + 51 × 129`)
- ProgramId (hex): `b904baea7e1adc245a6cd0802fb3c016eaf9bbcaec90989a9a51c75ac6064217`

**6 public on-chain transactions — each independently verifiable via `getTransaction` JSON-RPC or by clicking the explorer link:**

| # | Instruction | Explorer link |
|---|---|---|
| 1 | `wallet auth-transfer init` (signer account) | [`dd55dd1e…7b97f0`](https://explorer.testnet.lez.logos.co/transaction/dd55dd1e5b754fb975f7b5e523bee1cc361aee78e56f904d1f152ff1747b97f0) |
| 2 | `wallet pinata claim` (faucet → 150 tokens) | [`40b7966d…7476b4`](https://explorer.testnet.lez.logos.co/transaction/40b7966dd494645d7eaa2669ccbd734e254aecf6a359160508c7ff42707476b4) |
| 3 | **`wallet deploy-program`** | [`9e499b12…48c8a`](https://explorer.testnet.lez.logos.co/transaction/9e499b12781422f445d0e425f0b7499d4c975d3f96e12c9c0c35afb3dba48c8a) |
| 4 | **`spel init-registry`** | [`ae57ff1b…131d9`](https://explorer.testnet.lez.logos.co/transaction/ae57ff1bf480c949af23a1ae53592abbe3c44240632364fce0dc7624e0b131d9) |
| 5 | **`spel index_batch` n=1** (real Logos Storage CID) | [`1257c61c…ef55b`](https://explorer.testnet.lez.logos.co/transaction/1257c61c3ddff0ec083ef4756a81b28bc058ba55a11b147ef41ba3275edef55b) |
| 6 | **`spel index_batch` n=50** (50 CIDs anchored atomically) | [`2af12289…9d531`](https://explorer.testnet.lez.logos.co/transaction/2af12289409c55e8cee1ac172c35da518c0576e83a2ffaac7c8a67978209d531) |

Quick links:

- 📺 **Narrated video walkthrough:** https://youtu.be/J7eCklx3gEg
- 🧭 **Per-criterion compliance map:** [`docs/SPEC_COMPLIANCE.md`](docs/SPEC_COMPLIANCE.md)
- 🔗 **Full deployment record:** [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md)
- 📊 **CU benchmarks (measured live on this testnet):** [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md)
- 📦 **Basecamp `.lgx` plugin asset:** [release v0.1.0-rc1](https://github.com/edenbd1/lp-0017-whistleblower/releases/tag/v0.1.0-rc1)

## Architecture

```
                Basecamp Qt6/QML app
                (app/whistleblower)
                       │
            publishFile() │ anchor()
                       ▼
       ┌────────────────────────────────────┐
       │  document-indexing module          │  agnostic SDK
       │  (crates/indexing + crates/ffi)    │  any Logos app can drop in
       └──┬───────────────┬──────────────┬──┘
          ▼               ▼              ▼
   storage_module   delivery_module   LEZ registry
   (Codex)          (nwaku)           (SPEL guest)

                Permissionless batch CLI
                (crates/batch-anchor)
            subscribe → dedup → batch tx
```

* **`crates/registry-core`** — Borsh types shared between guest, FFI, and CLI. Single source of truth for the wire format. 11 tests.
* **`crates/indexing`** — Agnostic trait crate: `StorageClient`, `DeliveryClient`, `RegistryClient`, plus the canonical envelope schema and a retry helper. No Whistleblower-specific imports. 16 tests.
* **`crates/batch-anchor`** — Permissionless anchor CLI. Real nwaku REST subscribe + Codex REST upload + sled-free dedup seeded from on-chain. No mocks. 29 tests.
* **`crates/ffi`** — JSON-in / JSON-out C ABI for the Basecamp module. 4 tests.
* **`methods/guest`** — SPEL `#[lez_program]` for the registry. Single PDA + HashMap, `MAX_BATCH = 50`, in-guest dedup via `contains_key`.
* **`app/whistleblower`** — Basecamp Qt6/QML plugin. File picker → publish (upload + broadcast) → anchor button.
* **`docs/`** — Competitive recon, design notes, three ADRs ([registry layout](docs/decisions/001-registry-layout.md), [envelope schema](docs/decisions/002-envelope-schema.md), [CI with live sequencer](docs/decisions/003-ci-with-sequencer.md)).
* **`infra/docker-compose.yml`** — nwaku v0.38.0 + logos-storage-nim. Hermetic local stack.
* **`scripts/`** — `demo.sh`, `deploy.sh`, `ci-local.sh`. `demo.sh` exports `RISC0_DEV_MODE=0` as its first line.
* **`.github/workflows/`** — `ci.yml` (fast tier, fmt + clippy + tests), `e2e.yml` (live anchor round-trip with `RISC0_DEV_MODE=0`), `verify-deployment.yml` (nightly devnet read-only).

## Quickstart

### Prerequisites

```bash
# Rust + Risc0
rustup toolchain install stable
curl -L https://risczero.com/install | bash
rzup install cargo-risczero 3.0.5
rzup install r0vm           3.0.5

# Logos toolchain (SPEL pinned to v0.3.0, LEZ wallet to v0.2.0-rc3)
cargo install --git https://github.com/logos-co/spel.git --tag v0.3.0 spel
cargo install --git https://github.com/logos-blockchain/logos-execution-zone.git --tag v0.2.0-rc3 wallet

# Docker (for the local nwaku + storage stack)
docker --version
```

### End-to-end demo

```bash
git clone https://github.com/edenbd1/lp-0017-whistleblower.git
cd lp-0017-whistleblower

# Brings up nwaku + storage + lgs localnet, deploys the guest,
# uploads + broadcasts a synthetic doc, and runs the batch CLI in
# --once mode. First line of stdout is the RISC0_DEV_MODE=0 banner.
bash scripts/demo.sh
```

### Just the headless CLI

```bash
cargo build --release -p batch-anchor

# Health check the stack:
./target/release/batch-anchor doctor

# Run the anchor loop:
./target/release/batch-anchor watch
```

### Just the Basecamp app

See [`app/whistleblower/README.md`](app/whistleblower/README.md).

## Key facts for evaluators

| Criterion | Where |
|---|---|
| F4: permissionless batch CLI subscribes to real Delivery | [`crates/batch-anchor/src/delivery/nwaku.rs`](crates/batch-anchor/src/delivery/nwaku.rs) — no `--mock-delivery` flag, ever |
| F5: ≥10 CIDs/batch on-chain | [`crates/registry-core/src/lib.rs`](crates/registry-core/src/lib.rs) — `MAX_BATCH = 50` |
| F6: agnostic indexing module | [`crates/indexing/`](crates/indexing/) — zero `whistleblower::` imports |
| S15: e2e in CI with `RISC0_DEV_MODE=0` | [`.github/workflows/e2e.yml`](.github/workflows/e2e.yml) — spawns nwaku + storage + lgs, runs the live-lez round-trip |
| S17: `RISC0_DEV_MODE=0` in demo | [`scripts/demo.sh`](scripts/demo.sh) — first non-comment line: `export RISC0_DEV_MODE=0` |
| Deployed devnet | [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md) |
| CU benchmarks | [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) |

## License

Dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
