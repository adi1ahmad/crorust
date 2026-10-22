#!/usr/bin/env bash
# =============================================================================
# COMMON SHELL UTILITIES - Modern UI Formatting & Multi-Host Helpers
# =============================================================================

# =============================================================================
# ANSI COLOR PALETTE
# =============================================================================
export RED='\033[0;31m'
export GREEN='\033[0;32m'
export YELLOW='\033[1;33m'
export BLUE='\033[0;34m'
export PURPLE='\033[0;35m'
export CYAN='\033[0;36m'
export WHITE='\033[1;37m'
export BOLD='\033[1m'
export DIM='\033[2m'
export RESET='\033[0m'

# =============================================================================
# SILENT MODE & HOST OS DETECTION
# =============================================================================
is_silent() {
    [[ "${NIX_SHELL_SILENT:-}" == "1" || "${NIX_SHELL_SILENT:-}" == "true" ]]
}

detect_host_os() {
    case "$(uname -s)" in
        Darwin*) echo "macos" ;;
        Linux*)
            if [[ -e /dev/dxg ]] || grep -qi "microsoft\|wsl" /proc/version 2>/dev/null; then
                echo "wsl2"
            elif [[ -e /etc/NIXOS ]] || [[ -d /etc/nixos ]]; then
                echo "nixos"
            else
                echo "linux"
            fi
            ;;
        *) echo "unknown" ;;
    esac
}

# =============================================================================
# MODERN UNICODE UI HELPERS
# =============================================================================

# Print a dynamically padded section header with rounded Unicode borders
# Usage: print_header "🔧" "HARDWARE DETECTION" "$BLUE"
print_header() {
    is_silent && return 0

    local emoji="$1" title="$2" color="${3:-$CYAN}"
    local inner_width=78
    # emoji (2 display cols) + 2 spaces + title + 1 leading space = title_len + 5
    local used_cols=$(( ${#title} + 5 ))
    local pad_len=$(( inner_width - used_cols ))
    [[ $pad_len -lt 1 ]] && pad_len=1
    local padding
    padding=$(printf '%*s' "$pad_len" '')

    echo -e "\n${color}╭──────────────────────────────────────────────────────────────────────────────╮${RESET}"
    echo -e "${color}│${RESET} ${emoji}  ${BOLD}${color}${title}${RESET}${padding}${color}│${RESET}"
    echo -e "${color}╰──────────────────────────────────────────────────────────────────────────────╯${RESET}"
}

# Print a structured status line
# Levels: success, warning, error, info, skip, test
print_status() {
    local level="$1" message="$2"

    if is_silent && [[ "$level" != "error" ]]; then
        return 0
    fi

    case "$level" in
        success) echo -e "  ${GREEN}✔${RESET} ${message}" ;;
        warning) echo -e "  ${YELLOW}⚠${RESET} ${message}" ;;
        error)   echo -e "  ${RED}✖ ${message}${RESET}" >&2 ;;
        info)    echo -e "  ${CYAN}ℹ${RESET} ${DIM}${message}${RESET}" ;;
        skip)    echo -e "  ${DIM}↷ ${message}${RESET}" ;;
        test)    echo -e "\n  ${CYAN}${BOLD}▸ Testing: ${message}${RESET}" ;;
    esac
}

# Print a diagnostic check result
# Status: pass, fail, warn, info, skip
print_result() {
    is_silent && return 0

    local status="$1" message="$2"
    case "$status" in
        pass) echo -e "  ${GREEN}✔ PASS${RESET}  ${message}" ;;
        fail) echo -e "  ${RED}✖ FAIL${RESET}  ${message}" ;;
        warn) echo -e "  ${YELLOW}⚠ WARN${RESET}  ${message}" ;;
        info) echo -e "  ${BLUE}ℹ INFO${RESET}  ${message}" ;;
        skip) echo -e "  ${DIM}↷ SKIP${RESET}  ${DIM}${message}${RESET}" ;;
    esac
}

# Run a diagnostic test command and report result
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_success="${3:-1}"

    if eval "$test_command" >/dev/null 2>&1; then
        if [[ "$expected_success" == "1" ]]; then
            print_result "pass" "$test_name"
            return 0
        else
            print_result "info" "$test_name (available)"
            return 0
        fi
    else
        if [[ "$expected_success" == "1" ]]; then
            print_result "fail" "$test_name (missing or failed)"
            return 1
        else
            print_result "skip" "$test_name (optional)"
            return 0
        fi
    fi
}

# =============================================================================
# CRORUST QUICK START GUIDE & DASHBOARD
# =============================================================================
print_quick_start_guide() {
    is_silent && return 0

    echo -e "${CYAN}╭──────────────────────────────────────────────────────────────────────────────╮${RESET}"
    echo -e "${CYAN}│${RESET} ${BOLD}${WHITE}🚀 CroRust Multi-Platform Workflows${RESET}    ${BOLD}${WHITE}🛠️  Desktop, Web & CI Commands${RESET}        ${CYAN}│${RESET}"
    echo -e "${CYAN}├──────────────────────────────────────────────────────────────────────────────┤${RESET}"
    echo -e "${CYAN}│${RESET}  ${GREEN}crorust run${RESET}        Run (Host/Mobile)   ${YELLOW}cargo run${RESET}            Run desktop app ${CYAN}│${RESET}"
    echo -e "${CYAN}│${RESET}  ${GREEN}crorust deploy${RESET}     Release package     ${YELLOW}trunk serve${RESET}          Serve Web (WASM)${CYAN}│${RESET}"
    echo -e "${CYAN}│${RESET}  ${GREEN}crorust build${RESET}      Build (--platform)  ${YELLOW}nix run .#check${RESET}      Typecheck all   ${CYAN}│${RESET}"
    echo -e "${CYAN}│${RESET}  ${GREEN}crorust devices${RESET}    Connected devices   ${YELLOW}nix run .#ci${RESET}         Full 5-layer CI ${CYAN}│${RESET}"
    echo -e "${CYAN}│${RESET}  ${GREEN}crorust i${RESET}          Toolchain health    ${PURPLE}graphics-setup check${RESET} GPU diagnostics ${CYAN}│${RESET}"
    echo -e "${CYAN}│${RESET}  ${GREEN}crorust cache info${RESET} Inspect SDK cache   ${PURPLE}nix develop .#debug${RESET}  RenderDoc shell ${CYAN}│${RESET}"
    echo -e "${CYAN}╰──────────────────────────────────────────────────────────────────────────────╯${RESET}"
}

# =============================================================================
# GRAPHICS-SETUP CLI HELP
# =============================================================================
show_help() {
    echo -e "${BOLD}${CYAN}Graphics & Multi-Host Diagnostics Utility (graphics-setup)${RESET}"
    echo ""
    echo -e "${BOLD}Usage:${RESET} graphics-setup <MODE> [OPTIONS]"
    echo ""
    echo -e "${BOLD}Modes:${RESET}"
    echo -e "  ${GREEN}detect${RESET}      Auto-detect GPU (Intel/AMD/NVIDIA/WSL2/macOS) and emit shell exports"
    echo -e "  ${GREEN}check${RESET}       Quick summary of GPU, Vulkan, OpenGL/Metal, and display server"
    echo -e "  ${GREEN}diagnose${RESET}    Full hardware, driver, shader, and cross-toolchain diagnostic suite"
    echo -e "  ${GREEN}configure${RESET}   Apply detected GPU environment variables in current process"
    echo ""
    echo -e "${BOLD}Options:${RESET}"
    echo -e "  ${YELLOW}--performance, -p${RESET}   Include shader cache and rendering benchmarks (diagnose mode)"
    echo -e "  ${YELLOW}--help, -h${RESET}          Display this help message"
    echo ""
    echo -e "${BOLD}Examples:${RESET}"
    echo -e "  eval \"\$(graphics-setup detect)\"        # Configure GPU drivers in current shell"
    echo -e "  graphics-setup check                   # Fast GPU & Vulkan readiness check"
    echo -e "  graphics-setup diagnose --performance  # Full diagnostic audit"
}
