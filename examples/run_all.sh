#!/usr/bin/env bash
# run_all.sh — run every example through the interpreter, bytecode VM, and typechecker.
#
# Usage:
#   ./run_all.sh [--interpreter] [--vm] [--typecheck] [-- <binary>]
#
# With no flags, all three backends are run.
# Interpreter failures cause a non-zero exit; VM and typechecker are informational.

set -uo pipefail

EXAMPLES_DIR="$(dirname "$0")"
BINARY="./target/debug/sapphire-lang"

run_interpreter=false
run_vm=false
run_typecheck=false
explicit=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --interpreter) run_interpreter=true; explicit=true; shift ;;
    --vm)          run_vm=true;          explicit=true; shift ;;
    --typecheck)   run_typecheck=true;   explicit=true; shift ;;
    --) shift; BINARY="${1:-$BINARY}"; shift ;;
    *)  BINARY="$1"; shift ;;
  esac
done

if [[ "$explicit" == false ]]; then
  run_interpreter=true
  run_vm=true
  run_typecheck=true
fi

if [[ ! -x "$BINARY" ]]; then
  echo "Binary not found: $BINARY"
  echo "Run 'cargo build' first, or pass the binary path as an argument."
  exit 1
fi

interp_pass=0; interp_fail=0
vm_pass=0;     vm_fail=0
tc_pass=0;     tc_fail=0

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

if [[ "$run_interpreter" == true ]]; then
  echo ""
  echo "── Interpreter (sapphire run) ─────────────────────────────────"
  for spr_file in "$EXAMPLES_DIR"/*.spr; do
    if run_example run "$spr_file"; then
      interp_pass=$((interp_pass + 1))
    else
      interp_fail=$((interp_fail + 1))
    fi
  done
fi

if [[ "$run_vm" == true ]]; then
  echo ""
  echo "── Bytecode VM (sapphire vm) ──────────────────────────────────"
  for spr_file in "$EXAMPLES_DIR"/*.spr; do
    if run_example vm "$spr_file"; then
      vm_pass=$((vm_pass + 1))
    else
      vm_fail=$((vm_fail + 1))
    fi
  done
fi

if [[ "$run_typecheck" == true ]]; then
  echo ""
  echo "── Typechecker (sapphire typecheck) ───────────────────────────"
  for spr_file in "$EXAMPLES_DIR"/*.spr; do
    if run_example typecheck "$spr_file"; then
      tc_pass=$((tc_pass + 1))
    else
      tc_fail=$((tc_fail + 1))
    fi
  done
fi

echo ""
[[ "$run_interpreter" == true ]] && echo "Interpreter: ${interp_pass} passed, ${interp_fail} failed"
[[ "$run_vm"          == true ]] && echo "VM:          ${vm_pass} passed, ${vm_fail} failed"
[[ "$run_typecheck"   == true ]] && echo "Typechecker: ${tc_pass} passed, ${tc_fail} failed"

exit_code=0
[[ "$run_interpreter" == false ]] || [[ $interp_fail -eq 0 ]] || exit_code=1
[[ "$run_vm"          == false ]] || [[ $vm_fail -eq 0 ]]     || exit_code=1
[[ "$run_typecheck"   == false ]] || [[ $tc_fail -eq 0 ]]     || exit_code=1
exit $exit_code
