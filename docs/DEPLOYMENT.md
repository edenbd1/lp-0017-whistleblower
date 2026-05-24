# Deployment

Reproducible record of every successful deployment + on-chain
interaction of the LP-0017 registry program. Each block is
independently verifiable: every tx_hash here is queryable on the
public Logos LEZ testnet via the JSON-RPC `getTransaction` method.

## Public LEZ testnet (validated 2026-05-23)

**Status:** ✅ **Live on `https://testnet.lez.logos.co`** — the
public Logos Execution Zone testnet. 51 CIDs anchored across two
real `index_batch` transactions (n=1 + n=50). Registry PDA holds
6583 bytes of account data, exactly matching the theoretical Borsh
size for `Registry { entries: BTreeMap<String, CidRecord> }` at 51
entries.

```
Network:                  Public LEZ testnet
Sequencer JSON-RPC:       https://testnet.lez.logos.co
Block height at deploy:   ~20828
ProgramId (hex, comma):   eaba04b9,24dc1a7e,80d06c5a,16c0b32f,cabbf9ea,9a9890ec,5ac7519a,174206c6
ImageID (32-byte hex):    b904baea7e1adc245a6cd0802fb3c016eaf9bbcaec90989a9a51c75ac6064217
Program owner (base58):   DTEcET2jMJFxdUxmGA91j3bV9fcVf1DWW5xvH9KoQ3Ee
Registry PDA (base58):    A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH
Signer (base58):          CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r
Account-init tx_hash:     dd55dd1e5b754fb975f7b5e523bee1cc361aee78e56f904d1f152ff1747b97f0
Faucet (pinata) tx_hash:  40b7966dd494645d7eaa2669ccbd734e254aecf6a359160508c7ff42707476b4
```

### On-chain tx hashes + explorer links (public testnet)

Every tx is independently verifiable on the public block explorer at
**https://explorer.testnet.lez.logos.co** — click any link below.

| # | Instruction              | Explorer link |
|---|--------------------------|---------------|
| 1 | `wallet auth-transfer init` (signer account) | [`dd55dd1e…7b97f0`](https://explorer.testnet.lez.logos.co/transaction/dd55dd1e5b754fb975f7b5e523bee1cc361aee78e56f904d1f152ff1747b97f0) |
| 2 | `wallet pinata claim` (faucet → 150 tokens)  | [`40b7966d…7476b4`](https://explorer.testnet.lez.logos.co/transaction/40b7966dd494645d7eaa2669ccbd734e254aecf6a359160508c7ff42707476b4) |
| 3 | **`wallet deploy-program`**                  | [`9e499b12…48c8a`](https://explorer.testnet.lez.logos.co/transaction/9e499b12781422f445d0e425f0b7499d4c975d3f96e12c9c0c35afb3dba48c8a) |
| 4 | **`spel init-registry`**                     | [`ae57ff1b…131d9`](https://explorer.testnet.lez.logos.co/transaction/ae57ff1bf480c949af23a1ae53592abbe3c44240632364fce0dc7624e0b131d9) |
| 5 | **`spel index-batch` (n=1, real Logos Storage CID)** | [`1257c61c…ef55b`](https://explorer.testnet.lez.logos.co/transaction/1257c61c3ddff0ec083ef4756a81b28bc058ba55a11b147ef41ba3275edef55b) |
| 6 | **`spel index-batch` (n=50, batch ceiling)**  | [`2af12289…9d531`](https://explorer.testnet.lez.logos.co/transaction/2af12289409c55e8cee1ac172c35da518c0576e83a2ffaac7c8a67978209d531) |

**Accounts on the explorer**:

- Registry PDA (6583 bytes of `Registry { entries: BTreeMap<…> }` state with 51 anchored CIDs):
  https://explorer.testnet.lez.logos.co/account/A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH
- Signer / anchorer:
  https://explorer.testnet.lez.logos.co/account/CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r

Every hash is also queryable via JSON-RPC:

```bash
curl -sS -X POST https://testnet.lez.logos.co \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getTransaction",
       "params":["9e499b12781422f445d0e425f0b7499d4c975d3f96e12c9c0c35afb3dba48c8a"]}' | jq .
```

### Live registry readback (public testnet)

```bash
$ export NSSA_WALLET_HOME_DIR=~/logos/src/logos-execution-zone/wallet/configs/debug
$ wallet account get --account-id Public/A9ewyji3THdFGqLAtAd9GkoPX9B9R6yb5LZCfWLxbAeH
{
  "balance": 0,
  "program_owner": "DTEcET2jMJFxdUxmGA91j3bV9fcVf1DWW5xvH9KoQ3Ee",
  "data": "33000000…<6583 bytes>…",
  "nonce": 0
}

$ batch-anchor --config batch-anchor.devnet.toml lookup zDvZRwzm7MKZ33DbgqaDFZgXCkUyf4gsejrqtiTZWBagWZ1WZwDg
{
  "cid": "zDvZRwzm7MKZ33DbgqaDFZgXCkUyf4gsejrqtiTZWBagWZ1WZwDg",
  "metadata_hash": "v1:22959a617eb0fd7c70385f46d7dc7435ce202884a8f25bfc0ec97b3a7affd4f5",
  "anchor_timestamp": 1779442560,
  "anchored_by": "ac52def9a41094b8db385c91cbdcfb59d6b2261e6ccafbf194c87db95de3bdf7",
  "version": 1
}

$ batch-anchor --config batch-anchor.devnet.toml list | wc -l
51
```

Decoded Borsh layout matches the theoretical size exactly:

```
Registry { entries: BTreeMap<String, CidRecord> }
  ├─ map.len() = 51 (u32 LE: 0x33000000)
  ├─ 51 × ( 4-byte string-len + 52 cid bytes + 32 metadata_hash + 8 timestamp + 32 anchored_by + 1 version )
  └─ Total bytes = 4 + 51 × 129 = 6583 ✓ matches account data length
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
# spel: a fork that adds Vec<String> flag-repetition CLI parsing
# (see docs/BUGS_FILED.md for the upstream patch we filed).
cargo install --git https://github.com/edenbd1/spel.git --branch cli-vec-string spel

# macOS arm64 only: patch wallet's rpath for Python3.framework
install_name_tool -add_rpath \
  /Library/Developer/CommandLineTools/Library/Frameworks "$(which wallet)"

# Build the registry guest
cd /path/to/lp-0017-whistleblower
cargo risczero build --manifest-path methods/guest/Cargo.toml

# Point wallet at the public testnet
export NSSA_WALLET_HOME_DIR=~/logos/src/logos-execution-zone/wallet/configs/debug
wallet config set sequencer_addr https://testnet.lez.logos.co

# One-time per-account: init + claim faucet (150 tokens)
PAYER=CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r
wallet auth-transfer init --account-id Public/$PAYER
wallet pinata claim --to Public/$PAYER

# Deploy
wallet deploy-program \
  methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin

# Init + anchor — capture tx_hashes from stdout
spel --idl idl/whistleblower_registry.idl.json \
     -p methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin \
     -- init-registry --payer "$PAYER"

spel --idl idl/whistleblower_registry.idl.json \
     -p methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin \
     -- index-batch \
        --cids zDvZRwzm7MKZ33DbgqaDFZgXCkUyf4gsejrqtiTZWBagWZ1WZwDg \
        --metadata-hashes 22959a617eb0fd7c70385f46d7dc7435ce202884a8f25bfc0ec97b3a7affd4f5 \
        --anchor-timestamps 1779442560 \
        --anchorer "$PAYER"

# Readback
batch-anchor --config batch-anchor.devnet.toml list
batch-anchor --config batch-anchor.devnet.toml lookup zDvZRwzm7MKZ33DbgqaDFZgXCkUyf4gsejrqtiTZWBagWZ1WZwDg
```

## Local sequencer (also validated 2026-05-22)

Status: ✅ Validated on the local LEZ sequencer in standalone mode
before promoting to the public testnet. Same program_id (since
program_id is a deterministic hash of the guest binary). See git
history for the local-only deployment record.

## Deployment evidence

Every action above is paired with a real public-testnet `tx_hash`
queryable via `https://testnet.lez.logos.co`'s JSON-RPC and visible
on the public block explorer at `https://explorer.testnet.lez.logos.co`.
The whole audit chain — account init → faucet → deploy → init_registry
→ index_batch — is independently reproducible by any third party with
the recipe above. `program_id` is a deterministic hash of the guest
binary, so anyone running `cargo risczero build` against the committed
source obtains the same `program_id` that the deployed program owns
on chain.
