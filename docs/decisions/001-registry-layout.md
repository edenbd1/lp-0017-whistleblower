# ADR-001 — On-chain registry data layout

**Status:** Accepted, 2026-05-22
**Context for:** `methods/guest/src/bin/whistleblower_registry.rs`, `crates/registry-core/src/lib.rs`

## Decision

Adopt a **single-PDA HashMap** layout: one account at `pda = [literal("registry")]` holds a Borsh-serialized `Registry { entries: HashMap<String, CidRecord> }`. One transaction can write up to `MAX_BATCH = 50` records.

```rust
pub struct CidRecord {
    pub metadata_hash: [u8; 32],
    pub anchor_timestamp: i64,
    pub anchored_by: [u8; 32],
    pub version: u8,
}
```

PDA seed is a single ASCII literal. Wire format on the `index_batch` instruction is **three parallel vectors** of equal length (`Vec<String>` cids, `Vec<[u8;32]>` hashes, `Vec<u32>` timestamps) because the SPEL CLI cannot serialize tuples or structs through its IDL codegen.

## Alternatives considered

### PDA-per-CID (rejected)

```
pda = sha256(literal("cid:v1") || cid)
account holds CidRecord directly
```

Pros:
- Unbounded capacity.
- O(1) lookup per CID by deterministic PDA derivation, no whole-state read.
- No state-size cliff at ~900 entries (the 100 KiB account-data cap).

Cons:
- One PDA write **per CID** per batch — a 50-CID batch becomes 50 PDA initializations, each with its own compute cost. Bursts cost more.
- Cross-PDA enumeration ("list all anchored CIDs") requires off-chain indexing or a sequencer call per CID.
- Idempotency requires fetching each PDA before write (or eating the `AlreadyInitialized` error and ignoring) — extra round-trips for the off-chain batch CLI.
- Each PDA carries account-rent overhead.

The cap argument doesn't bind us: at 50 CIDs/batch the spec's hard floor is met, and 900 records is a long testing horizon. If we hit it, a v2 program can shard by month or by content-type prefix.

### `Vec<CidRecord>` instead of `HashMap` (rejected)

Pros: simpler Borsh, no hash bucket overhead.
Cons: linear scan on every dedup check (in-guest *and* off-chain). At 50 CIDs/batch the in-guest cost becomes O(N · M) where N is batch size and M is registry size — at 900 entries that's 45,000 string compares per `index_batch`. HashMap keeps it O(N).

### Multi-PDA "shard" layout (rejected for v1)

`pda = sha256(literal("registry") || shard_id)` where `shard_id = cid_hash % NUM_SHARDS`. Captures the "uncapped" benefit of PDA-per-CID while keeping per-shard batched writes. Adds enough complexity (shard selection, cross-shard lookup) that v1 punts on it.

## Comparison

| Property | Single-PDA HashMap (chosen) | PDA-per-CID | Vec | Sharded |
|---|---|---|---|---|
| Batched write cost | O(N) marginal | O(N) but each is a PDA init | O(N) | O(N) marginal |
| Lookup by CID | O(1) borsh decode then O(1) hash | O(1) PDA derive then borsh | O(M) | O(1) within shard |
| Listing | O(M) | needs off-chain index | O(M) | O(M / shards) per shard |
| Max records | ~900 | uncapped | ~5500 | uncapped |
| Re-anchor idempotency | `contains_key` in guest, free | needs per-PDA pre-check | linear scan | per-shard `contains_key` |
| Dedup seed by off-chain batch CLI | one PDA read | N PDA reads (or off-chain index) | one PDA read | N_shard PDA reads |

## Idempotency contract

In-guest at `index_batch` time:

```rust
for (cid, hash, ts) in zip(cids, hashes, timestamps) {
    if registry.entries.contains_key(&cid) { continue; }  // skip silently
    registry.entries.insert(cid, CidRecord { ... });
}
```

Off-chain at batch CLI startup:

```rust
let seed: HashSet<String> = registry_client.anchored_cid_set().await?;
batch_buffer.known.extend(seed);
```

This means re-broadcasted envelopes are dropped before they ever reach the sequencer. The CLI cannot double-anchor a CID even across kill-9-restart cycles, because the on-chain state is the dedup ground truth.

## Error codes

| Code | Name | When |
|---|---|---|
| 1 | `E_INVALID_HASH` | `metadata_hash` is not 32 bytes |
| 2 | `E_BAD_TIMESTAMP` | `timestamp == 0` or > `i64::MAX` |
| 3 | `E_BATCH_EMPTY` | `cids.is_empty()` |
| 4 | `E_BATCH_TOO_BIG` | `cids.len() > MAX_BATCH` |
| 5 | `E_REGISTRY_FULL` | total entry count > 900 (state-size cap) |
| 6 | `E_ARITY_MISMATCH` | the three input vectors are not the same length |

Numbers are stable across versions; reserved range is `6000–6099` after SPEL adds the standard offset.

## Consequences

- Single-PDA write keeps the off-chain batch CLI cheap (one PDA read at startup; one write per flush).
- 900-entry cap is the binding constraint at scale. If reached, a follow-up λPrize can ship the sharded design; existing entries remain queryable through the v1 program.
- The HashMap key being `String` (the CID) means Borsh sorts deterministically across guests — required for SPEL's chained-call validation invariants.
