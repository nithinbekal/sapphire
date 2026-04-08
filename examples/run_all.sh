#!/usr/bin/env bash
# run_all.sh — run every example through the tree-walk interpreter (required)
# and the bytecode VM (informational).  Exits 1 if any interpreter run fails.

set -uo pipefail

BINARY="${1:-./target/debug/sapphire}"
EXAMPLES_DIR="$(dirname "$0")"

if [[ ! -x "$BINARY" ]]; then
  echo "Binary not found: $BINARY"
  echo "Run 'cargo build' first, or pass the binary path as the first argument."
  exit 1
fi

pass=0
fail=0
vm_pass=0
vm_fail=0

run_example() {
  local backend="$1"
  local file="$2"
  local label
  label="$(basename "$file") [$backend]"

  if "$BINARY" "$backend" "$file" > /dev/null 2>&1; then
    echo "  PASS  $label"
    return 0
  else
    echo "  FAIL  $label"
    return 1
  fi
}

echo "=== Sapphire example runner ==="
echo "Binary: $BINARY"

echo ""
echo "── Interpreter (sapphire run) ─────────────────────────────────"
for spr_file in "$EXAMPLES_DIR"/*.spr; do
  if run_example run "$spr_file"; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
  fi
done

echo ""
echo "── Bytecode VM (sapphire vm) — informational ──────────────────"
for spr_file in "$EXAMPLES_DIR"/*.spr; do
  if run_example vm "$spr_file"; then
    vm_pass=$((vm_pass + 1))
  else
    vm_fail=$((vm_fail + 1))
  fi
done

echo ""
echo "Interpreter: ${pass} passed, ${fail} failed"
echo "VM:          ${vm_pass} passed, ${vm_fail} failed (not required)"
[[ $fail -eq 0 ]]
