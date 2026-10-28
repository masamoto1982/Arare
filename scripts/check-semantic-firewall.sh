#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Every check runs through ripgrep. Without it, each `if rg ...; then FAIL`
# condition is a "command not found" (non-zero), the FAIL branch never runs, and
# the whole firewall would pass VACUOUSLY without scanning anything. Fail loudly
# instead so a runner missing ripgrep is a visible error, never a silent pass.
if ! command -v rg >/dev/null 2>&1; then
  echo "[semantic-firewall] ERROR: ripgrep (rg) is required but not installed;" >&2
  echo "[semantic-firewall] without it these checks would pass without scanning." >&2
  exit 1
fi

failed=0

# rg exits 0 on a match, 1 on none, and 2 on an error such as a missing path
# (even when it also matched elsewhere). `if rg` would read 2 as "no match"
# and pass without having scanned the missing file, so every status is
# classified explicitly and anything but 0 or 1 aborts the check.
classify_rg_status() {
  local description="$1"
  local status="$2"
  case "$status" in
    0)
      echo "[semantic-firewall] FAIL: ${description}" >&2
      failed=1
      ;;
    1) ;;
    *)
      echo "[semantic-firewall] ERROR: rg exited ${status} while checking ${description}" >&2
      exit 2
      ;;
  esac
}

check_absent() {
  local description="$1"
  local pattern="$2"
  shift 2
  echo "[semantic-firewall] checking: ${description}"
  local status=0
  rg -n --color never "$pattern" "$@" || status=$?
  classify_rg_status "$description" "$status"
}

# External payloads must not expose disallowed camelCase fields.
check_absent "nilReason external field" 'nilReason' rust/src src
check_absent "top-level errorCategory external field" 'errorCategory' rust/src src

# Machine-readable WASM/TS/AI-facing outputs must use protocol strings,
# not Rust Debug formatting.
check_absent \
  'Debug formatting in external protocol payload code' \
  'format!\("\{:\?' \
  rust/src/wasm_interpreter_bindings/wasm_interpreter_state.rs src

# TypeScript and the WASM boundary must not depend on Rust Debug variant names.
check_absent \
  'Rust Debug-derived protocol literals in TS/WASM boundary' \
  'DivisionByZero|SafeCaught|ExecuteWord|ParseStructure|ResolveWord' \
  src rust/src/wasm_interpreter_bindings/wasm_interpreter_state.rs

# ── Internal-vocabulary firewall over user-visible strings ────────────────
# Two-tier disclosure: language users see optimizations only as speed, so
# error text, NIL diagnostics, LOOKUP/hover content, and GUI labels must
# never name a routing or optimization mechanism. Builder/AI channels are
# exempt:
#   - tests and benches (globs below)
#   - rust/src/cli/                agent-facing --json contract / explain
#   - rust/src/agent/              host-neutral agent boundary (same --json
#                                   contract as cli/, shared with the WASM
#                                   one-shot entry point)
#   - wasm_interpreter_bindings/   machine protocol keys (metric names)
#   - trace eprintln ("[trace-*", "[hedged]"), panics/expect, cfg attrs
# The pattern is applied to string literals only, so code comments stay free
# to name mechanisms for builders.
check_user_visible_absent() {
  local description="$1"
  local pattern="$2"
  echo "[semantic-firewall] checking: user-visible ${description}"
  local statuses
  set +e
  rg -n --color never "\"[^\"]*(${pattern})[^\"]*\"" \
      rust/src src \
      -g '!*test*' -g '!**/elastic/**' -g '!**/cli/**' -g '!**/agent/**' \
      -g '!**/wasm_interpreter_bindings/**' -g '!**/benches/**' \
      -g '!**/route_equivalence.rs' -g '!src/wasm/generated/**' \
    | rg -v '\.expect\(|eprintln!|\[trace-|debug_assert|panic!|#\[cfg\(feature'
  statuses=("${PIPESTATUS[@]}")
  set -e
  # The scan itself erroring is fatal; it finding nothing (1) is fine. The
  # filter's status then decides: 0 means a line survived it.
  if [[ "${statuses[0]}" -ge 2 ]]; then
    classify_rg_status "user-visible ${description}" "${statuses[0]}"
  fi
  classify_rg_status "user-visible ${description}" "${statuses[1]}"
}

check_user_visible_absent 'fast-kernel vocabulary' '[Ff]ast kernel'
check_user_visible_absent 'fast-path vocabulary' '[Ff]ast.?[Pp]ath'
check_user_visible_absent 'quantized-block vocabulary' '[Qq]uantized block'
check_user_visible_absent 'compiled-plan vocabulary' '[Cc]ompiled plan'
check_user_visible_absent 'inline-cache vocabulary' '[Ii]nline cache|shape IC'
check_user_visible_absent 'hedged-engine vocabulary' '[Hh]edged'
check_user_visible_absent 'epoch vocabulary' '[Ee]poch'
check_user_visible_absent 'internal roadmap phase numbers' '[Pp]hase [0-9]'
check_user_visible_absent 'memoization vocabulary' '[Mm]emoiz'

# ── Reading surfaces describe the present language ────────────────────────
# LANG.AUTHORITY.PRESENT: the README, the Reference, and the Specification
# describe Ajisai as it currently is. History and superseded designs live in
# docs/dev/, which defines nothing. This catches the framing half of the
# clause; check-reading-surfaces.mjs catches the vocabulary half.
READING_SURFACES=(README.md public/docs/index.html public/docs/en/index.html public/docs/ja/index.html SPECIFICATION.html spec/language-semantics.md spec/gui-semantics.md)

check_absent \
  'history framing on a reading surface (LANG.AUTHORITY.PRESENT)' \
  '\b([Nn]o longer|[Uu]sed to be|[Ff]ormerly|[Pp]reviously|[Dd]eprecated|[Ll]egacy|[Ee]arlier versions?|[Oo]nce (had|carried|supported)|has been removed|was removed|reserved for future)\b' \
  "${READING_SURFACES[@]}"

if [[ "$failed" -ne 0 ]]; then
  echo "[semantic-firewall] residue checks failed" >&2
  exit 1
fi

echo "[semantic-firewall] residue checks passed"
