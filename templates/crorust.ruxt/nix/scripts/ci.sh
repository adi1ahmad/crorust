#!/usr/bin/env bash
# =============================================================================
# LOCAL CI CHECK — Full 5-Layer Cross-Platform CI Automation Runner
# =============================================================================
set -euo pipefail
cd "$(dirname "$0")/../.."

echo "=== Layer 1: Code Formatting and Clippy Lints ==="
cargo fmt --all -- --check
cargo clippy --all-targets

echo "=== Layer 2: Fast Unit Tests ==="
SKIP_SNAPSHOTS=1 cargo test --lib

echo "=== Layer 3: Full Test Suite incl. Visual Snapshots (CI=1 Lavapipe) ==="
nix/scripts/check-snapshots.sh

echo "=== Layer 4: Cross-Target Build Smoke (WebAssembly & Android) ==="
cargo check --target wasm32-unknown-unknown --no-default-features --lib
cargo check --target aarch64-linux-android --lib

echo "=== Layer 5: Snapshot Artifact Sweep ==="
if [ -d tests/snapshots ] && find tests/snapshots -name '*.diff.png' -o -name '*.new.png' | grep -q .; then
    echo "ERROR: Untracked snapshot diffs found!"
    exit 1
fi

echo "=== CI Verification Passed Successfully (All 5 Layers Verified)! ==="
