#!/bin/bash
echo "=== Verification Script ==="
echo ""
echo "Bun version:"
bun --version
echo ""
echo "Building Tauri frontend:"
bun run build 2>&1 | tail -3
echo ""
echo "Building web frontend:"
cd src/web-frontend && bun run build 2>&1 | tail -3
echo ""
echo "Checking Rust:"
cd ../.. && cd src-tauri && cargo check 2>&1 | tail -3
echo ""
echo "=== All verifications complete ==="
