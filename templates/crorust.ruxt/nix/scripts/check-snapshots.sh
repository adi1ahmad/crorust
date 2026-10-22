#!/usr/bin/env bash
# =============================================================================
# SNAPSHOT CI CHECK — Tier-2 egui_kittest snapshot verification
# =============================================================================
# Runs the full snapshot suite in CI mode (asserts a CPU/software wgpu
# adapter) with `--test-threads 1` as belt-and-suspenders on top of the
# `#[serial_test::serial]` markers, then FAILS if any `.diff.png` artifact
# was produced (i.e. a snapshot drifted).
# Usage:
#   check-snapshots.sh            # full snapshot verification (gating)
#   check-snapshots.sh <filter>   # verify one snapshot area, e.g. `shell`
#   SKIP_SNAPSHOTS=1 check-snapshots.sh   # skip (no GPU machines)
#
# Exit codes:
#   0  — all snapshots matched (or were skipped)
#   1  — a snapshot drifted / build failed / diff artifacts found
# =============================================================================

set -euo pipefail

# No adapter available? Honor the skip guard.
if [[ -n "${SKIP_SNAPSHOTS:-}" ]]; then
    echo "SKIP_SNAPSHOTS is set — skipping snapshot verification."
    exit 0
fi

cd "$(dirname "$0")/../.." # project root (nix/scripts → root)

SNAP_DIR="tests/snapshots"

# ── Run the snapshot suite in CI mode, serialized ────────────────────────────
# CI=1 expects a software (CPU) wgpu adapter, so lavapipe/llvmpipe must be
# installed. This keeps snapshots identical across machines.
# --test-threads 1 serializes the whole run as a backup to #[serial_test].
FILTER="${1:-}"
echo "Running snapshot suite (CI=1, serial)..."
if [[ -n "$FILTER" ]]; then
    CI=1 cargo test "$FILTER" -- --test-threads 1
else
    CI=1 cargo test -- --test-threads 1
fi

# ── Fail on drift artifacts ──────────────────────────────────────────────────
# kittest writes {name}.new.png / {name}.diff.png on mismatch (and
# {name}.old.png under UPDATE_SNAPSHOTS). Committed snapshots must not
# coexist with these.
if [[ -d "$SNAP_DIR" ]]; then
    if find "$SNAP_DIR" \( -name '*.new.png' -o -name '*.diff.png' \) -print -quit | grep -q .; then
        echo
        echo "ERROR: snapshot drift detected — .new.png/.diff.png artifacts found:"
        find "$SNAP_DIR" \( -name '*.new.png' -o -name '*.diff.png' \) -print
        echo
        echo "Review the .diff.png files, then regenerate with intent:"
        echo "  UPDATE_SNAPSHOTS=1 cargo test <name>"
        echo "and commit the regenerated .png (never the .new/.diff/.old artifacts)."
        exit 1
    fi
fi

echo "Snapshot verification passed — no drift artifacts."
