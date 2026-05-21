# ADR-002 — Envelope schema on the Delivery topic

**Status:** Accepted, 2026-05-22
**Context for:** `crates/indexing/src/envelope.rs`, `crates/batch-anchor/src/delivery/`, `app/whistleblower/src/`

## Decision

Adopt the **chronicle envelope schema** verbatim — the one Thompson's PR #48 already uses on `/chronicle/1/document-index/json`. Our topic is `/whistleblower/1/document-broadcast/json` but the body shape is identical, so an independent batch-anchor tool can consume both:

```json
{
  "v": 1,
  "cid": "zDvZRwzkyHVgr59zFkX7vyfzK7oUP7Jc6k7qpFD9ssDi7V5fvdjw",
  "metadata_hash": "v1:8a3f...64hex...",
  "timestamp": 1715000000,
  "title": "leak.pdf",
  "description": "Internal memo",
  "content_type": "application/pdf",
  "size_bytes": 12345,
  "tags": ["leak", "internal"]
}
```

Encoding: UTF-8 JSON serialized as the Waku message `payload` (base64 wrap per nwaku REST). Topic name follows LIP-23: `/<app>/<version>/<name>/<encoding>`.

## Reasoning

### Interop wins beat differentiation

A bespoke schema would force a parallel batch-anchor tool. The whole *point* of LP-0017 is that anyone — an NGO, a journalist collective, an automated guardian — can subscribe and batch-anchor without coordination. Matching chronicle's schema means:

- Thompson's `batch-anchor` could consume our broadcasts (and vice-versa).
- We can use chronicle's `Envelope::from_payload` parser unchanged (`/tmp/lp17-thompson/batch-anchor/src/delivery/envelope.rs:7-66`) — already tested, already battle-checked.
- Spec interop is a strong signal to evaluators: "this team understands the ecosystem."

### Fields

| Field | Required | Anchor-critical | Purpose |
|---|---|---|---|
| `v` | yes | yes | Version. Reject `!=1` immediately. |
| `cid` | yes | yes | Multiformat CID, opaque string. Dedup key. |
| `metadata_hash` | yes | yes | `v1:<64 hex>` = `sha256(canonical-metadata-json)` (excluding `cid` and `metadata_hash`). |
| `timestamp` | yes | yes | Unix seconds. Anchor refuses `0` or future-by-more-than-clock-skew. |
| `title` | optional | no | Discovery. Free text. |
| `description` | optional | no | Discovery. Free text. |
| `content_type` | optional | no | MIME. Normalised lowercase. |
| `size_bytes` | optional | no | uint. Sanity-check vs storage manifest. |
| `tags` | optional | no | List of strings. |

### What is NOT in the envelope (and why)

- **No signer / publisher identity.** The point is anonymity. The on-chain `anchored_by` field records *who anchored*, not who originally published. Publication via nwaku is unsigned.
- **No content hash.** The CID *is* the content hash (multiformat sha256 over the Codex manifest). Storing it twice is redundant and an attack surface (which one is canonical?).
- **No retrieval URL.** CID + storage discovery is the retrieval path. Embedding a URL would centralise.
- **No `wrapped_key` for encryption.** Out of scope for v1 (per LP-0017 brief).

## `metadata_hash` definition

```text
metadata_hash = "v1:" + hex(sha256(canonical_json))

canonical_json := JSON object with ONLY the following keys in this order:
  { "title": ..., "description": ..., "content_type": ..., "size_bytes": ..., "tags": [...] }
  (omit any key that is null/empty)
```

Why: the on-chain registry stores the hash, not the metadata blob. A client can verify "this CID was anchored with this metadata" by recomputing the hash. The canonical form is required so two implementations agree on the bytes.

Reference impl: `crates/indexing/src/envelope.rs::canonical_metadata_hash(...)`.

## Topic naming

```
/whistleblower/1/document-broadcast/json
```

- Single namespace under our app name, leaving room for `/whistleblower/1/comments/json` etc. later.
- Version `1` matches `v` in the envelope.
- `document-broadcast` (not `document-index`) signals that anchoring is downstream.
- Encoding `json` matches the payload.

The batch CLI subscribes to this topic AND to `/chronicle/1/document-index/json` (configurable) so the same daemon can anchor for either app. See `crates/batch-anchor/src/config.rs::topics`.

## Validation pipeline

```
incoming bytes
  ├─ base64 decode      → fails: drop, log_warn
  ├─ utf-8 parse        → fails: drop, log_warn
  ├─ json parse         → fails: drop, log_warn
  ├─ v == 1             → else: drop, log_warn
  ├─ cid non-empty      → else: drop, log_warn
  ├─ timestamp > 0      → else: drop, log_warn
  ├─ metadata_hash 32B  → else: drop, log_warn
  └─ deduped vs known   → if dup: drop silently
```

The batch CLI does **not** reject duplicates as errors — they're the expected steady state once the anchoring loop is running.

## Cross-app dedup invariant

The on-chain registry is keyed on `cid` alone. Two envelopes with the same `cid` but different `metadata_hash` are both treated as duplicates by the registry — first-write wins, subsequent attempts are silent no-ops. This matches the spec's "idempotent" requirement and means clients should not rely on the anchored `metadata_hash` being any particular envelope's — only on the CID itself.

## Consequences

- `crates/indexing` is the canonical home for `Envelope`, `canonical_metadata_hash`, and the validator. Both batch CLI and Basecamp plugin import from it.
- Schema is *frozen for v1*. Any breaking change ships as `v: 2` on a new topic — old subscribers ignore it.
- If chronicle bumps their schema, we sync — see [`recon.md`](../recon.md) for the upstream watch.
