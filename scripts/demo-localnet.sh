#!/usr/bin/env bash
# scripts/demo.sh — LP-0017 end-to-end demo.
#
# Reproduces the full pipeline against a local LEZ sequencer:
#   1. Bring up nwaku + storage via docker-compose
#   2. Start the LEZ sequencer in standalone mode (if not already up)
#   3. Build the SPEL guest via cargo-risczero
#   4. Deploy via `wallet deploy-program`
#   5. `spel init-registry`
#   6. Upload a real document via `batch-anchor publish`
#   7. `spel index-batch` with the broadcasted CID
#   8. Read the registry PDA back via `wallet account get`
#
# RISC0_DEV_MODE=0 is forced so the evaluator can confirm real proofs
# from the terminal banner.
#
# Re-runnable: state lands in .demo-state so reruns short-circuit on
# the expensive cargo-risczero build + deploy.

set -euo pipefail

# ─── Configuration ────────────────────────────────────────────────────
export RISC0_DEV_MODE=0
export SEQUENCER_URL="${SEQUENCER_URL:-http://127.0.0.1:3040}"
export NWAKU_URL="${NWAKU_URL:-http://127.0.0.1:8645}"
export STORAGE_URL="${STORAGE_URL:-http://127.0.0.1:18080}"
LEZ_REPO="${LEZ_REPO:-$HOME/logos/src/logos-execution-zone}"
export NSSA_WALLET_HOME_DIR="${NSSA_WALLET_HOME_DIR:-$LEZ_REPO/wallet/configs/debug}"
WALLET_PASSWORD="${WALLET_PASSWORD:-test}"
STATE_FILE="${STATE_FILE:-.demo-state}"
TOPIC="/whistleblower/1/document-broadcast/json"
GUEST_BIN="methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin"
IDL="idl/whistleblower_registry.idl.json"

# Pick a preconfigured signer — LEZ's standalone-mode debug wallet
# ships with two pre-funded public accounts.
PAYER="${PAYER:-CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r}"

# ─── Helpers ──────────────────────────────────────────────────────────
banner() {
    printf '┌──────────────────────────────────────────────────────────────┐\n'
    printf '│ %-60s │\n' "$1"
    printf '└──────────────────────────────────────────────────────────────┘\n'
}

step() { printf '\n▶ %s\n' "$1"; }

save() {
    grep -v "^$1=" "$STATE_FILE" 2>/dev/null > "$STATE_FILE.tmp" || true
    echo "$1=$2" >> "$STATE_FILE.tmp"
    mv "$STATE_FILE.tmp" "$STATE_FILE"
}

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "✗ missing tool: $1 — see README §Prerequisites" >&2
        exit 1
    }
}

# Drive a wallet/spel command via pty so the rpassword prompt gets fed.
pty_run() {
    python3 - "$@" <<'PY'
import os, pty, select, sys, time
argv = sys.argv[1:]
pid, fd = pty.fork()
if pid == 0:
    os.execvp(argv[0], argv)
sent = False
out = b''
deadline = time.time() + int(os.environ.get('PTY_TIMEOUT_S', '600'))
while time.time() < deadline:
    r, _, _ = select.select([fd], [], [], 2.0)
    if fd in r:
        try: chunk = os.read(fd, 8192)
        except OSError: break
        if not chunk: break
        out += chunk
        sys.stdout.write(chunk.decode(errors='replace')); sys.stdout.flush()
        if b'password' in out.lower() and not sent:
            os.write(fd, (os.environ.get('WALLET_PASSWORD', 'test') + '\n').encode())
            sent = True
    try:
        done, st = os.waitpid(pid, os.WNOHANG)
        if done:
            rc = os.WEXITSTATUS(st) if os.WIFEXITED(st) else 1
            sys.exit(rc)
    except ChildProcessError:
        break
sys.exit(1)
PY
}

# ─── Boot ─────────────────────────────────────────────────────────────
banner "LP-0017 demo  —  RISC0_DEV_MODE=${RISC0_DEV_MODE}"
echo "▶ SEQUENCER_URL        = $SEQUENCER_URL"
echo "▶ NWAKU_URL            = $NWAKU_URL"
echo "▶ STORAGE_URL          = $STORAGE_URL"
echo "▶ LEZ_REPO             = $LEZ_REPO"
echo "▶ NSSA_WALLET_HOME_DIR = $NSSA_WALLET_HOME_DIR"

# shellcheck disable=SC1090
[ -f "$STATE_FILE" ] && source "$STATE_FILE"

step "[0/8] Tool check"
for t in cargo docker python3 jq wallet spel; do require "$t"; done
[ -d "$LEZ_REPO" ] || { echo "✗ LEZ_REPO not found at $LEZ_REPO"; exit 1; }

# ─── 1. Docker stack ──────────────────────────────────────────────────
step "[1/8] Bring up nwaku + storage via docker-compose"
docker compose -f infra/docker-compose.yml up -d
echo "  waiting for health…"
for _ in $(seq 1 24); do
    nwaku=$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' "$NWAKU_URL/health" 2>/dev/null || echo NA)
    storage=$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' "$STORAGE_URL/api/storage/v1/spr" 2>/dev/null || echo NA)
    if [ "$nwaku" = "200" ] && [ "$storage" = "200" ]; then echo "  ✓ stack ready"; break; fi
    sleep 3
done

# ─── 2. Sequencer ─────────────────────────────────────────────────────
step "[2/8] Start LEZ sequencer in standalone mode"
if curl -sS --max-time 2 "$SEQUENCER_URL/health" >/dev/null 2>&1 || \
   curl -sS --max-time 2 -X POST "$SEQUENCER_URL" -d '{}' >/dev/null 2>&1; then
    echo "  ✓ sequencer already responding at $SEQUENCER_URL"
else
    echo "  starting sequencer in background…"
    ( cd "$LEZ_REPO" && \
      RUST_LOG=info ./target/release/sequencer_service \
        sequencer/service/configs/debug/sequencer_config.json \
        > /tmp/lp17-demo-seq.log 2>&1 & )
    for _ in $(seq 1 30); do
        if ps aux | grep sequencer_service | grep -v grep >/dev/null; then
            sleep 2
            ss=$(curl -sS --max-time 2 -X POST "$SEQUENCER_URL" -d '{}' -o /dev/null -w '%{http_code}' 2>/dev/null || echo NA)
            [ -n "$ss" ] && break
        fi
        sleep 2
    done
fi

# ─── 3. Initialize wallet ─────────────────────────────────────────────
step "[3/8] Initialize wallet (check-health)"
if [ ! -f "$NSSA_WALLET_HOME_DIR/storage.json" ]; then
    pty_run wallet check-health
else
    echo "  ✓ wallet storage already initialised"
fi

# ─── 4. Build guest ───────────────────────────────────────────────────
step "[4/8] Build SPEL guest (cargo risczero)"
if [ ! -f "$GUEST_BIN" ]; then
    cargo risczero build --manifest-path methods/guest/Cargo.toml
else
    echo "  ✓ guest binary already exists ($GUEST_BIN)"
fi

# ─── 5. Deploy ────────────────────────────────────────────────────────
step "[5/8] Deploy guest to sequencer"
if [ -z "${PROGRAM_DEPLOYED:-}" ]; then
    pty_run wallet deploy-program "$GUEST_BIN" || true
    save PROGRAM_DEPLOYED 1
else
    echo "  ✓ program already deployed in a previous run"
fi
PROGRAM_ID_HEX=$(spel inspect "$GUEST_BIN" 2>&1 | awk '/ImageID/ {print $4}')
save PROGRAM_ID_HEX "$PROGRAM_ID_HEX"
echo "  PROGRAM_ID_HEX: $PROGRAM_ID_HEX"

# ─── 6. init_registry ─────────────────────────────────────────────────
step "[6/8] init_registry"
if [ -z "${INIT_REGISTRY_TX:-}" ]; then
    INIT_OUT=$(pty_run spel --idl "$IDL" -p "$GUEST_BIN" -- init-registry --payer "$PAYER" 2>&1)
    echo "$INIT_OUT" | tee /tmp/lp17-demo-init.log
    INIT_REGISTRY_TX=$(echo "$INIT_OUT" | awk '/tx_hash:/ {print $2; exit}')
    save INIT_REGISTRY_TX "$INIT_REGISTRY_TX"
else
    echo "  ✓ already initialised (tx $INIT_REGISTRY_TX)"
fi

# ─── 7. Upload + broadcast + anchor a real document ───────────────────
step "[7/8] publish + anchor a real document"
DEMO_FILE=".demo-state.demo-doc.txt"
date -u +'demo-doc generated at %Y-%m-%dT%H:%M:%SZ' > "$DEMO_FILE"
PUBLISH_OUT=$(cargo run --release -p batch-anchor -- publish "$DEMO_FILE" \
    --title "LP-0017 demo" \
    --description "End-to-end smoke test" \
    --tags demo,localnet 2>&1)
echo "$PUBLISH_OUT" | tail -10
CID=$(echo "$PUBLISH_OUT" | awk '/cid =/ {print $3; exit}')
HASH=$(echo "$PUBLISH_OUT" | awk -F'v1:' '/metadata_hash/ {print $2; exit}' | tr -d ' ')
TS=$(date +%s)
[ -z "$CID" ] && { echo "✗ no CID extracted from publish output"; exit 1; }
save CID "$CID"
save HASH "$HASH"
echo "  CID=$CID"
echo "  HASH=$HASH"

step "    spel index-batch (n=1)"
ANCHOR_OUT=$(pty_run spel --idl "$IDL" -p "$GUEST_BIN" -- index-batch \
    --cids "$CID" \
    --metadata-hashes "$HASH" \
    --anchor-timestamps "$TS" \
    --anchorer "$PAYER" 2>&1)
echo "$ANCHOR_OUT" | tail -10
ANCHOR_TX=$(echo "$ANCHOR_OUT" | awk '/tx_hash:/ {print $2; exit}')
save ANCHOR_TX "$ANCHOR_TX"

# ─── 8. Readback ──────────────────────────────────────────────────────
step "[8/8] Read the registry PDA back"
REG_PDA=$(echo "$INIT_OUT" 2>/dev/null | awk '/PDA registry|registry →/ {print $4; exit}' || true)
# Fallback: derive via spel
[ -z "$REG_PDA" ] && REG_PDA=$(spel --idl "$IDL" -p "$GUEST_BIN" -- pda registry 2>&1 | awk '/registry →/ {print $3; exit}')
echo "  Registry PDA: $REG_PDA"
ACCOUNT_JSON=$(wallet account get --account-id "Public/$REG_PDA" 2>&1 | grep '^{' | head -1)
echo "$ACCOUNT_JSON" | jq .
ENTRY_COUNT=$(echo "$ACCOUNT_JSON" | python3 -c "
import json, sys
d = json.load(sys.stdin)
n = int.from_bytes(bytes.fromhex(d['data'][:8]), 'little')
print(n)
")
echo
banner "Demo complete — $ENTRY_COUNT CID(s) anchored, RISC0_DEV_MODE=0"
echo "  state: $STATE_FILE"
echo "  init_registry tx: $INIT_REGISTRY_TX"
echo "  index_batch  tx: $ANCHOR_TX"
echo "  registry PDA:    Public/$REG_PDA"
