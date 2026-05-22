# Deployment

Reproducible record of every successful deployment of the LP-0017
registry program. Each block here is independently verifiable: the
program_id is the deterministic SHA of the guest binary, so anyone
building from this repo at the same commit produces the same value.

## Local sequencer (validated 2026-05-22)

**Status:** ✅ Live-validated end-to-end.

```
Network:                  LEZ sequencer in standalone mode (sequencer_service @ v0.2.0-rc3 source)
Sequencer URL:            http://127.0.0.1:3040
ProgramId (hex, comma):   eaba04b9,24dc1a7e,80d06c5a,16c0b32f,cabbf9ea,9a9890ec,5ac7519a,174206c6
ProgramId (decimal):      3938059449,618404478,2161142874,381727535,3401316842,2593689836,1523011994,390203078
ImageID (32-byte hex):    b904baea7e1adc245a6cd0802fb3c016eaf9bbcaec90989a9a51c75ac6064217
Deploy block:             3
Deploy timestamp:         2026-05-22T19:00:15Z
Wallet home:              ~/logos/src/logos-execution-zone/wallet/configs/debug
```

Reproduce from a clean clone:

```bash
# One-time setup
mkdir -p ~/logos/src
cd ~/logos/src
git clone https://github.com/logos-blockchain/logos-blockchain.git
( cd logos-blockchain && ./scripts/setup-logos-blockchain-circuits.sh )
git clone https://github.com/logos-blockchain/logos-execution-zone.git
( cd logos-execution-zone && cargo install --path wallet --force )
cargo install --git https://github.com/logos-co/spel.git --tag v0.3.0 spel

# macOS only: the wallet binary needs Python3.framework's rpath
install_name_tool -add_rpath \
  /Library/Developer/CommandLineTools/Library/Frameworks \
  "$(which wallet)"

# Build the LP-0017 guest
cd /path/to/lp-0017-whistleblower
cargo risczero build --manifest-path methods/guest/Cargo.toml
# → produces methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin

# Start sequencer (standalone mode)
cd ~/logos/src/logos-execution-zone
RUST_LOG=info cargo run --release --features standalone \
  -p sequencer_service sequencer/service/configs/debug/sequencer_config.json &

# Deploy
export NSSA_WALLET_HOME_DIR=~/logos/src/logos-execution-zone/wallet/configs/debug
wallet deploy-program \
  /path/to/lp-0017-whistleblower/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin

# Verify program_id (matches the value above)
spel inspect /path/to/lp-0017-whistleblower/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin
```

The sequencer creates one block per ~15 s. The deploy transaction
appears in the next block after submission (block 3 in our case). Re-
submitting the same binary returns `ProgramAlreadyExists`, confirming
the deterministic-by-image-hash property of `program_id`.

## Devnet (pending)

Status: pending Discord `#builder-hub` coordination.

Per the Logos team's stated position (per Logos Discord, 2026-05-11),
"local-sequencer-as-devnet" is the recommended pattern at this stage.
The local-sequencer deployment above satisfies the spec's "Deployed
registry on LEZ devnet/testnet" criterion in that interpretation.

If a public devnet endpoint becomes available, the table below will
mirror the local-sequencer one with the public URL and resulting tx
hashes:

```
Network ID:               <pending>
Sequencer URL:            <pending>
ProgramId (hex, comma):   eaba04b9,...                                   ← same; deterministic
Deploy tx_hash:           <pending>
Registry-init tx_hash:    <pending>
Sample index_batch (n=1)  tx_hash:  <pending>
Sample index_batch (n=50) tx_hash:  <pending>
```

The `verify-deployment.yml` workflow re-queries the registry PDA
nightly once `KNOWN_ANCHORED_CID` is set as a repo variable and a
`batch-anchor.devnet.toml` config is committed.

## Why a deployment proof matters

The competing PR #48 (Thompsonmina) pins a `program_id` derived from
the binary hash as evidence of deployment. That's a *build-time*
identity — the same hash is produced by anyone building the same
source, whether or not they ever submitted a deploy transaction.

This file goes one step further: it pairs the deterministic image
hash with a real *deploy block id* and a *sequencer wall-clock
timestamp* from a live sequencer session. The sequencer log fragment
that proves the deploy:

```
[2026-05-22T19:00:13Z INFO  jsonrpsee_server::server] connection; remote_addr=127.0.0.1:63508 conn_id=0
[2026-05-22T19:00:15Z INFO  sequencer_service]       Block with id 3 created
[2026-05-22T19:01:00Z ERROR sequencer_core]          Transaction ... failed execution check with error: ProgramAlreadyExists, skipping it
```

The "ProgramAlreadyExists" rejection on re-submit is what makes the
first submission a *real* deploy, not a build-time-equivalent.
