# Logos toolchain issues encountered during LP-0017 development

Per LP-0017 §Submission Requirements:

> GitHub issues filed for any problems encountered with Logos technology.

This file tracks every upstream issue we noticed. Drafts are ready to file; final filing waits on user approval (per the same hold-back that applies to the bounty PR — drafts published autonomously could surface in the wrong voice).

## Already documented upstream (no new file required)

| Date | Repo | Existing issue | Impact on this submission |
|------|------|-------|-----------|
| 2026-05-22 | `logos-co/spel` | [#183](https://github.com/logos-co/spel/issues/183) — scaffolded project uses wrong LEZ tag (`nssa_core` version mismatch) | Surfaces as `InvalidSignature` on the sequencer when `spel` + `wallet` + sequencer are built from different commits. We pin everything to `v0.2.0-rc3` / spel `v0.3.0` consistently to dodge this. |
| 2026-05-22 | `logos-blockchain/logos-execution-zone` | (no public issue) — `cargo install` from `tag = "v0.2.0-rc3"` fails on macOS Apple Silicon during dependency resolution | The CI e2e job uses Linux runners, so this only blocks local dev on M-series Macs. Local devs can run against the docker-compose stack only (storage + delivery half) without the full LEZ. |

## Drafts ready to file (held pending user approval)

### Draft 1 — `logos-storage/logos-storage-nim`: add `/health` endpoint

> **Title:** Add `/health` endpoint to the storage REST API
>
> **Body:**
> The storage daemon does not expose a `/health` endpoint. Plugin-side and CI-side health probes have to use `HEAD /data` (which returns 200 or 405) as a proxy. A real `/health` returning `{"status":"ok","peers":<N>,"quota_used":<N>}` would:
>
> - Make docker-compose `healthcheck:` simpler and less fragile.
> - Give Basecamp plugin authors a clean "is the storage backend up?" probe without coupling to the `/data` endpoint's response codes.
>
> Encountered while building https://github.com/edenbd1/lp-0017-whistleblower for LP-0017.

### Draft 2 — `logos-co/logos-delivery-module`: clarify "consume exactly once" semantics of `GET /relay/v1/auto/messages/<topic>`

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

### Draft 3 — `logos-co/spel`: document the `LogosAPIClient` integration pattern for UI plugins

> **Title:** Document the `LogosAPIClient` + `Q_INVOKABLE` bridge for Basecamp UI plugins
>
> **Body:**
> The current `spel` docs and the logos-basecamp `spec.md` describe the SPEL guest side (`#[lez_program]`, IDL, etc.) and the Basecamp host side (plugin discovery, UI plugin lifecycle), but there's no walk-through of the middle layer — how a `ui` C++ plugin connects to `storage_module` / `delivery_module` / a custom Logos Core module via `LogosAPIClient`. The chronicle module (Thompson's LP-17 submission) is the only complete public example.
>
> A short doc page covering:
>
> - `api->getClient("storage_module")` → `requestObject` → `invokeRemoteMethodAsync` flow.
> - The `onEvent` subscription pattern for async results (e.g. `storageUploadDone`).
> - The QVariantList type marshalling rules for the most common arg shapes (QUrl, QString, QByteArray, base64-encoded bytes).
>
> would unblock new Basecamp module authors and turn the "read chronicle's source" loop into a 15-minute read.
>
> Encountered while building https://github.com/edenbd1/lp-0017-whistleblower for LP-0017.

## Issues we did **not** file

- "`spel` install fails on Apple Silicon" — most likely a symptom of the toolchain ratcheting (Risc0 + multiple LEZ git tag pins). Would benefit from a `flake.nix` shim covering macOS, but the request belongs in `logos-module-builder` not in `logos-blockchain/logos-execution-zone`. Will defer until we can pinpoint the failure precisely.

- "nwaku `WAKUNODE2_CMD` env var ignored" — this turned out to be a GitHub Actions `services:` limitation, not a nwaku bug. Fixed in our e2e workflow by using docker-compose directly.

## How to file the drafts

Once the user gives the go-ahead, every draft is a 30-second `gh issue create` away:

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
