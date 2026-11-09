#!/usr/bin/env bash
# ==============================================================================
# scripts/build_manual_diagrams.sh
# High-Performance, Incremental Mermaid-to-SVG Compiler for Ruxt Manual
#
# Supports:
#   --check     Verify that all committed SVGs are fresh and valid (for CI)
#   --force     Force recompilation of all diagrams regardless of hash cache
#   --clean     Remove compiled SVGs and hash cache
# ==============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIAGRAMS_DIR="$ROOT_DIR/templates/crorust.ruxt/assets/manual/diagrams"
OUTPUT_DIR="$ROOT_DIR/templates/crorust.ruxt/assets/manual/svg"
CACHE_DIR="$ROOT_DIR/.cache/diagrams"

CHECK_MODE=false
FORCE_MODE=false

for arg in "$@"; do
    case "$arg" in
        --check) CHECK_MODE=true ;;
        --force) FORCE_MODE=true ;;
        --clean)
            echo "Cleaning SVG output and cache..."
            rm -rf "$OUTPUT_DIR"/*.svg "$CACHE_DIR"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg"
            echo "Usage: $0 [--check | --force | --clean]"
            exit 1
            ;;
    esac
done

mkdir -p "$OUTPUT_DIR" "$CACHE_DIR"

# 1. Verify Mermaid CLI Availability
MMDC_CMD=""
if command -v mmdc >/dev/null 2>&1; then
    MMDC_CMD="mmdc"
elif command -v npx >/dev/null 2>&1; then
    MMDC_CMD="npx -y @mermaid-js/mermaid-cli"
else
    if [ "$CHECK_MODE" = true ]; then
        echo "Error: Node.js / npx / mmdc not found, cannot verify in CI mode!"
        exit 1
    fi
    echo "⚠️  Warning: Neither 'mmdc' nor 'npx' was found."
    echo "   Using pre-existing compiled SVGs in $OUTPUT_DIR without rebuilding."
    exit 0
fi

echo "=== Ruxt Manual Vector Diagram Pipeline ==="
echo "Source: $DIAGRAMS_DIR"
echo "Target: $OUTPUT_DIR"
echo "Tool:   $MMDC_CMD"
echo ""

STALE_COUNT=0
COMPILED_COUNT=0

CHROME_BIN="$(command -v chromium 2>/dev/null || command -v google-chrome 2>/dev/null || command -v google-chrome-stable 2>/dev/null || command -v chrome 2>/dev/null || true)"

PUPPETEER_CFG="$CACHE_DIR/puppeteer-config.json"
if [ -n "$CHROME_BIN" ]; then
cat << EOF > "$PUPPETEER_CFG"
{
  "executablePath": "$CHROME_BIN",
  "args": ["--no-sandbox", "--disable-setuid-sandbox", "--disable-dev-shm-usage"]
}
EOF
else
cat << 'EOF' > "$PUPPETEER_CFG"
{
  "args": ["--no-sandbox", "--disable-setuid-sandbox", "--disable-dev-shm-usage"]
}
EOF
fi

MERMAID_CFG="$CACHE_DIR/mermaid-config.json"
cat << 'EOF' > "$MERMAID_CFG"
{
  "theme": "dark",
  "htmlLabels": false,
  "flowchart": {
    "htmlLabels": false,
    "useMaxWidth": false,
    "curve": "basis"
  },
  "sequence": {
    "useMaxWidth": false
  },
  "themeVariables": {
    "darkMode": true,
    "background": "transparent",
    "fontFamily": "Inter, system-ui, sans-serif",
    "fontSize": "18px",
    "textColor": "#FFFFFF",
    "primaryTextColor": "#FFFFFF",
    "nodeTextColor": "#FFFFFF",
    "primaryColor": "#1E293B",
    "primaryBorderColor": "#60A5FA",
    "lineColor": "#38BDF8",
    "secondaryColor": "#1E3A8A",
    "tertiaryColor": "#0F172A",
    "clusterBkg": "#0B132B",
    "clusterBorder": "#475569",
    "edgeLabelBackground": "#1E293B"
  }
}
EOF

for mmd in "$DIAGRAMS_DIR"/*.mmd; do
    [ -f "$mmd" ] || continue
    name="$(basename "$mmd" .mmd)"
    svg="$OUTPUT_DIR/$name.svg"
    hash_file="$CACHE_DIR/$name.sha256"

    current_hash="$(sha256sum "$mmd" | cut -d' ' -f1)"
    cached_hash=""
    if [ -f "$hash_file" ]; then
        cached_hash="$(cat "$hash_file")"
    fi

    if [ "$FORCE_MODE" = false ] && [ -f "$svg" ] && [ "$current_hash" = "$cached_hash" ]; then
        continue
    fi

    if [ "$CHECK_MODE" = true ]; then
        echo "❌ [STALE] $name.svg is missing or out-of-date compared to $name.mmd"
        STALE_COUNT=$((STALE_COUNT + 1))
        continue
    fi

    echo "⚙️  Compiling $name.mmd -> $name.svg..."
    $MMDC_CMD \
        -i "$mmd" \
        -o "$svg" \
        -b transparent \
        -p "$PUPPETEER_CFG" \
        -c "$MERMAID_CFG" \
        --quiet

    # Strict SVG XML Integrity Check
    if [ ! -s "$svg" ]; then
        echo "❌ Compilation failed: $svg is empty!"
        exit 1
    fi

    if ! grep -q "<svg" "$svg" || ! grep -q "</svg>" "$svg"; then
        echo "❌ Compilation failed: $svg does not contain valid <svg> root element!"
        exit 1
    fi

    if grep -q "Syntax error in graph" "$svg"; then
        echo "❌ Mermaid syntax error detected inside $svg!"
        exit 1
    fi

    # Sanitize SVG for strict simplecss / resvg compatibility
    python3 "$ROOT_DIR/scripts/sanitize_svg.py" "$svg" >/dev/null

    # Reject unsupported CSS rules
    if grep -q "@keyframes" "$svg"; then
        echo "❌ Rejection: $svg contains @keyframes CSS rules which cannot be rendered by simplecss!"
        exit 1
    fi
    if grep -q ":root" "$svg"; then
        echo "❌ Rejection: $svg contains :root CSS selectors which cannot be parsed by simplecss!"
        exit 1
    fi

    # Reject HTML <foreignObject> elements (unsupported by resvg)
    if grep -q "<foreignObject" "$svg"; then
        echo "❌ Rejection: $svg contains <foreignObject> HTML elements which cannot be rendered by resvg!"
        exit 1
    fi

    # Assert presence of native SVG <text> elements
    if ! grep -q "<text" "$svg"; then
        echo "❌ Rejection: $svg does not contain any native <text> elements!"
        exit 1
    fi

    # Assert all <rect> elements have valid width attributes
    if grep -E '<rect[ >]' "$svg" | grep -v 'width=' | grep -q .; then
        echo "❌ Rejection: $svg contains <rect> tags lacking a valid width attribute!"
        exit 1
    fi

    echo "$current_hash" > "$hash_file"
    COMPILED_COUNT=$((COMPILED_COUNT + 1))
    echo "✓ Generated $name.svg ($(wc -c < "$svg") bytes)"
done

if [ "$CHECK_MODE" = true ]; then
    if [ "$STALE_COUNT" -gt 0 ]; then
        echo "❌ CI Check Failed: $STALE_COUNT diagrams need recompilation. Run 'bash scripts/build_manual_diagrams.sh'."
        exit 1
    else
        echo "✅ CI Check Passed: All diagrams are up to date and valid."
        exit 0
    fi
fi

if [ "$COMPILED_COUNT" -eq 0 ]; then
    echo "✨ All diagrams are fresh (cache valid). Nothing to do."
else
    echo "✅ Successfully compiled $COMPILED_COUNT diagram(s)."
fi
