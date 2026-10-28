#!/bin/bash
# Install the root toolchain (lint, vitest, npm run check:*), the Ajisai MCP
# server's dependencies, so the `ajisai` entry in .mcp.json can start, and the
# lexicon-emergence harness's. The server's WASM backend is committed, so npm
# is the only step for it.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR"
npm install --no-audit --no-fund

cd "$CLAUDE_PROJECT_DIR/tools/mcp-server"
npm install --no-audit --no-fund
node index.js --doctor

# The lexicon-emergence harness (route B) calls the Claude API through the SDK.
cd "$CLAUDE_PROJECT_DIR/tools/lexicon-emergence"
npm install --no-audit --no-fund
