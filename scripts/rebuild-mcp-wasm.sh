#!/usr/bin/env bash
# Rebuild the Node worker_threads WASM bundle for the MCP server's WASM
# backend (tools/mcp-server/wasm/generated/).
#
# Mirrors rebuild-wasm.sh (the browser `--target web` build) but targets
# `nodejs`: the generated module synchronously reads and instantiates its own
# `.wasm` file with plain `fs`/`require`, no bundler and no `fetch`, which is
# what a worker_threads Worker needs.
#
# wasm-pack writes a publishable npm package (with package.json, README, etc.)
# into its --out-dir, but the runtime only consumes four files:
#   ajisai_core.js, ajisai_core.d.ts,
#   ajisai_core_bg.wasm, ajisai_core_bg.wasm.d.ts
#
# We build into a scratch directory and copy just those four files so the
# committed tree stays clean.
#
# wasm-opt stays opt-in for the same reason as rebuild-wasm.sh: a pinned,
# smoke-tested Binaryen version is still outstanding. Set AJISAI_WASM_OPT=1 to
# exercise the speed-optimized profile.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
out_dir="${repo_root}/tools/mcp-server/wasm/generated"
scratch_dir="$(mktemp -d)"
trap 'rm -rf "${scratch_dir}"' EXIT

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "rebuild-mcp-wasm: wasm-pack is not installed." >&2
  echo "  Install with:  bash ./scripts/install-wasm-pack.sh" >&2
  exit 1
fi

cd "${repo_root}/rust"
wasm_pack_args=(build --target nodejs --out-dir "${scratch_dir}")
if [[ "${AJISAI_WASM_OPT:-0}" == "1" ]]; then
  if command -v wasm-opt >/dev/null 2>&1; then
    echo "rebuild-mcp-wasm: AJISAI_WASM_OPT=1; using wasm-pack release wasm-opt profile ($(wasm-opt --version))."
  else
    echo "rebuild-mcp-wasm: AJISAI_WASM_OPT=1 but wasm-opt is not on PATH; wasm-pack may fail." >&2
  fi
else
  echo "rebuild-mcp-wasm: using stable no-opt wasm build; set AJISAI_WASM_OPT=1 to enable wasm-opt -O3."
  wasm_pack_args+=(--no-opt)
fi
wasm-pack "${wasm_pack_args[@]}" -- --features wasm

mkdir -p "${out_dir}"
for f in ajisai_core.js ajisai_core.d.ts ajisai_core_bg.wasm ajisai_core_bg.wasm.d.ts; do
  cp "${scratch_dir}/${f}" "${out_dir}/${f}"
done

# The wasm-pack `nodejs` target emits CommonJS (`require`, `__dirname`), but
# tools/mcp-server/package.json declares "type": "module" for the rest of the
# server. Without an override, Node classifies this plain `.js` file as ESM by
# the nearest-package.json rule, which silently breaks `__dirname` (Node's
# CJS-in-ESM interop shim resolves it to "." instead of the real directory,
# so the bundle fails to find its own `.wasm` file). A directory-local
# `type: commonjs` override fixes classification for every loader
# (`require`, `createRequire`, and Node's `require(esm)` interop) without
# touching the server's own ESM sources.
cat > "${out_dir}/package.json" <<'EOF'
{ "type": "commonjs" }
EOF

echo "rebuild-mcp-wasm: refreshed ${out_dir}"
