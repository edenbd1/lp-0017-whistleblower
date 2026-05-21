# Whistleblower — Basecamp Qt6/QML plugin

UI half of LP-0017. Three actions: pick a file → publish (upload +
broadcast) → anchor on-chain.

```
┌────────────────────────────────────────────────────────────────┐
│ Whistleblower                                                  │
│ Censorship-resistant document upload + on-chain anchoring      │
│                                                                │
│ [Choose file…] /tmp/leak.pdf                                   │
│                                                                │
│ Title       [ leak.pdf                                       ] │
│ Description [ (optional)                                     ] │
│ Tags        [ leak,internal                                  ] │
│                                                                │
│ [ Publish ] [ Anchor on-chain ]                                │
│                                                                │
│  Status: broadcast sent                                        │
│  CID:    zDvZRwzkyHVgr59zFkX7vyfzK7oUP7Jc6k7qpFD9ssDi7V5fvdjw  │
│  tx_hash: 0xabc…                                               │
└────────────────────────────────────────────────────────────────┘
```

## Architecture

* **`plugin.cpp`** — `IComponent` impl. Creates the `QQuickWidget`,
  registers the `WhistleblowerBackend` as a context property under the
  name `backend`, and loads `qml/Main.qml`.
* **`backend.cpp`** — Wires `storage_module.uploadUrl` and
  `delivery_module.send` via `LogosAPIClient`. Computes the canonical
  `metadata_hash` (matches `crates/indexing/src/envelope.rs::canonical_metadata_hash`)
  and the JSON envelope that goes on Waku.
* **`qml/Main.qml`** — File picker + metadata form + two action
  buttons + a status panel. No imperative networking — every signal
  goes through `backend`.

## Building

### Framework build (production)

Inside the `logos-module-builder` Nix dev shell:

```bash
cmake -S app/whistleblower -B build/whistleblower
cmake --build build/whistleblower
nix bundle --bundler github:logos-co/nix-bundle-lgx#portable .#whistleblower
```

This produces `whistleblower.lgx` ready to install via
`Basecamp → Modules → Install LGX Package`.

### Manual build (IDE / QML iteration)

```bash
cmake -S app/whistleblower -B build/whistleblower-manual \
    -DCMAKE_BUILD_TYPE=Debug
cmake --build build/whistleblower-manual
```

The manual build stubs out the LogosAPI symbols (preview-only mode)
so you can iterate on QML without bringing up the full Logos stack.
The publish/anchor buttons no-op gracefully — see
`backend.cpp::publish()` for the `LogosAPI not wired` branch.

### Drop into a running Basecamp

```bash
mkdir -p ~/Library/Application\ Support/Logos/LogosBasecampDev/plugins/whistleblower
cp build/whistleblower-manual/whistleblower.* \
   ~/Library/Application\ Support/Logos/LogosBasecampDev/plugins/whistleblower/
cp metadata.json module.json \
   ~/Library/Application\ Support/Logos/LogosBasecampDev/plugins/whistleblower/
./result/bin/LogosBasecamp --user-dir /tmp/wb-isolated
```

## Dependencies declared in `metadata.json`

```json
"dependencies": ["storage_module", "delivery_module"]
```

`package_manager` auto-fetches both before instantiating the plugin.
If either is missing Basecamp refuses to load us and surfaces a
red-cross popup (`UIPluginManager.cpp:155-162` in `logos-basecamp`).

## What lives in the Rust workspace

The on-chain path (`anchorLast()`) calls into `lp0017_ffi` — the C ABI
exposed by `crates/ffi`. The framework build links the cdylib next to
the plugin binary; the preview build short-circuits with a placeholder
status message so QML iteration doesn't require the cdylib on disk.

See `../crates/ffi/src/lib.rs` for the wire format the plugin uses.

## Files

- `metadata.json`      Basecamp manifest. Sole source of truth.
- `module.json`        Duplicate, for the lambda-prize validation bot.
- `CMakeLists.txt`     Framework + manual build paths.
- `src/plugin.h/cpp`   `WhistleblowerPlugin` (IComponent impl).
- `src/backend.h/cpp`  `WhistleblowerBackend` (LogosAPI wiring).
- `qml/Main.qml`       UI.
