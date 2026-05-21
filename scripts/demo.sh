#!/usr/bin/env bash
# scripts/demo.sh — LP-0017 end-to-end demo.
#
# Runs the full upload → broadcast → batch-anchor → on-chain readback
# pipeline against a local lgs sequencer + the nwaku/storage stack in
# infra/docker-compose.yml. RISC0_DEV_MODE=0 is forced so the evaluator
# can confirm real proofs from the terminal banner.
#
# Prerequisites (printed on missing-tool exit):
#   - cargo, cargo-risczero, r0vm
#   - docker compose
#   - lgs (cargo install --git https://github.com/logos-co/spel.git spel)
#   - wallet (cargo install --git https://github.com/logos-blockchain/logos-execution-zone.git --tag v0.2.0-rc3 wallet)
#
# Re-runnable: every step writes a key=value pair to .demo-state so a
# rerun can short-circuit on the expensive cargo-risczero build + deploy.

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────
export RISC0_DEV_MODE=0
export LGS_NETWORK="${LGS_NETWORK:-localnet}"
export SEQUENCER_URL="${SEQUENCER_URL:-http://127.0.0.1:3040}"
export NWAKU_URL="${NWAKU_URL:-http://127.0.0.1:8645}"
export STORAGE_URL="${STORAGE_URL:-http://127.0.0.1:8080}"

STATE_FILE="${STATE_FILE:-.demo-state}"
TOPIC="/whistleblower/1/document-broadcast/json"
PROOF_BANNER_WIDTH=64

banner() {
  printf '┌'; printf '─%.0s' $(seq 1 $((PROOF_BANNER_WIDTH - 2))); printf '┐\n'
  printf '│ %-'"$((PROOF_BANNER_WIDTH - 4))"'s │\n' "$1"
  printf '└'; printf '─%.0s' $(seq 1 $((PROOF_BANNER_WIDTH - 2))); printf '┘\n'
}

# Print the value of RISC0_DEV_MODE as the very first line, so the
# narrated video / evaluator can confirm we're running real proofs.
banner "LP-0017 demo — RISC0_DEV_MODE=${RISC0_DEV_MODE}"
echo "▶ SEQUENCER_URL = ${SEQUENCER_URL}"
echo "▶ NWAKU_URL     = ${NWAKU_URL}"
echo "▶ STORAGE_URL   = ${STORAGE_URL}"
echo "▶ LGS_NETWORK   = ${LGS_NETWORK}"
echo

save() {
  grep -v "^$1=" "$STATE_FILE" 2>/dev/null > "$STATE_FILE.tmp" || true
  echo "$1=$2" >> "$STATE_FILE.tmp"
  mv "$STATE_FILE.tmp" "$STATE_FILE"
}

# shellcheck disable=SC1090
[ -f "$STATE_FILE" ] && source "$STATE_FILE"

# ── 0. Tool sanity ─────────────────────────────────────────────────────
banner "[0/6] sanity check"
for tool in cargo cargo-risczero docker lgs wallet spel jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing required tool: $tool"
    echo "Install it and re-run. See README.md §Prerequisites."
    exit 1
  fi
done

# ── 1. Bring up nwaku + storage ────────────────────────────────────────
banner "[1/6] start nwaku + storage (docker compose)"
docker compose -f infra/docker-compose.yml up -d
echo "waiting for healthchecks..."
for _ in $(seq 1 24); do
  if curl -sS "${NWAKU_URL}/health" >/dev/null \
     && curl -sS --max-time 2 -X HEAD "${STORAGE_URL}/data" >/dev/null 2>&1; then
    echo "  ok"
    break
  fi
  sleep 2
done

# ── 2. Start lgs localnet sequencer (if not already running) ───────────
banner "[2/6] start lgs localnet"
if ! curl -sS "${SEQUENCER_URL}/health" >/dev/null 2>&1; then
  lgs localnet start &
  for _ in $(seq 1 60); do
    curl -sS "${SEQUENCER_URL}/health" >/dev/null 2>&1 && break
    sleep 2
  done
else
  echo "  sequencer already running"
fi

# ── 3. Build guest + deploy ────────────────────────────────────────────
banner "[3/6] build guest + deploy"
if [ -z "${PROGRAM_ID:-}" ]; then
  cargo risczero build --manifest-path methods/guest/Cargo.toml
  GUEST_BIN=methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin
  DEPLOY_OUT=$(wallet deploy-program "$GUEST_BIN" 2>&1)
  echo "$DEPLOY_OUT"
  PROGRAM_ID=$(echo "$DEPLOY_OUT" | grep -oE '[0-9a-fA-F]{64}' | head -1)
  [ -z "$PROGRAM_ID" ] && { echo "could not parse PROGRAM_ID from deploy output"; exit 1; }
  save PROGRAM_ID "$PROGRAM_ID"
fi
export PROGRAM_ID
echo "PROGRAM_ID = $PROGRAM_ID"

# ── 4. Init the registry PDA ───────────────────────────────────────────
banner "[4/6] init registry PDA"
if [ -z "${REGISTRY_INIT_TX:-}" ]; then
  REGISTRY_INIT_TX=$(cargo run --release -p batch-anchor -- init || true)
  save REGISTRY_INIT_TX "$REGISTRY_INIT_TX"
fi
echo "init tx hash: $REGISTRY_INIT_TX"

# ── 5. Publish + watch ────────────────────────────────────────────────
banner "[5/6] publish a file + watch + anchor"

# Create a tiny synthetic file for the demo so the script is hermetic.
DEMO_FILE=".demo-state.demo-doc.txt"
date -u +'demo-doc generated at %Y-%m-%dT%H:%M:%SZ' > "$DEMO_FILE"

# Upload + broadcast in one shot.
cargo run --release -p batch-anchor -- publish "$DEMO_FILE" \
  --title "demo.txt" --description "LP-0017 demo upload" --tags demo,localnet

# Run the watcher with --once so the script terminates.
cargo run --release -p batch-anchor -- watch --once

# ── 6. Verify on-chain ────────────────────────────────────────────────
banner "[6/6] verify the anchor"
cargo run --release -p batch-anchor -- list

banner "demo complete — see $STATE_FILE for deployed addresses"
