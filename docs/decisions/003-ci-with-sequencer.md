# ADR-003 — CI with a live LEZ sequencer

**Status:** Accepted, 2026-05-22
**Context for:** `.github/workflows/`

## Decision

Run an end-to-end anchor round-trip in CI against a live `lgs localnet` sequencer with `RISC0_DEV_MODE=0`. Two workflows:

1. **`ci.yml`** — fast tier on every push/PR: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (in-process, no sequencer). Target wall-clock: < 4 minutes.
2. **`e2e.yml`** — slow tier on push to `main`, nightly, and `workflow_dispatch`: spawns `lgs localnet`, deploys the guest, runs an anchor round-trip. Target wall-clock: ≤ 30 minutes.

A separate `verify-deployment.yml` runs nightly on `main` only — hits the public devnet (see ADR + DEPLOYMENT.md), reads the registry PDA, asserts entries ≥ 1.

## Why this matters

A live-sequencer e2e workflow is what makes "the on-chain registry actually works end-to-end" a CI-enforced invariant rather than a manual claim. The host-only test tier (`ci.yml`) covers unit semantics; the live tier (`e2e.yml`) covers the full pipeline against a real sequencer with `RISC0_DEV_MODE=0`. Together they keep regressions in the load-bearing path visible on every push to `main`.

## `e2e.yml` outline

```yaml
name: e2e

on:
  push:
    branches: [main]
  schedule:
    - cron: '17 3 * * *'   # nightly
  workflow_dispatch:

jobs:
  anchor-roundtrip:
    runs-on: ubuntu-latest-8core
    timeout-minutes: 35
    env:
      RISC0_DEV_MODE: "0"
      LGS_NETWORK: localnet
    steps:
      - uses: actions/checkout@v4

      - name: cache cargo + risc0
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            ~/.risc0
            target
          key: ${{ runner.os }}-e2e-${{ hashFiles('**/Cargo.lock', 'rust-toolchain.toml') }}

      - name: install rust + risc0
        run: |
          rustup toolchain install stable
          curl -L https://risczero.com/install | bash
          ~/.risc0/bin/rzup install cargo-risczero 3.0.5
          ~/.risc0/bin/rzup install r0vm 3.0.5
          echo "$HOME/.risc0/bin" >> $GITHUB_PATH

      - name: install lgs CLI
        run: |
          curl -sSL -o /tmp/lgs.tgz \
            https://github.com/logos-co/lgs/releases/download/v0.2.0-rc3/lgs-linux-x86_64.tgz
          tar -xzf /tmp/lgs.tgz -C /usr/local/bin/

      - name: start nwaku
        run: docker compose -f infra/docker-compose.yml up -d nwaku

      - name: start sequencer
        run: |
          lgs localnet start &
          for i in {1..60}; do
            curl -sS http://127.0.0.1:3040/health && break
            sleep 2
          done

      - name: build guest
        run: cargo risczero build --manifest-path methods/guest/Cargo.toml

      - name: deploy + capture program_id
        id: deploy
        run: |
          PROGRAM_ID=$(./scripts/deploy.sh --output-format=hex)
          echo "program_id=$PROGRAM_ID" >> $GITHUB_OUTPUT

      - name: e2e anchor round-trip
        env:
          PROGRAM_ID: ${{ steps.deploy.outputs.program_id }}
          SEQUENCER_URL: http://127.0.0.1:3040
          NWAKU_URL: http://127.0.0.1:8645
        run: |
          cargo test -p batch-anchor --features live-lez \
            --test e2e_anchor -- --include-ignored --nocapture

      - name: assert RISC0_DEV_MODE was active
        run: |
          # Sanity: the test prints "RISC0_DEV_MODE=0" at startup; the
          # banner must appear or the run is invalid.
          grep -q "RISC0_DEV_MODE=0" /tmp/e2e-anchor.log || exit 1

      - name: upload artefacts
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-anchor-logs
          path: /tmp/e2e-anchor.log
```

The `cargo test --features live-lez --test e2e_anchor` target lives at `crates/batch-anchor/tests/e2e_anchor.rs`. It:

1. Publishes 50 envelopes to nwaku via REST.
2. Starts `batch-anchor watch` as a subprocess for 60 s.
3. Polls the registry PDA via the FFI / `lgs` until ≥ 50 entries appear or 5 min elapses.
4. Performs `lookup` on three random CIDs from the batch.
5. Asserts each lookup hits.
6. Performs a re-publish of the same envelopes; asserts the registry size does **not** grow (idempotency).
7. Captures the deploy `tx_hash`, init `tx_hash`, and one batch `tx_hash`; writes them to `/tmp/e2e-anchor.log` and prints them to the runner log.

## Caching strategy

| Path | Why |
|---|---|
| `~/.cargo/registry`, `~/.cargo/git`, `target` | Standard Rust cache. ~1 GB cold, ~5 min cold install. |
| `~/.risc0` | Risc0 toolchain. ~700 MB. Avoids re-install per run. |
| `~/.cache/lgs` | lgs's own model + binary cache. Optional. |

The `Cargo.lock + rust-toolchain.toml` hash is the cache key — bumping either invalidates and triggers a rebuild, which is correct.

## `ci.yml` (fast tier) outline

```yaml
name: ci
on: [push, pull_request]
jobs:
  fmt-clippy-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - name: cache cargo
        uses: actions/cache@v4
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-ci-${{ hashFiles('**/Cargo.lock') }}
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace --exclude lp0017-methods-guest
```

The guest crate is excluded from the host workspace test run because it builds for `riscv32im-risc0-zkvm-elf`; building it is `e2e.yml`'s job.

## Consequences

- Fast tier runs in < 4 min — devex-friendly.
- Slow tier on `main` proves the e2e path actually works, and the artefact upload gives evaluators a reproducible log.
- Cache misses on a cold runner cost ~12 min (`lgs` install + risc0 install + cargo build). With caches warm, the loop is ~6 min.
- A single test failure in `e2e_anchor` blocks merge to `main` — strong signal but creates a hard "the sequencer changed under us" failure mode. Mitigation: pin LEZ to `tag = "v0.2.0-rc3"` in `Cargo.toml` and refuse to bump unless `e2e.yml` is green on the bumped revision.
- `verify-deployment.yml` (nightly, read-only) catches devnet regressions independently of code changes on our side.
