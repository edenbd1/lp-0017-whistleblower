# Deployment

Status: **pending devnet access**. Coordination with the Logos team on Discord `#builder-hub` is the next step. This file is the template the final values land in.

## Local (localnet)

Reproducible from a clean clone via [`scripts/demo.sh`](../scripts/demo.sh). The script:

1. Brings up nwaku + storage from `infra/docker-compose.yml`.
2. Starts `lgs localnet` if `:3040/health` is not already responding.
3. Builds the guest with `cargo risczero build --manifest-path methods/guest/Cargo.toml`.
4. Calls `wallet deploy-program` and captures the `program_id`.
5. Initialises the registry PDA via `batch-anchor init`.
6. Publishes a synthetic file and runs `batch-anchor watch --once`.
7. Verifies the registry has at least one entry via `batch-anchor list`.

Localnet artefacts land in `.demo-state` so reruns short-circuit.

## Devnet

```
Network ID:       <pending Discord ack>
Sequencer URL:    <pending>
Storage gateway:  <pending>
Delivery fleet:   delivery-01.do-ams3.logos.dev.status.im
                  delivery-02.do-ams3.logos.dev.status.im
                  delivery-01.gc-us-central1-a.logos.dev.status.im
                  (already pinned in infra/docker-compose.yml as static peers)
```

| Artefact | Value | Verifier |
|---|---|---|
| `program_id`             | `<pending devnet deploy>` | `lgs program info <pid>` |
| Deploy `tx_hash`          | `<pending>`               | block explorer (TBD) |
| Registry PDA address      | `Public/registry@<prefix>` | `lgs wallet account get --raw <pda>` |
| `init_registry` `tx_hash` | `<pending>`               | block explorer |
| Sample `index_batch` (1 CID)  `tx_hash` | `<pending>` | block explorer |
| Sample `index_batch` (50 CIDs) `tx_hash` | `<pending>` | block explorer |
| Sample anchored CID       | `<pending>`               | `batch-anchor lookup <cid>` |

### Verification recipe

Once the table above lands, anyone can reproduce the read-side independently:

```bash
cargo build --release -p batch-anchor

# Point at the public devnet.
cat > batch-anchor.devnet.toml <<EOF
[registry]
sequencer_url     = "<from table above>"
program_id        = "<from table above>"
idl_path          = "./idl/whistleblower_registry.json"
signer_account_id = "<your wallet>"
EOF

./target/release/batch-anchor --config batch-anchor.devnet.toml list
./target/release/batch-anchor --config batch-anchor.devnet.toml lookup <SAMPLE_CID>
```

The nightly `verify-deployment.yml` workflow runs the lookup
automatically once `KNOWN_ANCHORED_CID` is set as a repo variable.

## Why a deployment proof matters

The competing PR #48 (Thompsonmina) pins a `program_id` that is just
the deterministic SHA of the guest binary — same value on every
sequencer that loads the same `.bin`. That's a build-time identity,
not a deployment proof. This file lands real on-chain tx hashes so a
third party can verify the program is live on a public network
without trusting our README.
