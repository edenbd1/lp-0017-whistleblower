# Logos toolchain issues encountered during LP-0017 development

Per LP-0017 §Submission Requirements:

> GitHub issues filed for any problems encountered with Logos technology.

This file is the audit log of every upstream issue we noticed while building the submission. Each entry includes the reproducer, the workaround we shipped, and (for items not yet upstream) the ready-to-file title and body.

## Already documented upstream

| Date | Repo | Existing issue | Impact on this submission |
|------|------|-------|-----------|
| 2026-05-22 | `logos-co/spel` | [#183](https://github.com/logos-co/spel/issues/183) — scaffolded project uses wrong LEZ tag (`nssa_core` version mismatch) | Surfaces as `InvalidSignature` on the sequencer when `spel` + `wallet` + sequencer are built from different commits. We pin everything to `v0.2.0-rc3` / spel `v0.3.0` consistently to dodge this. |
| 2026-05-22 | `logos-blockchain/logos-execution-zone` | (no public issue) — `cargo install` from `tag = "v0.2.0-rc3"` fails on macOS Apple Silicon during dependency resolution | The CI e2e job uses Linux runners, so this only blocks local dev on M-series Macs. Local devs can run against the docker-compose stack only (storage + delivery half) without the full LEZ. |

## Guest-build papercuts encountered during live validation (workarounds in place)

Surfaced while running `cargo risczero build --manifest-path methods/guest/Cargo.toml` on 2026-05-22. Each one is patched in our `methods/guest/Cargo.toml` or guest source with a comment pointing back here so the patch can be reverted when upstream fixes ship.

| Symptom | Root cause | Our workaround |
|---|---|---|
| `error: rustc 1.88.0-dev is not supported by the following packages: ruint@1.18.0 requires rustc 1.90` | The Risc0 builder image (`risczero/risc0-guest-builder:r0.1.88.0`) ships rustc 1.88-dev. `ruint 1.18.0` bumped its `rust-version` to 1.90 in a recent point release. | Pinned `ruint = "=1.17.0"` in `methods/guest/Cargo.toml`. The better fix is for the Risc0 builder image to bump its toolchain. |
| `error[E0433]: failed to resolve: use of unresolved module or unlinked crate \`serde\`` | The `#[lez_program]` macro expansion emits `serde::de::DeserializeOwned` paths, requiring the consuming crate to depend on `serde` directly even when the user code doesn't import it. | Added `serde = { version = "1.0", features = ["derive"] }` to `methods/guest/Cargo.toml`. SPEL docs request — "guest crates must depend on serde directly" is a non-obvious gotcha. |
| `error[E0283]: type annotations needed` at `SpelError::custom(CODE, "lit".into())` | `SpelError::custom(code: u32, message: impl Into<String>)` + the macro-expanded call site combine to defeat type inference on rustc 1.88-dev. The compiler can't pick which `Into<String>` impl applies. | Replaced every `.into()` with `.to_string()` in our guest source. SPEL improvement — `SpelError::custom` should take `&str` or `String` concretely, not `impl Into<String>`, to avoid this inference cliff. |

## Issues prepared for upstream

### 1 — `logos-storage/logos-storage-nim`: add `/health` endpoint

> **Title:** Add `/health` endpoint to the storage REST API
>
> **Body:**
> The storage daemon does not expose a `/health` endpoint. Plugin-side and CI-side health probes have to use `HEAD /data` (which returns 200 or 405) as a proxy. A real `/health` returning `{"status":"ok","peers":<N>,"quota_used":<N>}` would:
>
> - Make docker-compose `healthcheck:` simpler and less fragile.
> - Give Basecamp plugin authors a clean "is the storage backend up?" probe without coupling to the `/data` endpoint's response codes.
>
> Encountered while building https://github.com/edenbd1/lp-0017-whistleblower for LP-0017.

### 2 — `logos-co/logos-delivery-module`: clarify "consume exactly once" semantics of `GET /relay/v1/auto/messages/<topic>`

> **Title:** Document destructive-drain semantics of the relay messages endpoint
>
> **Body:**
> Calling `GET /relay/v1/auto/messages/<urlencoded-topic>` clears the per-subscription queue on each successful response. This is surprising for HTTP REST conventions (a `GET` ought to be idempotent) and is not documented in the module's README. A note like:
>
> > Note: `GET /relay/v1/auto/messages/<topic>` is destructive — it returns *and removes* the messages buffered since the last call. Use the store-protocol endpoint (`GET /store/v3/messages?...`) for replay.
>
> would have saved at least a day during LP-0017 development. The current behaviour is correct (matches nwaku semantics); only the docs lag.
>
> Encountered while building https://github.com/edenbd1/lp-0017-whistleblower for LP-0017.

### 3 — `logos-co/spel`: document the `LogosAPIClient` integration pattern for UI plugins

> **Title:** Document the `LogosAPIClient` + `Q_INVOKABLE` bridge for Basecamp UI plugins
>
> **Body:**
> The current `spel` docs and the logos-basecamp `spec.md` describe the SPEL guest side (`#[lez_program]`, IDL, etc.) and the Basecamp host side (plugin discovery, UI plugin lifecycle), but there's no walk-through of the middle layer — how a `ui` C++ plugin connects to `storage_module` / `delivery_module` / a custom Logos Core module via `LogosAPIClient`.
>
> A short doc page covering:
>
> - `api->getClient("storage_module")` → `requestObject` → `invokeRemoteMethodAsync` flow.
> - The `onEvent` subscription pattern for async results (e.g. `storageUploadDone`).
> - The QVariantList type marshalling rules for the most common arg shapes (QUrl, QString, QByteArray, base64-encoded bytes).
>
> would unblock new Basecamp module authors and turn the "read existing module source" loop into a 15-minute read.
>
> Encountered while building https://github.com/edenbd1/lp-0017-whistleblower for LP-0017.

## Issues we deliberately did not file

- "`spel` install fails on Apple Silicon" — most likely a symptom of the toolchain ratcheting (Risc0 + multiple LEZ git tag pins). Would benefit from a `flake.nix` shim covering macOS, but the request belongs in `logos-module-builder` not in `logos-blockchain/logos-execution-zone`. Will defer until we can pinpoint the failure precisely.

- "nwaku `WAKUNODE2_CMD` env var ignored" — this turned out to be a GitHub Actions `services:` limitation, not a nwaku bug. Fixed in our e2e workflow by using docker-compose directly.

## Filing commands

```bash
gh issue create --repo logos-storage/logos-storage-nim \
  --title "Add /health endpoint to the storage REST API" \
  --body-file docs/issues/storage-health.md

gh issue create --repo logos-co/logos-delivery-module \
  --title "Document destructive-drain semantics of the relay messages endpoint" \
  --body-file docs/issues/delivery-drain-docs.md

gh issue create --repo logos-co/spel \
  --title "Document the LogosAPIClient + Q_INVOKABLE bridge for Basecamp UI plugins" \
  --body-file docs/issues/spel-ui-bridge-docs.md
```

(The body files are inlined above; create them under `docs/issues/` if we want the file-based flow.)
