#!/usr/bin/env bash
# ==============================================================================
# crorust: Local CI & Multi-Platform Matrix Dogfood Test Runner
#
# Faithfully mimics GitHub Actions CI (.github/workflows/ci.yml and
# .github/actions/dogfood-build) on the local workstation host.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${SCRIPT_DIR}"

# ANSI Color Codes
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET="\033[0m"
    C_BOLD="\033[1m"
    C_DIM="\033[2m"
    C_RED="\033[31m"
    C_GREEN="\033[32m"
    C_YELLOW="\033[33m"
    C_BLUE="\033[34m"
    C_CYAN="\033[36m"
else
    C_RESET=""
    C_BOLD=""
    C_DIM=""
    C_RED=""
    C_GREEN=""
    C_YELLOW=""
    C_BLUE=""
    C_CYAN=""
fi

# Default Configuration
DEFAULT_CACHE_DIR="${WORKSPACE_ROOT}/.cache/crorust-tools"
CACHE_DIR="${CRORUST_CACHE_DIR:-$DEFAULT_CACHE_DIR}"
STAGE_DIR=""
LOG_BASE_DIR="${WORKSPACE_ROOT}/.ci-logs/local"
RUN_TIMESTAMP="$(date +'%Y%m%d-%H%M%S')"
RUN_LOG_DIR="${LOG_BASE_DIR}/run-${RUN_TIMESTAMP}"
FAIL_FAST=true
VERBOSE=false
DOCTOR_MODE=false
FIX_DEPS=false
CLEAN_FIRST=false

# Tier Selection Defaults
RUN_TIER1=true
RUN_TIER2=true
RUN_TIER3=true
RUN_TIER4=true
CUSTOM_TIERS=false

# Tier 1 Granular Flags
SKIP_FMT=false
SKIP_CLIPPY=false
SKIP_TESTS=false

# Tier 4 Platform Filters (empty means all supported platforms for host)
SELECTED_PLATFORMS=()

# Tracking & Scorecard
SCORECARD_ENTRIES=() # Tuple format: TIER|PHASE_NAME|STATUS|DURATION|DETAILS
STEP_COUNTER=0
OVERALL_SUCCESS=true
START_TIME_TOTAL=$(date +%s)
CLEANUP_DIRS=()

cleanup() {
    local exit_code=$?
    for dir in "${CLEANUP_DIRS[@]:-}"; do
        if [ -n "$dir" ] && [ -d "$dir" ]; then
            rm -rf "$dir" 2>/dev/null || true
        fi
    done
    if [ "$DOCTOR_MODE" = "false" ] && [ ${#SCORECARD_ENTRIES[@]} -gt 0 ]; then
        render_tier5_scorecard
    fi
    exit $exit_code
}
trap cleanup EXIT INT TERM

log_info() {
    echo -e "${C_BLUE}${C_BOLD}[INFO]${C_RESET} $*"
}

log_pass() {
    echo -e "${C_GREEN}${C_BOLD}[PASS]${C_RESET} $*"
}

log_warn() {
    echo -e "${C_YELLOW}${C_BOLD}[WARN]${C_RESET} $*"
}

log_error() {
    echo -e "${C_RED}${C_BOLD}[ERROR]${C_RESET} $*" >&2
}

register_cleanup_dir() {
    CLEANUP_DIRS+=("$1")
}

# ------------------------------------------------------------------------------
# Usage & Help
# ------------------------------------------------------------------------------
print_usage() {
    cat << EOF
crorust Local CI Dogfood Test Runner

Usage: $(basename "$0") [OPTIONS]

Options:
  --doctor            Inspect host capabilities, pinned nightly, and exit
  --fix-deps          Attempt to auto-install missing nightly targets or trunk
  --all               Run full CI pipeline (Tiers 1, 2, 3, and 4) [default]
  --tier <1|2|3|4>    Execute only the specified tier (repeatable)
  --platform <name>   In Tier 4, build only specified platform (linux, android,
                      windows, web, macos, ios) (repeatable or comma-separated)
  --skip-tier1        Skip Tier 1 (formatting, clippy, unit tests)
  --skip-fmt          Skip 'cargo fmt --check' in Tier 1
  --skip-clippy       Skip 'cargo clippy' in Tier 1
  --skip-tests        Skip 'cargo test' in Tier 1
  --cache-dir <DIR>   Toolchain cache directory (default: .cache/crorust-tools)
  --stage-dir <DIR>   Stage compiled dogfood artifacts into DIR (like CI)
  --clean             Clean target and intermediate caches before starting
  --keep-going        Continue subsequent legs even if one fails (default: fail-fast)
  --fail-fast         Halt on the first step failure [default]
  --log-dir <DIR>     Directory to write detailed step logs (default: .ci-logs/local/)
  -v, --verbose       Print all command outputs to terminal in addition to log files
  -h, --help          Show this help message

Examples:
  $(basename "$0") --doctor
  $(basename "$0") --tier 4 --platform web,android
  $(basename "$0") --skip-tier1
  $(basename "$0") --all
EOF
}

# ------------------------------------------------------------------------------
# Host & Toolchain Introspection
# ------------------------------------------------------------------------------
detect_host() {
    HOST_OS_RAW="$(uname -s)"
    HOST_ARCH_RAW="$(uname -m)"

    case "$HOST_OS_RAW" in
        Linux*)
            HOST_OS="linux"
            CLI_EXE_NAME="crorust"
            ;;
        Darwin*)
            HOST_OS="macos"
            CLI_EXE_NAME="crorust"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            HOST_OS="windows"
            CLI_EXE_NAME="crorust.exe"
            ;;
        *)
            HOST_OS="unknown"
            CLI_EXE_NAME="crorust"
            ;;
    esac

    case "$HOST_ARCH_RAW" in
        x86_64|amd64) HOST_ARCH="x64" ;;
        aarch64|arm64) HOST_ARCH="arm64" ;;
        *) HOST_ARCH="$HOST_ARCH_RAW" ;;
    esac

    # Determine default host dogfood matrix legs (matches .github/workflows/ci.yml)
    # Target format: platform:arch:rust_target
    HOST_LEGS=()
    case "$HOST_OS" in
        linux)
            HOST_LEGS=(
                "linux:x64:x86_64-unknown-linux-gnu"
                "android:arm64:aarch64-linux-android"
                "windows:x64:x86_64-pc-windows-msvc"
                "web:x64:wasm32-unknown-unknown"
                "macos:arm64:aarch64-apple-darwin"
                "ios:arm64:aarch64-apple-ios"
            )
            ;;
        macos)
            HOST_LEGS=(
                "macos:arm64:aarch64-apple-darwin"
                "ios:arm64:aarch64-apple-ios"
                "android:arm64:aarch64-linux-android"
                "web:x64:wasm32-unknown-unknown"
                "linux:x64:x86_64-unknown-linux-gnu"
                "windows:x64:x86_64-pc-windows-msvc"
            )
            ;;
        windows)
            HOST_LEGS=(
                "windows:x64:x86_64-pc-windows-msvc"
                "android:arm64:aarch64-linux-android"
                "web:x64:wasm32-unknown-unknown"
                "linux:x64:x86_64-unknown-linux-gnu"
                "macos:arm64:aarch64-apple-darwin"
                "ios:arm64:aarch64-apple-ios"
            )
            ;;
        *)
            HOST_LEGS=()
            ;;
    esac
}

parse_template_toolchain() {
    local tc_file="${WORKSPACE_ROOT}/templates/crorust.ruxt/rust-toolchain.toml"
    if [ ! -f "$tc_file" ]; then
        log_error "Missing template toolchain file at $tc_file"
        exit 1
    fi

    PINNED_NIGHTLY="$(grep -E '^\s*channel\s*=' "$tc_file" | sed -E 's/.*"([^"]+)".*/\1/')"
    if [ -z "$PINNED_NIGHTLY" ]; then
        PINNED_NIGHTLY="nightly"
    fi

    # Extract targets array from rust-toolchain.toml
    REQUIRED_TARGETS=($(awk '/targets\s*=\s*\[/,/\]/' "$tc_file" | grep -o '"[^"]*"' | tr -d '"'))
}

check_single_tool() {
    local name="$1"
    local cmd="$2"
    local req="$3"
    local version=""
    if command -v "$cmd" >/dev/null 2>&1; then
        version="$("$cmd" --version 2>&1 | head -n 1 || true)"
        echo -e "  [${C_GREEN}PASS${C_RESET}] ${name} (${cmd}): ${C_DIM}${version}${C_RESET}"
        return 0
    else
        if [ "$req" = "true" ]; then
            echo -e "  [${C_RED}FAIL${C_RESET}] ${name} (${cmd}): NOT FOUND (Required)"
            return 1
        else
            echo -e "  [${C_YELLOW}WARN${C_RESET}] ${name} (${cmd}): NOT FOUND (Optional / Recommended)"
            return 0
        fi
    fi
}

run_doctor() {
    echo -e "${C_BOLD}================================================================================${C_RESET}"
    echo -e "${C_CYAN}${C_BOLD}CRORUST LOCAL CI PRE-FLIGHT DOCTOR${C_RESET}"
    echo -e "${C_BOLD}================================================================================${C_RESET}"
    echo -e "Host Workstation : ${C_BOLD}${HOST_OS} (${HOST_ARCH})${C_RESET}"
    echo -e "Workspace Root   : ${WORKSPACE_ROOT}"
    echo -e "Toolchain Cache  : ${CACHE_DIR}"
    echo -e "Pinned Nightly   : ${C_BOLD}${PINNED_NIGHTLY}${C_RESET} (from templates/crorust.ruxt/rust-toolchain.toml)"
    echo -e "--------------------------------------------------------------------------------"
    echo -e "${C_BOLD}Host Tools & Compilers:${C_RESET}"

    local all_good=true

    check_single_tool "Rustup Manager" "rustup" "true" || all_good=false
    check_single_tool "Cargo Package Manager" "cargo" "true" || all_good=false
    check_single_tool "Rust Compiler (Default)" "rustc" "true" || all_good=false
    check_single_tool "Clang C Compiler" "clang" "true" || all_good=false
    check_single_tool "LLD Linker" "lld" "true" || all_good=false
    check_single_tool "File Utility" "file" "true" || all_good=false
    check_single_tool "Unzip Archive Tool" "unzip" "true" || all_good=false
    check_single_tool "Trunk (Web Bundler)" "trunk" "false" || true
    if [ "$HOST_OS" = "linux" ]; then
        check_single_tool "Xvfb Headless Display" "xvfb-run" "false" || true
    fi

    # Python plistlib
    echo -n "  "
    if python3 -c "import plistlib" >/dev/null 2>&1; then
        echo -e "[${C_GREEN}PASS${C_RESET}] Python 3 plistlib module: Available"
    elif python -c "import plistlib" >/dev/null 2>&1; then
        echo -e "[${C_GREEN}PASS${C_RESET}] Python plistlib module: Available"
    else
        echo -e "[${C_YELLOW}WARN${C_RESET}] Python plistlib: NOT FOUND (Needed for macOS/iOS Info.plist checks)"
    fi

    echo -e "--------------------------------------------------------------------------------"
    echo -e "${C_BOLD}Pinned Toolchain & Target Cross Triples (${PINNED_NIGHTLY}):${C_RESET}"

    if command -v rustup >/dev/null 2>&1; then
        if rustup toolchain list | grep -F "$PINNED_NIGHTLY" >/dev/null 2>&1; then
            echo -e "  [${C_GREEN}PASS${C_RESET}] Pinned toolchain ${PINNED_NIGHTLY} is installed"
            local installed_targets
            installed_targets="$(rustup target list --toolchain "$PINNED_NIGHTLY" | grep '(installed)' | awk '{print $1}')"

            for target in "${REQUIRED_TARGETS[@]}"; do
                if echo "$installed_targets" | grep -F -x "$target" >/dev/null 2>&1; then
                    echo -e "    [${C_GREEN}OK${C_RESET}] Target: ${target}"
                else
                    echo -e "    [${C_RED}MISSING${C_RESET}] Target: ${target}"
                    all_good=false
                fi
            done
        else
            echo -e "  [${C_RED}FAIL${C_RESET}] Pinned toolchain ${PINNED_NIGHTLY} is NOT installed."
            all_good=false
        fi
    fi

    echo -e "--------------------------------------------------------------------------------"
    if [ "$all_good" = "true" ]; then
        echo -e "${C_GREEN}${C_BOLD}Doctor Verdict: System is ready for full CI Dogfood testing!${C_RESET}"
    else
        echo -e "${C_YELLOW}${C_BOLD}Doctor Verdict: Some prerequisites are missing.${C_RESET}"
        echo -e "Run '${C_BOLD}$(basename "$0") --fix-deps${C_RESET}' to automatically add missing targets."
    fi
    echo -e "================================================================================"
    if [ "$all_good" = "true" ]; then
        return 0
    else
        return 1
    fi
}

fix_dependencies() {
    log_info "Attempting automatic dependency remediation..."

    if ! command -v rustup >/dev/null 2>&1; then
        log_error "rustup is required to manage toolchains. Install from https://rustup.rs"
        return 1
    fi

    if ! rustup toolchain list | grep -F "$PINNED_NIGHTLY" >/dev/null 2>&1; then
        log_info "Installing pinned toolchain: ${PINNED_NIGHTLY}..."
        rustup toolchain install "$PINNED_NIGHTLY"
    fi

    local installed_targets
    installed_targets="$(rustup target list --toolchain "$PINNED_NIGHTLY" | grep '(installed)' | awk '{print $1}')"

    for target in "${REQUIRED_TARGETS[@]}"; do
        if ! echo "$installed_targets" | grep -F -x "$target" >/dev/null 2>&1; then
            log_info "Adding missing target ${target} to ${PINNED_NIGHTLY}..."
            rustup target add "$target" --toolchain "$PINNED_NIGHTLY"
        fi
    done

    # Check / install trunk if missing
    if ! command -v trunk >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/trunk" ]; then
        log_info "Trunk not found. Installing trunk for Web builds..."
        if [ "$HOST_OS" = "linux" ] && [ "$HOST_ARCH" = "x64" ]; then
            local trunk_url="https://github.com/trunk-rs/trunk/releases/download/v0.21.8/trunk-x86_64-unknown-linux-gnu.tar.gz"
            local trunk_tar="/tmp/trunk.tar.gz"
            curl -sSfL "$trunk_url" -o "$trunk_tar"
            mkdir -p "$HOME/.cargo/bin"
            tar -xzf "$trunk_tar" -C "$HOME/.cargo/bin"
            rm -f "$trunk_tar"
            log_pass "Trunk v0.21.8 installed into $HOME/.cargo/bin/trunk"
        else
            cargo install trunk --locked || true
        fi
    fi

    log_pass "Dependency remediation complete."
}

# ------------------------------------------------------------------------------
# Step Runner & Logger
# ------------------------------------------------------------------------------
run_step() {
    local tier_label="$1"
    local phase_name="$2"
    shift 2

    STEP_COUNTER=$((STEP_COUNTER + 1))
    local log_slug
    log_slug="$(echo "${phase_name}" | tr ' /:()' '____' | tr -s '_' | tr '[:upper:]' '[:lower:]')"
    local step_log="${RUN_LOG_DIR}/step_${STEP_COUNTER}_${log_slug}.log"
    mkdir -p "$RUN_LOG_DIR"

    echo -ne "  ${C_BOLD}${tier_label}${C_RESET} ${phase_name} ... "

    local step_start
    step_start=$(date +%s)
    local step_detail=""

    local exit_code=0
    if [ "$VERBOSE" = "true" ]; then
        echo ""
        set +e
        "$@" 2>&1 | tee "$step_log"
        exit_code=${PIPESTATUS[0]}
        set -e
    else
        set +e
        "$@" > "$step_log" 2>&1
        exit_code=$?
        set -e
    fi

    local step_end
    step_end=$(date +%s)
    local duration=$((step_end - step_start))
    local dur_formatted="${duration}s"

    if [ $exit_code -eq 0 ]; then
        echo -e "${C_GREEN}${C_BOLD}[PASS]${C_RESET} (${dur_formatted})"
        step_detail="Completed cleanly"
        SCORECARD_ENTRIES+=("${tier_label}|${phase_name}|PASS|${dur_formatted}|${step_detail}")
    else
        echo -e "${C_RED}${C_BOLD}[FAIL]${C_RESET} (${dur_formatted})"
        step_detail="Exit code $exit_code (see $(basename "$step_log"))"
        SCORECARD_ENTRIES+=("${tier_label}|${phase_name}|FAIL|${dur_formatted}|${step_detail}")
        OVERALL_SUCCESS=false

        log_error "Step failed: ${phase_name}"
        log_error "Log file: ${step_log}"
        echo -e "${C_RED}--- Tail of ${step_log} ---${C_RESET}"
        tail -n 25 "$step_log" >&2 || true
        echo -e "${C_RED}-----------------------------------${C_RESET}"

        if [ "$FAIL_FAST" = "true" ]; then
            log_error "Aborting immediately due to --fail-fast."
            return $exit_code
        fi
    fi

    return $exit_code
}

# ------------------------------------------------------------------------------
# TIER 1: Code Quality & Workspace Tests
# ------------------------------------------------------------------------------
run_tier1() {
    log_info "===================================================================="
    log_info "Tier 1: Code Quality & Multi-Host Workspace Tests"
    log_info "===================================================================="

    # CI Environment Variables
    export CARGO_TERM_COLOR="always"
    export RUST_BACKTRACE="1"
    export CARGO_INCREMENTAL="0"
    export CARGO_PROFILE_DEV_DEBUG="line-tables-only"
    export CARGO_PROFILE_TEST_DEBUG="line-tables-only"

    if [ "$SKIP_FMT" != "true" ]; then
        run_step "T1" "Code Formatting (cargo fmt --check)" \
            cargo fmt --all -- --check
    fi

    if [ "$SKIP_CLIPPY" != "true" ]; then
        run_step "T1" "Workspace Clippy Lints (-D warnings)" \
            cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    fi

    if [ "$SKIP_TESTS" != "true" ]; then
        run_step "T1" "Workspace Unit & Integration Tests" \
            cargo test --workspace --all-features --locked --no-fail-fast
    fi
}

# ------------------------------------------------------------------------------
# TIER 2: Fast CLI Binary Staging
# ------------------------------------------------------------------------------
run_tier2() {
    log_info "===================================================================="
    log_info "Tier 2: Fast CLI Binary Staging (crorust release build)"
    log_info "===================================================================="

    local target_dir="${CARGO_TARGET_DIR:-}"
    if [ -z "$target_dir" ]; then
        if [ -d "${WORKSPACE_ROOT}/.cache/target" ]; then
            target_dir="${WORKSPACE_ROOT}/.cache/target"
        else
            target_dir="${WORKSPACE_ROOT}/target"
        fi
    fi

    run_step "T2" "Compile crorust CLI in release mode" \
        cargo build --release --locked -p crorust

    run_step "T2" "Stage and strip CLI binary" \
        bash -c "
            mkdir -p '${WORKSPACE_ROOT}/bin' &&
            if [ -f '${target_dir}/release/${CLI_EXE_NAME}' ]; then
                SRC='${target_dir}/release/${CLI_EXE_NAME}'
            elif [ -f '${WORKSPACE_ROOT}/.cache/target/release/${CLI_EXE_NAME}' ]; then
                SRC='${WORKSPACE_ROOT}/.cache/target/release/${CLI_EXE_NAME}'
            else
                SRC='${WORKSPACE_ROOT}/target/release/${CLI_EXE_NAME}'
            fi &&
            cp \"\$SRC\" '${WORKSPACE_ROOT}/bin/${CLI_EXE_NAME}' &&
            if command -v strip >/dev/null 2>&1; then
                strip '${WORKSPACE_ROOT}/bin/${CLI_EXE_NAME}' 2>/dev/null || true
            elif command -v llvm-strip >/dev/null 2>&1; then
                llvm-strip '${WORKSPACE_ROOT}/bin/${CLI_EXE_NAME}' 2>/dev/null || true
            fi
        "

    local staged_bin="${WORKSPACE_ROOT}/bin/${CLI_EXE_NAME}"
    run_step "T2" "Verify CLI execution (--version)" \
        "$staged_bin" --version

    run_step "T2" "Verify CLI system diagnostics (i)" \
        "$staged_bin" i
}

# ------------------------------------------------------------------------------
# TIER 3: Scaffolding E2E Smoke Test
# ------------------------------------------------------------------------------
run_tier3() {
    log_info "===================================================================="
    log_info "Tier 3: Scaffolding E2E Smoke Test (template init -> check)"
    log_info "===================================================================="

    local staged_bin="${WORKSPACE_ROOT}/bin/${CLI_EXE_NAME}"
    if [ ! -x "$staged_bin" ]; then
        log_error "Staged CLI not found at $staged_bin. Run Tier 2 first."
        return 1
    fi

    run_step "T3" "Inspect template metadata (template info)" \
        "$staged_bin" template info

    local sandbox
    sandbox=$(mktemp -d "${TMPDIR:-/tmp}/crorust-e2e-XXXXXX")
    register_cleanup_dir "$sandbox"
    local app_dir="${sandbox}/smoke_test_app"

    run_step "T3" "Scaffold application with template init" \
        "$staged_bin" template init "$app_dir"

    run_step "T3" "Verify template identifier macro substitutions" \
        bash -c "
            grep -q 'name = \"smoke-test-app\"' '${app_dir}/Cargo.toml' &&
            grep -q 'package = \"smoke-test-app\"' '${app_dir}/manifest.toml' &&
            grep -q 'name = \"smoke_test_app\"' '${app_dir}/manifest.toml' &&
            grep -q 'label = \"Smoke Test App\"' '${app_dir}/manifest.toml' &&
            grep -q 'package com.example.smoke_test_app' '${app_dir}/kotlin/MainActivity.kt' &&
            grep -q 'System.loadLibrary(\"smoke_test_app\")' '${app_dir}/kotlin/MainActivity.kt'
        "

    run_step "T3" "Compile scaffolded app (cargo check --locked)" \
        bash -c "cd '${app_dir}' && cargo check --locked"

    run_step "T3" "Reference template self-check (template check)" \
        "$staged_bin" template check
}

# ------------------------------------------------------------------------------
# TIER 4: Cross-Platform Matrix Dogfooding & Deep Inspection
# ------------------------------------------------------------------------------
verify_artifact() {
    local platform="$1"
    local arch="$2"
    local template_dir="${WORKSPACE_ROOT}/templates/crorust.ruxt"

    case "$platform" in
        linux)
            local appdir
            appdir=$(find "$template_dir" -type d -name "*AppDir" 2>/dev/null | head -n 1)
            test -n "$appdir" && test -d "$appdir" || { echo "AppDir directory missing"; return 1; }
            test -f "$appdir/AppRun" || { echo "AppRun missing in $appdir"; return 1; }
            chmod +x "$appdir/AppRun" || true
            file -L "$appdir/AppRun" | grep -E "ELF 64-bit" || { echo "AppRun is not ELF 64-bit"; return 1; }

            if [ "$HOST_OS" = "linux" ] && command -v xvfb-run >/dev/null 2>&1; then
                LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe \
                    timeout 2s xvfb-run -a -s "-screen 0 1024x768x24 +extension GLX" "$appdir/AppRun" || [ $? -eq 124 ] || {
                        echo "AppRun failed headless execution test"; return 1;
                    }
            fi
            ;;
        macos)
            local app_bundle
            app_bundle=$(find "$template_dir" -type d -path "*/macos/*" -name "*.app" 2>/dev/null | head -n 1)
            test -n "$app_bundle" && test -d "$app_bundle" || { echo "macOS .app bundle missing"; return 1; }
            local plist="$app_bundle/Contents/Info.plist"
            test -f "$plist" || { echo "Info.plist missing in $app_bundle"; return 1; }
            python3 -c "import plistlib; p = plistlib.load(open('$plist', 'rb')); assert 'CFBundleIdentifier' in p" || {
                echo "Info.plist invalid or missing CFBundleIdentifier"; return 1;
            }
            file "$app_bundle/Contents/MacOS/"* | grep -E "Mach-O 64-bit" || { echo "Missing Mach-O 64-bit binary"; return 1; }
            ;;
        ios)
            local ios_app
            ios_app=$(find "$template_dir" -type d -path "*/ios/*" -name "*.app" 2>/dev/null | head -n 1)
            test -n "$ios_app" && test -d "$ios_app" || { echo "iOS .app bundle missing"; return 1; }
            local plist="$ios_app/Info.plist"
            test -f "$plist" || { echo "Info.plist missing in $ios_app"; return 1; }
            python3 -c "import plistlib; p = plistlib.load(open('$plist', 'rb')); assert 'CFBundleIdentifier' in p" || {
                echo "iOS Info.plist invalid or missing CFBundleIdentifier"; return 1;
            }
            file "$ios_app/"* | grep -E "Mach-O 64-bit" || { echo "Missing iOS Mach-O 64-bit binary"; return 1; }
            ;;
        android)
            local apk_file
            apk_file=$(find "$template_dir" -type f -name "*.apk" 2>/dev/null | head -n 1)
            test -n "$apk_file" && test -f "$apk_file" || { echo "Android APK missing"; return 1; }
            local size
            size=$(stat -c%s "$apk_file" 2>/dev/null || stat -f%z "$apk_file")
            test "$size" -gt 500000 || { echo "APK size $size bytes is suspiciously small (<500KB)"; return 1; }
            unzip -tq "$apk_file" || { echo "APK ZIP integrity test failed"; return 1; }
            unzip -l "$apk_file" | grep -E "classes\.dex" >/dev/null || { echo "classes.dex missing from APK"; return 1; }
            unzip -l "$apk_file" | grep -E "AndroidManifest\.xml" >/dev/null || { echo "AndroidManifest.xml missing from APK"; return 1; }
            unzip -l "$apk_file" | grep -E "resources\.arsc" >/dev/null || { echo "resources.arsc missing from APK"; return 1; }
            ;;
        windows)
            local win_exe
            win_exe=$(find "$template_dir" -type f -path "*/windows/*" -name "*.exe" 2>/dev/null | head -n 1)
            test -n "$win_exe" && test -f "$win_exe" || { echo "Windows executable missing"; return 1; }
            local size
            size=$(stat -c%s "$win_exe" 2>/dev/null || stat -f%z "$win_exe")
            test "$size" -gt 100000 || { echo "Windows binary size $size bytes is suspiciously small (<100KB)"; return 1; }
            file "$win_exe" | grep -E "(PE32\+|MS Windows)" || { echo "Binary is not PE32+ executable"; return 1; }
            ;;
        web)
            local dist_dir="$template_dir/.cache/web-static/dist"
            test -d "$dist_dir" || { echo "Web dist directory missing at $dist_dir"; return 1; }
            test -f "$dist_dir/index.html" || { echo "index.html missing in $dist_dir"; return 1; }
            local wasm_file
            wasm_file=$(find "$dist_dir" -maxdepth 2 -type f -name "*.wasm" 2>/dev/null | head -n 1)
            test -n "$wasm_file" && test -f "$wasm_file" || { echo "WASM binary missing in $dist_dir"; return 1; }
            ;;
    esac
}

stage_dogfood_artifact() {
    local platform="$1"
    local arch="$2"
    [ -n "$STAGE_DIR" ] || return 0
    mkdir -p "$STAGE_DIR"
    local template_dir="${WORKSPACE_ROOT}/templates/crorust.ruxt"

    case "$platform" in
        android)
            find "$template_dir" -type f -name "*.apk" -exec cp {} "${STAGE_DIR}/ruxt-android-${arch}.apk" \;
            ;;
        web)
            local web_dist="$template_dir/.cache/web-static/dist"
            if [ -d "$web_dist" ]; then
                cp -r "$web_dist" "${STAGE_DIR}/ruxt-web-${arch}"
            fi
            ;;
        windows)
            find "$template_dir" -type f -path "*/windows/*" -name "*.exe" -exec cp {} "${STAGE_DIR}/ruxt-windows-${arch}.exe" \;
            find "$template_dir" -type f -path "*/windows/*" -name "*.msix" -exec cp {} "${STAGE_DIR}/ruxt-windows-${arch}.msix" \; 2>/dev/null || true
            ;;
        macos|ios)
            local app_bundle
            app_bundle=$(find "$template_dir" -type d -path "*/$platform/*" -name "*.app" 2>/dev/null | head -n 1)
            if [ -n "$app_bundle" ]; then
                cp -r "$app_bundle" "${STAGE_DIR}/ruxt-${platform}-${arch}.app"
            fi
            ;;
        linux)
            local appdir
            appdir=$(find "$template_dir" -type d -name "*AppDir" 2>/dev/null | head -n 1)
            if [ -n "$appdir" ]; then
                cp -r "$appdir" "${STAGE_DIR}/ruxt-linux-${arch}.AppDir"
            fi
            find "$template_dir" -type f -name "*.AppImage" -exec cp {} "${STAGE_DIR}/ruxt-linux-${arch}.AppImage" \; 2>/dev/null || true
            ;;
    esac
}

run_tier4() {
    log_info "===================================================================="
    log_info "Tier 4: Multi-Host Cross-Platform Matrix Dogfooding"
    log_info "===================================================================="

    local staged_bin="${WORKSPACE_ROOT}/bin/${CLI_EXE_NAME}"
    if [ ! -x "$staged_bin" ]; then
        log_error "Staged CLI not found at $staged_bin. Run Tier 2 first."
        return 1
    fi

    # Determine legs to execute
    local legs_to_run=()
    if [ ${#SELECTED_PLATFORMS[@]} -eq 0 ]; then
        legs_to_run=("${HOST_LEGS[@]}")
    else
        for leg in "${HOST_LEGS[@]}"; do
            local p="$(echo "$leg" | cut -d: -f1)"
            for sel in "${SELECTED_PLATFORMS[@]}"; do
                if [ "$p" = "$sel" ]; then
                    legs_to_run+=("$leg")
                    break
                fi
            done
        done
    fi

    if [ ${#legs_to_run[@]} -eq 0 ]; then
        log_warn "No matching target legs found for current host (${HOST_OS}) with filters: ${SELECTED_PLATFORMS[*]}"
        return 0
    fi

    for leg in "${legs_to_run[@]}"; do
        local platform="$(echo "$leg" | cut -d: -f1)"
        local arch="$(echo "$leg" | cut -d: -f2)"
        local rust_target="$(echo "$leg" | cut -d: -f3)"

        log_info "--------------------------------------------------------"
        log_info "Dogfood Target Leg: ${platform} (${arch}) -> ${rust_target}"
        log_info "--------------------------------------------------------"

        # Pre-diagnostics check
        run_step "T4" "Toolchain check (${platform})" \
            "$staged_bin" i --project templates/crorust.ruxt --cache-dir "$CACHE_DIR"

        # Build Leg
        run_step "T4" "Build templates/crorust.ruxt (${platform}-${arch})" \
            "$staged_bin" build \
                --project templates/crorust.ruxt \
                --platform "$platform" \
                --arch "$arch" \
                --cache-dir "$CACHE_DIR" \
                --release \
                --verbose

        # Deep Inspection
        run_step "T4" "Deep artifact inspection (${platform})" \
            verify_artifact "$platform" "$arch"

        # Optional Staging
        stage_dogfood_artifact "$platform" "$arch"
    done
}

# ------------------------------------------------------------------------------
# TIER 5: Local Diagnostics Scorecard Summary
# ------------------------------------------------------------------------------
render_tier5_scorecard() {
    local end_time_total=$(date +%s)
    local total_elapsed=$((end_time_total - START_TIME_TOTAL))
    local total_minutes=$((total_elapsed / 60))
    local total_seconds=$((total_elapsed % 60))

    echo ""
    echo -e "${C_BOLD}================================================================================${C_RESET}"
    echo -e "${C_CYAN}${C_BOLD}CRORUST LOCAL CI VERIFICATION SCORECARD${C_RESET}"
    echo -e "${C_BOLD}================================================================================${C_RESET}"
    echo -e "Host Platform : ${HOST_OS} (${HOST_ARCH}) | Pinned Nightly: ${PINNED_NIGHTLY}"
    echo -e "Toolchain Dir : ${CACHE_DIR}"
    echo -e "Logs Written  : ${RUN_LOG_DIR}"
    echo -e "--------------------------------------------------------------------------------"
    printf "%-5s %-42s %-8s %-10s %s\n" "TIER" "PHASE / STEP" "STATUS" "DURATION" "DETAILS"
    echo -e "--------------------------------------------------------------------------------"

    local pass_count=0
    local fail_count=0

    for entry in "${SCORECARD_ENTRIES[@]}"; do
        IFS='|' read -r tier phase status dur details <<< "$entry"
        local status_str
        if [ "$status" = "PASS" ]; then
            status_str="${C_GREEN}[PASS]${C_RESET}"
            pass_count=$((pass_count + 1))
        else
            status_str="${C_RED}[FAIL]${C_RESET}"
            fail_count=$((fail_count + 1))
        fi
        printf "%-5s %-42s %-17b %-10s %s\n" "$tier" "${phase:0:41}" "$status_str" "$dur" "$details"
    done

    echo -e "--------------------------------------------------------------------------------"
    local total_steps=$((pass_count + fail_count))
    if [ "$fail_count" -eq 0 ] && [ "$total_steps" -gt 0 ]; then
        echo -e "${C_GREEN}${C_BOLD}TOTAL RESULT: SUCCESS (${pass_count}/${total_steps} passed in ${total_minutes}m ${total_seconds}s)${C_RESET}"
    else
        echo -e "${C_RED}${C_BOLD}TOTAL RESULT: FAILED (${fail_count}/${total_steps} failed in ${total_minutes}m ${total_seconds}s)${C_RESET}"
    fi
    echo -e "${C_BOLD}================================================================================${C_RESET}"
}

# ------------------------------------------------------------------------------
# Main Argument Parsing & Dispatch
# ------------------------------------------------------------------------------
detect_host
parse_template_toolchain

while [[ $# -gt 0 ]]; do
    case "$1" in
        --doctor)
            DOCTOR_MODE=true
            shift
            ;;
        --fix-deps)
            FIX_DEPS=true
            shift
            ;;
        --all)
            RUN_TIER1=true
            RUN_TIER2=true
            RUN_TIER3=true
            RUN_TIER4=true
            shift
            ;;
        --tier)
            if [ "$CUSTOM_TIERS" = "false" ]; then
                RUN_TIER1=false
                RUN_TIER2=false
                RUN_TIER3=false
                RUN_TIER4=false
                CUSTOM_TIERS=true
            fi
            case "$2" in
                1) RUN_TIER1=true ;;
                2) RUN_TIER2=true ;;
                3) RUN_TIER3=true ;;
                4) RUN_TIER4=true ;;
                *) log_error "Invalid tier '$2'. Must be 1, 2, 3, or 4."; exit 1 ;;
            esac
            shift 2
            ;;
        --platform)
            IFS=',' read -r -a p_list <<< "$2"
            for p in "${p_list[@]}"; do
                SELECTED_PLATFORMS+=("$p")
            done
            shift 2
            ;;
        --skip-tier1)
            RUN_TIER1=false
            shift
            ;;
        --skip-fmt)
            SKIP_FMT=true
            shift
            ;;
        --skip-clippy)
            SKIP_CLIPPY=true
            shift
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --cache-dir)
            CACHE_DIR="$2"
            shift 2
            ;;
        --stage-dir)
            STAGE_DIR="$2"
            shift 2
            ;;
        --clean)
            CLEAN_FIRST=true
            shift
            ;;
        --keep-going)
            FAIL_FAST=false
            shift
            ;;
        --fail-fast)
            FAIL_FAST=true
            shift
            ;;
        --log-dir)
            LOG_BASE_DIR="$2"
            RUN_LOG_DIR="${LOG_BASE_DIR}/run-${RUN_TIMESTAMP}"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# If doctor requested
if [ "$DOCTOR_MODE" = "true" ]; then
    run_doctor
    exit $?
fi

# If dependency fix requested
if [ "$FIX_DEPS" = "true" ]; then
    fix_dependencies
    exit $?
fi

# Optional cleaning step
if [ "$CLEAN_FIRST" = "true" ]; then
    log_info "Cleaning local build and intermediate targets..."
    cargo clean
    rm -rf "${WORKSPACE_ROOT}/bin" "${WORKSPACE_ROOT}/.cache/target" 2>/dev/null || true
fi

# Run preflight sanity check
if ! run_doctor >/dev/null 2>&1; then
    log_warn "Some host prerequisites or nightly targets are missing. Run '$(basename "$0") --doctor' for details."
fi

# Execute Pipeline Tiers
if [ "$RUN_TIER1" = "true" ]; then
    run_tier1
fi

if [ "$RUN_TIER2" = "true" ]; then
    run_tier2
fi

if [ "$RUN_TIER3" = "true" ]; then
    run_tier3
fi

if [ "$RUN_TIER4" = "true" ]; then
    run_tier4
fi

[ "$OVERALL_SUCCESS" = "true" ] || exit 1
