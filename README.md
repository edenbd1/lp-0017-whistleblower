# LP-0017: Whistleblower

Censorship-resistant document upload and indexing on the Logos stack.

A Logos Basecamp app that uploads a document to **Logos Storage**, broadcasts the resulting CID over **Logos Delivery** so it is immediately discoverable, and optionally anchors the CID on-chain via a **LEZ registry program**. A permissionless **batch CLI** lets any third party gather broadcasted CIDs and commit them on-chain in a single transaction — with no coordination required from the original publisher.

> Submission for [LP-0017 on ns.com](https://ns.com/earn/lp-0017-whistleblower-censorship-resistant-document-upload-and-indexing-basecamp-app). Full brief at [`prizes/LP-0017.md`](https://github.com/logos-co/lambda-prize/blob/main/prizes/LP-0017.md).

## Status

Work in progress. Functional baseline implemented; live devnet deploy + narrated video are next.

See [`docs/SPEC_COMPLIANCE.md`](docs/SPEC_COMPLIANCE.md) for the per-criterion status map.

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
