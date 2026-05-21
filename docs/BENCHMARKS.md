# CU Benchmarks — LP-0017

Per LP-0017 §Performance:

> Document and measure the compute unit (CU) cost of a single-CID anchor and a 50-CID batch anchor on LEZ devnet/testnet.

Methodology + table below. Numbers land once devnet access is granted (see [`DEPLOYMENT.md`](DEPLOYMENT.md)).

## Methodology

The `spel` CLI prints proof-generation stats (cycle counts and total CU) on every `submit` when run with `--verbose`. The CI `e2e.yml` workflow captures these from the `cargo test --features live-lez --test e2e_anchor` run and writes them to the action's artefact `e2e-anchor-log`.

```bash
export RISC0_DEV_MODE=0      # required — dev-mode CU is meaningless
export SEQUENCER_URL=https://<devnet>:3040
export PROGRAM_ID=<from DEPLOYMENT.md>

# Single-CID anchor.
spel index_batch \
    --cids "<cid-1>" \
    --metadata-hashes "<hash-1>" \
    --anchor-timestamps "<ts-1>" \
    --payer <signer> --verbose 2>&1 | tee .bench/single.log

# 50-CID batch.
spel index_batch \
    --cids "<csv of 50 cids>" \
    --metadata-hashes "<csv of 50 hashes>" \
    --anchor-timestamps "<csv of 50 timestamps>" \
    --payer <signer> --verbose 2>&1 | tee .bench/batch50.log

grep -E 'cycles|CU' .bench/single.log .bench/batch50.log
```

Each measurement is the average of three back-to-back runs; the CI
job runs against a freshly initialised registry so the second and
third invocations skip duplicate CIDs cheaply (matches the steady
state of the watcher in production).

## Table

| Operation                       | Cycles (live, n=3 avg) | CU (≈ cycles ÷ 100) | Wall-clock | Notes |
|----------------------------------|-----------------------:|--------------------:|-----------:|-------|
| `init_registry`                  | TBD                    | TBD                 | TBD        | Default-encoded `Registry`, one PDA claim |
| `index_batch` n=1                | TBD                    | TBD                 | TBD        | Anchor cost floor |
| `index_batch` n=10               | TBD                    | TBD                 | TBD        | Spec hard-floor |
| `index_batch` n=50               | TBD                    | TBD                 | TBD        | `MAX_BATCH` ceiling |
| `index_batch` n=50 (all dupes)   | TBD                    | TBD                 | TBD        | Idempotency cost — all entries skipped |

### Expected shape

The guest does at most: one Borsh decode of the `Registry` state (cost ~linear in entry count) + one Borsh encode + per-CID `BTreeMap::insert` (O(log n) per entry). Two borsh dereferences per instruction plus n × (hash + decode + insert). At n=50 we expect:

- Cycles ≈ `α + β · entries_total + γ · 50` for some empirically-determined `α, β, γ`.
- The 50-dupe case should be cheaper than the 50-new case because `try_insert` short-circuits before the insert.

The `e2e.yml` artefact log will contain enough data points to fit those constants.

## LEZ CU budget caveat

The per-transaction CU budget on the LEZ testnet is still being tuned. Numbers in the table are absolute; if the budget changes, every operation above should still fit comfortably — none of them allocate large heap structures or iterate unbounded collections beyond the already-bounded `Registry::entries` (capped at 900 by `MAX_ENTRIES`).
