#!/usr/bin/env bash
# scripts/deploy.sh — build the guest + deploy + write the program_id
# to stdout (single hex line). Used by the e2e CI job and the demo.

set -euo pipefail

OUTPUT_FORMAT="text"
for arg in "$@"; do
  case "$arg" in
    --output-format=hex) OUTPUT_FORMAT="hex" ;;
    --help|-h)
      echo "deploy.sh — build + deploy the LP-0017 registry guest"
      echo "  --output-format=hex   print only the 64-char hex program_id on stdout"
      exit 0
      ;;
  esac
done

export RISC0_DEV_MODE=0

cargo risczero build --manifest-path methods/guest/Cargo.toml >/dev/null

GUEST_BIN=methods/guest/target/riscv32im-risc0-zkvm-elf/docker/whistleblower_registry.bin
test -f "$GUEST_BIN" || { echo "guest binary missing at $GUEST_BIN" >&2; exit 1; }

DEPLOY_OUT=$(wallet deploy-program "$GUEST_BIN")
PROGRAM_ID=$(echo "$DEPLOY_OUT" | grep -oE '[0-9a-fA-F]{64}' | head -1)
[ -z "$PROGRAM_ID" ] && { echo "could not parse program_id from deploy output" >&2; exit 1; }

if [ "$OUTPUT_FORMAT" = "hex" ]; then
  printf '%s' "$PROGRAM_ID"
else
  echo "$DEPLOY_OUT"
  echo
  echo "program_id: $PROGRAM_ID"
fi
