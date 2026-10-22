#!/usr/bin/env bash
# =============================================================================
# GRAPHICS SETUP AND MULTI-HOST DIAGNOSTICS UTILITY
# =============================================================================
# Supports Linux (NixOS & FHS), Windows Subsystem for Linux (WSL2), and macOS.
# Automatically detects Intel, AMD Radeon, NVIDIA, WSL2 D3D12, Apple Metal,
# and Lavapipe CPU software rendering.
# =============================================================================

set -euo pipefail
# Note: common.sh is concatenated before this file by nix/overlays.nix

# =============================================================================
# 1. UNIFIED MULTI-OS & MULTI-GPU DETECTION ENGINE
# =============================================================================
detect_gpu_environment() {
    local host_os
    host_os=$(detect_host_os)

    # --- A. macOS (Darwin) Native Metal Environment ---
    if [[ "$host_os" == "macos" ]]; then
        local mac_gpu="Apple Metal GPU"
        local chip_model
        chip_model=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "")
        if [[ -n "$chip_model" ]]; then
            mac_gpu="$chip_model"
        elif command -v system_profiler >/dev/null 2>&1; then
            mac_gpu=$(system_profiler SPDisplaysDataType 2>/dev/null | awk -F': ' '/Chipset Model:/ {print $2; exit}' || echo "Apple Metal GPU")
        fi
        echo "# macOS Metal GPU detected: ${mac_gpu}"
        echo "export HOST_ENVIRONMENT='macos'"
        echo "export GPU_VENDOR='apple'"
        echo "export GPU_SUMMARY='${mac_gpu//\'/} (Metal)'"
        echo "export VULKAN_SUPPORTED='0'"
        echo "export METAL_SUPPORTED='1'"
        echo "export VAAPI_SUPPORTED='0'"
        return 0
    fi

    # --- B. Windows Subsystem for Linux (WSL2) DirectX 12 Passthrough ---
    if [[ "$host_os" == "wsl2" ]]; then
        if [[ -e /dev/dxg ]]; then
            echo "# WSL2 Direct3D 12 GPU environment detected"
            echo "export HOST_ENVIRONMENT='wsl2'"
            echo "export GPU_VENDOR='wsl2'"
            echo "export GPU_SUMMARY='WSL2 DirectX 12 (Dozen / d3d12)'"
            echo "export INTEL_GPU_GENERATION='wsl2'"
            echo "export LIBVA_DRIVER_NAME='d3d12'"
            echo "export MESA_LOADER_DRIVER_OVERRIDE='d3d12'"
            echo "export __GLX_VENDOR_LIBRARY_NAME='mesa'"
            echo "export MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE='0'"
            echo "export VULKAN_SUPPORTED='1'"
            echo "export VAAPI_SUPPORTED='1'"
            return 0
        else
            echo "# WSL2 detected without /dev/dxg hardware channel (Software Fallback)"
            echo "export HOST_ENVIRONMENT='wsl2'"
            echo "export GPU_VENDOR='software'"
            echo "export GPU_SUMMARY='WSL2 Software (Lavapipe/llvmpipe)'"
            echo "unset MESA_LOADER_DRIVER_OVERRIDE"
            echo "export __GLX_VENDOR_LIBRARY_NAME='mesa'"
            echo "export VULKAN_SUPPORTED='1'"
            echo "export VAAPI_SUPPORTED='0'"
            return 0
        fi
    fi

    # --- C. Linux PCI Hardware Detection (AMD, NVIDIA, Intel) ---
    local gpu_info=""
    local gpu_generation="unknown"
    local mesa_driver=""
    local vaapi_driver=""
    local glx_vendor="mesa"
    local gpu_vendor="unknown"
    local gpu_summary="Software / Lavapipe Fallback"
    local vulkan_supported="1"

    if command -v lspci >/dev/null 2>&1; then
        local pci_vga
        pci_vga=$(lspci 2>/dev/null | grep -iE "vga|3d|display" || echo "")

        # 1. Check AMD Radeon (Discrete or APU)
        local amd_gpu
        amd_gpu=$(echo "$pci_vga" | grep -iE "\b(amd|ati|radeon)\b" | head -1 || echo "")
        if [[ -n "$amd_gpu" ]]; then
            local clean_amd="${amd_gpu##*: }"
            clean_amd=$(echo "$clean_amd" | sed -E 's/Advanced Micro Devices, Inc\. \[AMD\/ATI\] //g; s/ \(rev [0-9a-fA-F]+\)//g')
            echo "# AMD Radeon GPU detected: $amd_gpu"
            echo "export HOST_ENVIRONMENT='${host_os}'"
            echo "export GPU_VENDOR='amd'"
            echo "export GPU_SUMMARY='${clean_amd//\'/} (RADV)'"
            echo "export INTEL_GPU_GENERATION='none'"
            echo "export LIBVA_DRIVER_NAME='radeonsi'"
            echo "export MESA_LOADER_DRIVER_OVERRIDE='radeonsi'"
            echo "export __GLX_VENDOR_LIBRARY_NAME='mesa'"
            echo "export MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE='0'"
            echo "export VULKAN_SUPPORTED='1'"
            echo "export VAAPI_SUPPORTED='1'"
            return 0
        fi

        # 2. Check NVIDIA
        local nvidia_gpu
        nvidia_gpu=$(echo "$pci_vga" | grep -i "nvidia" | head -1 || echo "")
        if [[ -n "$nvidia_gpu" ]]; then
            local clean_nv="${nvidia_gpu##*: }"
            clean_nv=$(echo "$clean_nv" | sed -E 's/NVIDIA Corporation //g; s/ \(rev [0-9a-fA-F]+\)//g')
            echo "# NVIDIA GPU detected: $nvidia_gpu"
            echo "export HOST_ENVIRONMENT='${host_os}'"
            echo "export GPU_VENDOR='nvidia'"
            echo "export GPU_SUMMARY='${clean_nv//\'/} (NVIDIA)'"
            echo "export INTEL_GPU_GENERATION='none'"
            echo "export LIBVA_DRIVER_NAME='nvidia'"
            echo "unset MESA_LOADER_DRIVER_OVERRIDE"
            echo "export __GLX_VENDOR_LIBRARY_NAME='nvidia'"
            echo "export MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE='0'"
            echo "export VULKAN_SUPPORTED='1'"
            echo "export VAAPI_SUPPORTED='1'"
            return 0
        fi

        # 3. Check Intel
        gpu_info=$(echo "$pci_vga" | grep -i "intel" | head -1 || echo "")
        if [[ -n "$gpu_info" ]]; then
            local clean_intel="${gpu_info##*: }"
            clean_intel=$(echo "$clean_intel" | sed -E 's/Intel Corporation //g; s/ \(rev [0-9a-fA-F]+\)//g')
            gpu_vendor="intel"
            echo "# Intel GPU detected: $gpu_info"
            case "$gpu_info" in
                *"Xe"*|*"Arc"*|*"DG1"*|*"DG2"*|*"Meteor Lake"*|*"Lunar Lake"*|*"Arrow Lake"*)
                    gpu_generation="xe"
                    mesa_driver="iris"
                    vaapi_driver="iHD"
                    vulkan_supported="1"
                    ;;
                *"Tiger Lake"*|*"Rocket Lake"*|*"Alder Lake"*|*"Raptor Lake"*|*"Ice Lake"*|*"Comet Lake"*)
                    gpu_generation="gen12"
                    mesa_driver="iris"
                    vaapi_driver="iHD"
                    vulkan_supported="1"
                    ;;
                *"Coffee Lake"*|*"Whiskey Lake"*|*"Amber Lake"*|*"Kaby Lake"*|*"Skylake"*|*"Apollo Lake"*|*"Gemini Lake"*)
                    gpu_generation="gen9"
                    mesa_driver="iris"
                    vaapi_driver="iHD"
                    vulkan_supported="1"
                    ;;
                *"Broadwell"*|*"Cherryview"*)
                    gpu_generation="gen8"
                    mesa_driver="iris"
                    vaapi_driver="i965"
                    vulkan_supported="partial"
                    ;;
                *"Haswell"*|*"Bay Trail"*|*"Braswell"*|*"Ivy Bridge"*)
                    gpu_generation="gen7"
                    mesa_driver="crocus"
                    vaapi_driver="i965"
                    vulkan_supported="partial"
                    ;;
                *"Sandy Bridge"*)
                    gpu_generation="gen6"
                    mesa_driver="crocus"
                    vaapi_driver="i965"
                    vulkan_supported="0"
                    ;;
                *)
                    gpu_generation="modern-intel"
                    mesa_driver="iris"
                    vaapi_driver="iHD"
                    vulkan_supported="1"
                    ;;
            esac
            gpu_summary="Intel ${clean_intel//\'/} (${mesa_driver}/${vaapi_driver})"
        fi
    fi

    echo "export HOST_ENVIRONMENT='${host_os}'"
    echo "export GPU_VENDOR='${gpu_vendor}'"
    echo "export GPU_SUMMARY='${gpu_summary}'"
    echo "export INTEL_GPU_GENERATION='${gpu_generation}'"
    echo "export LIBVA_DRIVER_NAME='${vaapi_driver}'"
    echo "export __GLX_VENDOR_LIBRARY_NAME='${glx_vendor}'"
    echo "export MESA_LOADER_DRIVER_OVERRIDE='${mesa_driver}'"
    echo "export MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE='0'"
    echo "export VULKAN_SUPPORTED='${vulkan_supported}'"
}

# =============================================================================
# 2. DIAGNOSTIC CHECK SUITES
# =============================================================================

check_system_info() {
    print_header "🖥️" "HOST SYSTEM & DISPLAY SERVER" "$BLUE"

    local host_os
    host_os=$(detect_host_os)

    if [[ "$host_os" == "macos" ]]; then
        local mac_ver
        mac_ver=$(sw_vers -productVersion 2>/dev/null || uname -r)
        print_result "info" "OS: macOS ${mac_ver} ($(uname -m))"
        print_result "pass" "Display Server: Quartz Compositor / Cocoa"
        return 0
    fi

    if [[ -f /etc/os-release ]]; then
        local os_info
        os_info=$(grep '^PRETTY_NAME=' /etc/os-release | cut -d'"' -f2)
        print_result "info" "OS: ${os_info} (${host_os})"
    fi

    print_result "info" "Kernel: $(uname -sr) ($(uname -m))"

    if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
        print_result "pass" "Display Server: Wayland (${WAYLAND_DISPLAY})"
    elif [[ -n "${DISPLAY:-}" ]]; then
        print_result "pass" "Display Server: X11 (${DISPLAY})"
    else
        print_result "warn" "Display Server: Headless (Lavapipe CPU Vulkan available for CI)"
    fi
}

check_hardware() {
    print_header "🔧" "HARDWARE & GPU DETECTION" "$BLUE"

    eval "$(detect_gpu_environment)"
    print_result "pass" "Detected GPU: ${GPU_SUMMARY:-Unknown}"
    print_result "info" "Vendor Profile: ${GPU_VENDOR:-unknown} (Host: ${HOST_ENVIRONMENT:-unknown})"
}

check_vulkan() {
    print_header "🌋" "VULKAN / WGPU GRAPHICS STACK" "$BLUE"

    if [[ "$(detect_host_os)" == "macos" ]]; then
        print_result "pass" "macOS uses native Apple Metal backend in wgpu"
        return 0
    fi

    if run_test "Vulkan Loader (vulkaninfo)" "command -v vulkaninfo"; then
        local vk_devices
        vk_devices=$(vulkaninfo --summary 2>/dev/null | awk -F= '/deviceName/ {gsub(/^[ \t]+/, "", $2); print $2}' || echo "")
        if [[ -n "$vk_devices" ]]; then
            while IFS= read -r dev; do
                [[ -n "$dev" ]] && print_result "pass" "Vulkan Adapter: ${dev}"
            done <<< "$vk_devices"
        else
            print_result "warn" "No Vulkan adapters enumerated by vulkaninfo"
        fi
    fi

    if [[ -n "${VK_ICD_FILENAMES:-}" ]]; then
        local icd_count=0
        IFS=':' read -ra icds <<< "$VK_ICD_FILENAMES"
        for icd in "${icds[@]}"; do
            [[ -f "$icd" ]] && icd_count=$((icd_count + 1))
        done
        print_result "info" "Active Vulkan ICD manifests found: ${icd_count}"
    fi
}

check_opengl() {
    [[ "$(detect_host_os)" == "macos" ]] && return 0

    print_header "🎨" "OPENGL / MESA DRI DIAGNOSTICS" "$BLUE"

    if run_test "OpenGL Utility (glxinfo)" "command -v glxinfo" 0; then
        local gl_renderer
        gl_renderer=$(glxinfo 2>/dev/null | awk -F': ' '/OpenGL renderer string/ {print $2; exit}' || echo "")
        if [[ -n "$gl_renderer" ]]; then
            if echo "$gl_renderer" | grep -qi "llvmpipe\|softpipe\|swrast"; then
                print_result "warn" "OpenGL Renderer: ${gl_renderer} (Software)"
            else
                print_result "pass" "OpenGL Renderer: ${gl_renderer} (Hardware Accelerated)"
            fi
        fi
    fi
}

check_vaapi() {
    [[ "$(detect_host_os)" == "macos" ]] && return 0

    print_header "🎬" "VA-API VIDEO ACCELERATION" "$BLUE"
    if command -v vainfo >/dev/null 2>&1; then
        if vainfo >/dev/null 2>&1; then
            local profiles
            profiles=$(vainfo 2>/dev/null | grep -c "VAProfile" || echo "0")
            print_result "pass" "VA-API active (${profiles} hardware profiles, driver: ${LIBVA_DRIVER_NAME:-auto})"
        else
            print_result "skip" "VA-API not active on this display/GPU (${LIBVA_DRIVER_NAME:-none})"
        fi
    else
        print_result "skip" "vainfo not installed (use 'nix develop .#debug')"
    fi
}

check_dev_environment() {
    print_header "🦀" "CRORUST & CROSS-COMPILATION TOOLCHAIN" "$BLUE"

    run_test "CroRust CLI (crorust)" "command -v crorust"
    run_test "Rust Compiler (rustc)" "command -v rustc"
    run_test "Cargo Build System (cargo)" "command -v cargo"
    run_test "LLVM Clang Driver (clang)" "command -v clang"
    run_test "LLVM Linker (lld)" "command -v lld"
    run_test "LLVM Archiver (llvm-ar)" "command -v llvm-ar"
    run_test "LLVM ELF Analyzer (llvm-readobj)" "command -v llvm-readobj"
    run_test "Java JDK 17 (javac)" "command -v javac"
    run_test "Kotlin Standalone Compiler (kotlinc)" "command -v kotlinc"
    run_test "Android Platform Tools (adb)" "command -v adb"
    run_test "WebAssembly Bundler (trunk)" "command -v trunk"

    if [[ -n "${SHADERC_LIB_DIR:-}" && -d "$SHADERC_LIB_DIR" ]]; then
        print_result "pass" "Prebuilt Shaderc library (${SHADERC_LIB_DIR})"
    else
        print_result "warn" "SHADERC_LIB_DIR not configured"
    fi
}

run_performance_tests() {
    local run_perf="${1:-0}"
    [[ "$run_perf" != "1" ]] && return 0

    print_header "🚀" "CACHE & PERFORMANCE METRICS" "$BLUE"
    if [[ -n "${MESA_GLSL_CACHE_DIR:-}" && -d "$MESA_GLSL_CACHE_DIR" ]]; then
        local cache_size
        cache_size=$(du -sh "$MESA_GLSL_CACHE_DIR" 2>/dev/null | cut -f1 || echo "0B")
        print_result "info" "Mesa Shader Cache (${MESA_GLSL_CACHE_DIR}): ${cache_size}"
    fi
    if command -v crorust >/dev/null 2>&1; then
        crorust cache info || true
    fi
}

# =============================================================================
# 3. ENTRYPOINT MODES
# =============================================================================

mode_detect() {
    echo "#!/usr/bin/env bash"
    echo "# Auto-generated by graphics-setup detect"
    detect_gpu_environment
}

mode_check() {
    eval "$(detect_gpu_environment)"
    print_header "🎮" "GRAPHICS & ENVIRONMENT STATUS" "$CYAN"
    print_status "success" "Host Environment : ${HOST_ENVIRONMENT:-unknown} ($(uname -m))"
    print_status "success" "Graphics Profile : ${GPU_SUMMARY:-Unknown}"

    if [[ "${HOST_ENVIRONMENT:-}" != "macos" ]] && command -v vulkaninfo >/dev/null 2>&1; then
        local vk_dev
        vk_dev=$(vulkaninfo --summary 2>/dev/null | awk -F= '/deviceName/ {gsub(/^[ \t]+/, "", $2); print $2; exit}' || echo "")
        if [[ -n "$vk_dev" ]]; then
            print_status "success" "Primary Vulkan   : ${vk_dev}"
        else
            print_status "warning" "Primary Vulkan   : Enumeration failed"
        fi
    fi
    echo ""
}

mode_diagnose() {
    local run_performance=0
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --performance|-p) run_performance=1; shift ;;
            --help|-h) show_help; exit 0 ;;
            *) echo "Unknown option: $1"; exit 1 ;;
        esac
    done

    check_system_info
    check_hardware
    check_vulkan
    check_opengl
    check_vaapi
    check_dev_environment
    run_performance_tests "$run_performance"
    echo ""
}

mode_configure() {
    eval "$(detect_gpu_environment)"
    print_status "success" "Applied GPU profile: ${GPU_SUMMARY:-Unknown}"
}

main() {
    if [[ $# -eq 0 ]]; then
        show_help
        exit 0
    fi

    local mode="$1"
    shift

    case "$mode" in
        detect)    mode_detect ;;
        check)     mode_check ;;
        diagnose)  mode_diagnose "$@" ;;
        configure) mode_configure ;;
        --help|-h) show_help ;;
        *)
            echo "Unknown mode: $mode (use --help for usage)" >&2
            exit 1
            ;;
    esac
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi