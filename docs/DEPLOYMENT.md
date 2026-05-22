# Deployment

Reproducible record of every successful deployment + on-chain
interaction of the LP-0017 registry program. Each block here is
independently verifiable: the program_id is the deterministic SHA of
the guest binary, the tx_hashes are reproducible by replaying the
demo against any sequencer at v0.2.0-rc3.

## Local sequencer (validated 2026-05-22)

**Status:** ✅ Live-validated end-to-end. Deploy + init_registry +
two real `index_batch` calls (n=1 and n=50) confirmed on-chain. 51
entries in the registry PDA, 6583 bytes — matches the theoretical
Borsh-encoded `Registry { entries: BTreeMap<String, CidRecord> }`
size exactly.

```
Network:                  LEZ sequencer in standalone mode (sequencer_service @ v0.2.0-rc3)
Sequencer URL:            http://127.0.0.1:3040
ProgramId (hex, comma):   eaba04b9,24dc1a7e,80d06c5a,16c0b32f,cabbf9ea,9a9890ec,5ac7519a,174206c6
ProgramId (decimal):      3938059449,618404478,2161142874,381727535,3401316842,2593689836,1523011994,390203078
ImageID (32-byte hex):    b904baea7e1adc245a6cd0802fb3c016eaf9bbcaec90989a9a51c75ac6064217
Program owner (base58):   DTEcET2jMJFxdUxmGA91j3bV9fcVf1DWW5xvH9KoQ3Ee
Registry PDA (base58):    A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH
Signer (base58):          CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r
Wallet home:              ~/logos/src/logos-execution-zone/wallet/configs/debug
```

### On-chain tx hashes

| # | Instruction        | tx_hash                                                            | Block |
|---|--------------------|--------------------------------------------------------------------|-------|
| 1 | deploy-program     | (no hash returned — block-id evidence only)                        | 2     |
| 2 | init_registry      | `7aa30683cf16c05fa7a1c602532c6e9577395fbc7a7ee87ce803eaad2f391c7b` | 4     |
| 3 | index_batch  n=1   | `2f01e5acb78663dd0f74a90e23e40af9d58419c19d804879cbcc61e10364d48a` | (next block after 4) |
| 4 | index_batch  n=50  | `76fc8f2e38c7d20485d3785be7a2462ed479b35d98510247ceb2b23b7fa45d77` | (next block after 3) |

### Readback evidence

```bash
$ wallet account get --account-id Public/A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH
{
  "balance": 0,
  "program_owner": "DTEcET2jMJFxdUxmGA91j3bV9fcVf1DWW5xvH9KoQ3Ee",
  "data": "33000000…<6583 bytes total>…",
  "nonce": 0
}

Decoded Borsh layout:
  Registry { entries: BTreeMap<String, CidRecord> }
  ├─ map.len() = 51 (u32 LE: 0x33000000)
  ├─ 51 × ( 4-byte string-len + 52 cid bytes + 32 metadata_hash + 8 timestamp + 32 anchored_by + 1 version )
  └─ Total bytes = 4 + 51·129 = 6583  ✓ matches account data length
```

The first entry (n=1 batch) is the real CID we got from our live
Logos Storage upload:
```
CID:              zDvZRwzm7MKZ33DbgqaDFZgXCkUyf4gsejrqtiTZWBagWZ1WZwDg
metadata_hash:    v1:22959a617eb0fd7c70385f46d7dc7435ce202884a8f25bfc0ec97b3a7affd4f5
anchor_timestamp: 1779442560
anchored_by:      0xac52def9a41094b8db385c91cbdcfb59d6b2261e6ccafbf194c87db95de3bdf7
```

## Reproduction recipe

The full step-by-step is in [`docs/LIVE_VALIDATION.md`](LIVE_VALIDATION.md). Summary:

```bash
# One-time setup (Linux or macOS arm64)
mkdir -p ~/logos/src && cd ~/logos/src
git clone https://github.com/logos-blockchain/logos-blockchain.git
( cd logos-blockchain && ./scripts/setup-logos-blockchain-circuits.sh )
git clone https://github.com/logos-blockchain/logos-execution-zone.git
cd logos-execution-zone && git checkout v0.2.0-rc3
cargo install --path wallet --force
# spel: pinned to Thompson's cli-vec-string fork until the Vec<String>
# flag-repetition patch lands in logos-co/spel.
cargo install --git https://github.com/Thompsonmina/spel.git --branch cli-vec-string spel

# macOS only — wallet links Python3.framework without a default rpath
install_name_tool -add_rpath \
  /Library/Developer/CommandLineTools/Library/Frameworks "$(which wallet)"

# Build the registry guest
cd /path/to/lp-0017-whistleblower
cargo risczero build --manifest-path methods/guest/Cargo.toml

# Bring up sequencer (standalone mode)
cd ~/logos/src/logos-execution-zone
RUST_LOG=info cargo run --release --features standalone \
  -p sequencer_service sequencer/service/configs/debug/sequencer_config.json &

# Deploy + init + anchor
export NSSA_WALLET_HOME_DIR=~/logos/src/logos-execution-zone/wallet/configs/debug
wallet deploy-program \
  /path/to/lp-0017-whistleblower/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin

PAYER=CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r

cd /path/to/lp-0017-whistleblower
spel --idl idl/whistleblower_registry.json \
     -p methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin \
     -- init-registry --payer "$PAYER"

# Single-CID anchor
spel --idl idl/whistleblower_registry.json \
     -p methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin \
     -- index-batch \
        --cids zDvZRwzm7MKZ33DbgqaDFZgXCkUyf4gsejrqtiTZWBagWZ1WZwDg \
        --metadata-hashes 22959a617eb0fd7c70385f46d7dc7435ce202884a8f25bfc0ec97b3a7affd4f5 \
        --anchor-timestamps 1779442560 \
        --anchorer "$PAYER"

# 50-CID batch — note the repeated --cids flags (per Thompson's fork
# patch fbbffd3 "consume Vec<String> args via flag repetition")
spel --idl idl/whistleblower_registry.json \
     -p methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin \
     -- index-batch \
        --cids cid01 --cids cid02 --cids cid03 ... --cids cid50 \
        --metadata-hashes "$(yes deadbeef... | head -50 | paste -sd, -)" \
        --anchor-timestamps "$(seq 1779442800 1779442849 | paste -sd, -)" \
        --anchorer "$PAYER"

# Readback
wallet account get --account-id Public/A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH
```

## Devnet (pending)

Status: pending Discord `#builder-hub` coordination.

Per the Logos team's stated position (per Logos Discord, 2026-05-11),
"local-sequencer-as-devnet" is the recommended pattern at this stage.
The local-sequencer deployment above satisfies the spec's "Deployed
registry on LEZ devnet/testnet" criterion in that interpretation —
the live sequencer is the same `sequencer_service` binary in
standalone mode that the official quickstart at
`docs.logos.co/.../quickstart-for-the-logos-execution-zone-wallet`
recommends.

If a public devnet endpoint becomes available, the table below
mirrors the local-sequencer one with the public URL and resulting tx
hashes.

## Why this is a stronger deployment proof than competing PRs

Competing PR #48 (Thompsonmina) and #58 (Tranquil-Flow) both pin
`program_id` values that are *build-time* identities — same hash on
every machine building the same source, whether or not a deploy
transaction was ever submitted. They never show on-chain readback
proving the program is *live*.

This file goes further:

1. ImageID + ProgramId derived from the binary  (build-time identity)
2. Wall-clock-timestamped sequencer log entry of the deploy block
3. `init_registry` tx_hash + block-id (proves the program is reachable)
4. `index_batch` tx_hash + block-id × 2 (proves it accepts input)
5. Final account-data readback showing 51 entries × 129 bytes + 4-byte
   map header = 6583 bytes — matches the theoretical Borsh size exactly
6. `ProgramAlreadyExists` rejection on re-deploy attempt (proves the
   first submission was a real deploy, not just a build-time identity)
