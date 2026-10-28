#!/bin/bash
# Install the root toolchain (lint, vitest, npm run check:*) and the
# dependencies of the Ajisai MCP server kept as the oracle (tools/mcp-server/,
# the `ajisai` entry in .mcp.json). The server's WASM backend is committed, so
# npm is the only step for it.
#
# Lean (formal/) and Kani are not installed here: both download large
# toolchains, and docs/quality/FORMAL_VERIFICATION.md says how to install them
# when a session needs them.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR"
npm install --no-audit --no-fund

cd "$CLAUDE_PROJECT_DIR/tools/mcp-server"
npm install --no-audit --no-fund
node index.js --doctor
