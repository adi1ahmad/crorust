#!/usr/bin/env bash
# =============================================================================
# CRORUST.RUXT DEVELOPMENT ENVIRONMENT — SHELL HOOK
# =============================================================================
# Note: common.sh is concatenated before this file by flake.nix

# 1. Project-local Mesa shader cache (crorust manages .cache/crorust natively)
PROJECT_ROOT="${PWD}"
CACHE_DIR="${PROJECT_ROOT}/.cache"
mkdir -p "${CACHE_DIR}/mesa" "${CACHE_DIR}/crorust" 2>/dev/null || true
export MESA_GLSL_CACHE_DIR="${CACHE_DIR}/mesa"

# Preserve preexisting host library search paths if present in the caller shell
if [[ -n "${__HOST_ORIGINAL_LD_LIBRARY_PATH:-}" ]]; then
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:${__HOST_ORIGINAL_LD_LIBRARY_PATH}"
fi

# 2. Prepend Smart LLVM 19 Bridge (`clang`, `clang++`, `clang-cl`) to PATH
#    so cross-targets (Android, WASM, iOS, Windows) use unwrapped Clang with
#    -resource-dir while host builds use Nix's wrapped Clang.
if [[ -n "${CRORUST_LLVM_BRIDGE_BIN:-}" ]]; then
    export PATH="${CRORUST_LLVM_BRIDGE_BIN}:${PATH}"
fi

# 3. Auto-detect GPU & apply driver overrides (Intel / AMD / NVIDIA / WSL2 / macOS)
if command -v graphics-setup >/dev/null 2>&1; then
    eval "$(graphics-setup detect 2>/dev/null)" || true
fi

# 4. Render interactive dashboard (skipped automatically when NIX_SHELL_SILENT=1)
if ! is_silent; then
    _host_os="$(uname -s) $(uname -r | cut -d'-' -f1) ($(uname -m))"
    if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
        _display="Wayland"
    elif [[ -n "${DISPLAY:-}" ]]; then
        _display="X11"
    elif [[ "$(uname -s)" == "Darwin" ]]; then
        _display="Quartz/Metal"
    else
        _display="Headless/CI"
    fi

    _rustc_ver=$(rustc --version 2>/dev/null | awk '{print $2}' || echo "missing")
    _crorust_ver=$(crorust --version 2>/dev/null | awk '{print $2}' || echo "ready")
    _kotlinc_ver=$(kotlinc -version 2>&1 | awk '{print $3}' | head -1 || echo "ready")
    _gpu_label="${GPU_SUMMARY:-Auto-detected}"
    # Truncate GPU label to 44 chars so table borders stay strictly aligned
    if [[ ${#_gpu_label} -gt 44 ]]; then
        _gpu_label="${_gpu_label:0:41}..."
    fi

    echo ""
    echo -e "${CYAN}╭──────────────────────────────────────────────────────────────────────────────╮${RESET}"
    echo -e "${CYAN}│${RESET} 🦀 ${BOLD}${WHITE}crorust.ruxt${RESET}  ${DIM}·  Multi-Host Cross-Platform Rust Development Environment${RESET}   ${CYAN}│${RESET}"
    echo -e "${CYAN}├──────────────────────────────────────────────────────────────────────────────┤${RESET}"
    printf "${CYAN}│${RESET} ${BOLD}Host OS${RESET}   %-44s ${BOLD}Display${RESET}   %-11s ${CYAN}│${RESET}\n" "$_host_os" "$_display"
    printf "${CYAN}│${RESET} ${BOLD}GPU${RESET}       %-44s ${BOLD}Vulkan${RESET}    %-11s ${CYAN}│${RESET}\n" "$_gpu_label" "${VULKAN_SUPPORTED:+Ready [OK]}"
    printf "${CYAN}│${RESET} ${BOLD}Rust${RESET}      rustc %-38s ${BOLD}crorust${RESET}   v%-10s ${CYAN}│${RESET}\n" "$_rustc_ver" "$_crorust_ver"
    printf "${CYAN}│${RESET} ${BOLD}Android${RESET}   API 35 · JDK 17 · kotlinc %-18s ${BOLD}WebASM${RESET}    %-11s ${CYAN}│${RESET}\n" "$_kotlinc_ver" "trunk [OK]"
    echo -e "${CYAN}╰──────────────────────────────────────────────────────────────────────────────╯${RESET}"

    print_quick_start_guide
    echo -e "  ${DIM}Workspace: ${PWD##*/}  ·  Cache: .cache/crorust  ·  Quiet mode: NIX_SHELL_SILENT=1${RESET}\n"

    unset _host_os _display _rustc_ver _crorust_ver _kotlinc_ver _gpu_label
fi