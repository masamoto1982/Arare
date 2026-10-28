#!/usr/bin/env bash
# Rebuild the wasm bundle that ships in src/wasm/generated/.
#
# wasm-pack writes a publishable npm package (with package.json, README, etc.)
# into its --out-dir, but the runtime only consumes four files:
#   ajisai_core.js, ajisai_core.d.ts,
#   ajisai_core_bg.wasm, ajisai_core_bg.wasm.d.ts
#
# We build into a scratch directory and copy just those four files so the
# committed tree stays clean.
#
# wasm-opt runs by default, from a pinned Binaryen (BINARYEN_VERSION below),
# never from whatever wasm-pack downloads. wasm-opt 108 (the version wasm-pack
# fetched in an earlier build environment) miscompiled wasm-bindgen output and
# produced a corrupted module (see commit 89a0c7b), so the optimized module is
# not trusted on the optimizer's word: wasm-pack builds unoptimized, the pinned
# wasm-opt rewrites a copy, and scripts/bench/wasm-parity.mjs runs ~11k
# programs through both and requires identical envelopes (stack, errors,
# traces, meters) before the optimized copy is installed. A mismatch fails the
# build. Measured with Binaryen 132: ~18% smaller module, 5-15% faster on the
# interpreter-bound cases of scripts/bench/speed-bench-cases.json.
#
# Binaryen comes from npm (the `binaryen` package ships wasm-opt as a Node
# program, so no platform binary is needed): node_modules/.bin/wasm-opt when it
# is the pinned version, else `npx -p binaryen@<version>`. It is not a
# devDependency because it is ~100 MB and only this script needs it.
# AJISAI_WASM_OPT=0 skips the optimizer.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
out_dir="${repo_root}/src/wasm/generated"
scratch_dir="$(mktemp -d)"
trap 'rm -rf "${scratch_dir}"' EXIT

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "rebuild-wasm: wasm-pack is not installed." >&2
  echo "  Install with:  bash ./scripts/install-wasm-pack.sh" >&2
  exit 1
fi

cd "${repo_root}/rust"
# The WASM/JS bindings are gated behind the `wasm` Cargo feature so the native
# Core build never pulls in wasm-bindgen. wasm-pack passes args after `--`
# straight to cargo.
BINARYEN_VERSION="132.0.0"
wasm-pack build --target web --out-dir "${scratch_dir}" --no-opt -- --features wasm

if [[ "${AJISAI_WASM_OPT:-1}" == "1" ]]; then
  local_wasm_opt="${repo_root}/node_modules/.bin/wasm-opt"
  if [[ -x "${local_wasm_opt}" ]] \
    && [[ "$("${local_wasm_opt}" --version)" == *"version_${BINARYEN_VERSION%%.*}"* ]]; then
    wasm_opt=("${local_wasm_opt}")
  else
    wasm_opt=(npx --yes -p "binaryen@${BINARYEN_VERSION}" wasm-opt)
  fi
  echo "rebuild-wasm: optimizing with $("${wasm_opt[@]}" --version)."
  optimized_dir="$(mktemp -d)"
  trap 'rm -rf "${scratch_dir}" "${optimized_dir}"' EXIT
  cp "${scratch_dir}"/ajisai_core* "${optimized_dir}/"
  # The feature flags are the ones rustc's wasm32 target emits by default;
  # wasm-opt must be told, or it rejects the module.
  "${wasm_opt[@]}" -O3 \
    --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "${scratch_dir}/ajisai_core_bg.wasm" -o "${optimized_dir}/ajisai_core_bg.wasm"
  node "${repo_root}/scripts/bench/wasm-parity.mjs" "${scratch_dir}" "${optimized_dir}"
  cp "${optimized_dir}/ajisai_core_bg.wasm" "${scratch_dir}/ajisai_core_bg.wasm"
else
  echo "rebuild-wasm: AJISAI_WASM_OPT=0; shipping the unoptimized module."
fi

mkdir -p "${out_dir}"
for f in ajisai_core.js ajisai_core.d.ts ajisai_core_bg.wasm ajisai_core_bg.wasm.d.ts; do
  cp "${scratch_dir}/${f}" "${out_dir}/${f}"
done

echo "rebuild-wasm: refreshed ${out_dir}"
