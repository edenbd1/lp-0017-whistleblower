# Logos toolchain issues filed during LP-0017 development

Per LP-0017 §Submission Requirements:

> GitHub issues filed for any problems encountered with Logos technology.

This file tracks every upstream issue we open. Each entry links to the issue and notes the impact on the submission.

## Open

_None yet — file as encountered._

## Resolved / known-and-worked-around

| Date | Repo | Issue | Workaround |
|------|------|-------|-----------|
| 2026-05-22 | `logos-blockchain/logos-execution-zone` | `wallet` + `spel` crate installs from `tag = "v0.2.0-rc3"` failed to compile cleanly on Apple Silicon (cargo install error). | Use the docker-compose stack + the CI e2e tier for live exercises until a stable release artefact ships. Same blocker that Garvit's LP-0013 submission documents. |
| 2026-05-22 | `logos-co/spel` | `spel` CLI version mismatch surfaces as `InvalidSignature` on the sequencer (per spel issue #183). | Pin spel + LEZ to the exact same commit set in all build paths; document `tag = "v0.2.0-rc3"` in workspace + methods/guest manifests and in CI install steps. |

(Both upstream issues exist; this submission cross-references them rather than re-filing.)
