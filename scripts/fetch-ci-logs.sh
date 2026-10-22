#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# crorust: CI Diagnostics & Artifact Retrieval Utility
#
# Retrieves workflow logs, diagnostic traces, and platform artifacts
# from GitHub Actions and unpacks them locally into .ci-logs/run-<run_id>/
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_BASE="${WORKSPACE_ROOT}/.ci-logs"

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS] [RUN_ID]

Options:
  --latest            Fetch the latest workflow run on the current branch (default)
  --run-id <ID>       Fetch diagnostics for a specific GitHub Actions Run ID
  --branch <BRANCH>   Filter runs by Git branch (default: current git branch)
  --output <DIR>      Base directory for downloaded logs (default: .ci-logs/)
  --no-artifacts      Only fetch the workflow log, skip artifact bundles
  --logs-only         Alias for --no-artifacts
  --artifact <NAME>   Only download this artifact (repeatable)
  --cli-only          Only download crorust CLI binaries (crorust-*)
  --app-only          Only download packaged application bundles (ruxt-*)
  --failed-only       Only download artifacts associated with failed/cancelled jobs
  --max-size <MB>     Skip any individual artifact exceeding this size in MB
  --concurrency <N>   Number of concurrent artifact downloads (default: 3)
  --list-artifacts    List available artifacts with sizes and exit (dry run)
  --dry-run           Alias for --list-artifacts
  --timeout <SECS>    Per-artifact download timeout in seconds (default: 120)
  --repo <OWNER/REPO> Override the repository (default: inferred from git remote)
  -h, --help          Show this help message

Authentication:
  - Uses the GitHub CLI ('gh') if installed and authenticated ('gh auth status').
  - Alternatively, export 'GITHUB_TOKEN' in your environment for REST API access.
EOF
}

TARGET_RUN_ID=""
TARGET_BRANCH=""
SKIP_ARTIFACTS=false
ARTIFACT_NAMES=()
GH_REPO_ARGS=()
REPO_OVERRIDE=""
TIMEOUT_SECS=120
CONCURRENCY=3
CLI_ONLY=false
APP_ONLY=false
FAILED_ONLY=false
MAX_SIZE_MB=""
LIST_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --latest)
            shift
            ;;
        --run-id)
            TARGET_RUN_ID="$2"
            shift 2
            ;;
        --branch)
            TARGET_BRANCH="$2"
            shift 2
            ;;
        --output)
            OUTPUT_BASE="$2"
            shift 2
            ;;
        --no-artifacts|--logs-only)
            SKIP_ARTIFACTS=true
            shift
            ;;
        --artifact)
            ARTIFACT_NAMES+=("$2")
            shift 2
            ;;
        --cli-only)
            CLI_ONLY=true
            shift
            ;;
        --app-only)
            APP_ONLY=true
            shift
            ;;
        --failed-only)
            FAILED_ONLY=true
            shift
            ;;
        --max-size)
            MAX_SIZE_MB="$2"
            shift 2
            ;;
        --concurrency)
            CONCURRENCY="$2"
            shift 2
            ;;
        --list-artifacts|--dry-run)
            LIST_ONLY=true
            shift
            ;;
        --timeout)
            TIMEOUT_SECS="$2"
            shift 2
            ;;
        --repo)
            REPO_OVERRIDE="$2"
            GH_REPO_ARGS=(--repo "$2")
            shift 2
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            if [[ -z "${TARGET_RUN_ID}" && "$1" =~ ^[0-9]+$ ]]; then
                TARGET_RUN_ID="$1"
                shift
            else
                echo "Unknown option: $1"
                print_usage
                exit 1
            fi
            ;;
    esac
done

if [[ -z "${TARGET_BRANCH}" ]]; then
    TARGET_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "master")
fi

mkdir -p "${OUTPUT_BASE}"

# Check GitHub CLI authentication
HAS_GH=false
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    HAS_GH=true
fi

# Resolve repository slug
if [[ -n "${REPO_OVERRIDE}" ]]; then
    REPO_SLUG="${REPO_OVERRIDE}"
else
    REMOTE_URL=$(git config --get remote.origin.url || true)
    REPO_SLUG=$(echo "${REMOTE_URL}" | sed -E 's/.*github\.com[:\/]([^/]+\/[^/.]+)(\.git)?/\1/')
fi

# Determine Run ID if not explicitly specified
if [[ -z "${TARGET_RUN_ID}" ]]; then
    if [[ "${HAS_GH}" = true ]]; then
        echo "Querying GitHub CLI for latest workflow run on branch '${TARGET_BRANCH}'..."
        TARGET_RUN_ID=$(gh run list ${GH_REPO_ARGS[@]+"${GH_REPO_ARGS[@]}"} --branch "${TARGET_BRANCH}" --workflow "ci.yml" --limit 1 --json databaseId --jq '.[0].databaseId')
    elif [[ -n "${GITHUB_TOKEN:-}" ]]; then
        echo "Querying GitHub REST API for latest run on '${REPO_SLUG}' (${TARGET_BRANCH})..."
        TARGET_RUN_ID=$(curl -s -H "Authorization: Bearer ${GITHUB_TOKEN}" \
            "https://api.github.com/repos/${REPO_SLUG}/actions/workflows/ci.yml/runs?branch=${TARGET_BRANCH}&per_page=1" \
            | grep -m 1 '"id":' | tr -dc '0-9')
    else
        echo "Error: No RUN_ID provided, 'gh' CLI is not authenticated, and 'GITHUB_TOKEN' is unset."
        echo ""
        echo "To authenticate:"
        echo "  gh auth login"
        echo "  or: export GITHUB_TOKEN=ghp_..."
        echo "  or specify an explicit RUN_ID: $0 <RUN_ID>"
        exit 1
    fi
fi

if [[ -z "${TARGET_RUN_ID}" || "${TARGET_RUN_ID}" == "null" ]]; then
    echo "Error: No workflow run found for branch '${TARGET_BRANCH}'."
    exit 1
fi

RUN_DIR="${OUTPUT_BASE}/run-${TARGET_RUN_ID}"
mkdir -p "${RUN_DIR}/raw-logs"
mkdir -p "${RUN_DIR}/artifacts"

format_bytes() {
    local b="$1"
    if (( b >= 1073741824 )); then
        awk -v b="$b" 'BEGIN { printf "%.2f GB", b / 1073741824 }'
    elif (( b >= 1048576 )); then
        awk -v b="$b" 'BEGIN { printf "%.2f MB", b / 1048576 }'
    elif (( b >= 1024 )); then
        awk -v b="$b" 'BEGIN { printf "%.2f KB", b / 1024 }'
    else
        printf "%d B" "$b"
    fi
}

echo "========================================================================"
echo "Fetching CI diagnostics for Run: ${TARGET_RUN_ID}"
echo "Repository: ${REPO_SLUG}"
echo "Destination directory: ${RUN_DIR}"
echo "========================================================================"

# Step 1: Download full console workflow log
if [[ "${LIST_ONLY}" = false ]]; then
    if [[ "${HAS_GH}" = true ]]; then
        echo "Downloading full console workflow log via gh..."
        if ! timeout "${TIMEOUT_SECS}" gh run view "${TARGET_RUN_ID}" ${GH_REPO_ARGS[@]+"${GH_REPO_ARGS[@]}"} --log \
                > "${RUN_DIR}/raw-logs/full_workflow.log"; then
            echo "Warning: workflow log download failed or timed out after ${TIMEOUT_SECS}s." >&2
        fi
    elif [[ -n "${GITHUB_TOKEN:-}" ]]; then
        echo "Note: Console workflow log download is not supported via basic REST fallback. Skipping console log."
    fi
fi

# Step 2: Retrieve artifact inventory
ARTIFACT_ITEMS=()
if [[ "${SKIP_ARTIFACTS}" = false ]]; then
    echo "Retrieving artifact inventory from GitHub..."
    if [[ "${HAS_GH}" = true ]]; then
        # Format: id<TAB>name<TAB>size_in_bytes<TAB>expired
        while IFS=$'\t' read -r art_id art_name art_size art_exp; do
            if [[ -n "${art_name}" && "${art_name}" != "null" ]]; then
                ARTIFACT_ITEMS+=("${art_id}	${art_name}	${art_size}	${art_exp}")
            fi
        done < <(gh api --paginate "repos/${REPO_SLUG}/actions/runs/${TARGET_RUN_ID}/artifacts" \
            --jq '.artifacts[] | "\(.id)\t\(.name)\t\(.size_in_bytes)\t\(.expired)"' 2>/dev/null || true)
    elif [[ -n "${GITHUB_TOKEN:-}" ]]; then
        REST_JSON=$(curl -s -H "Authorization: Bearer ${GITHUB_TOKEN}" \
            -H "Accept: application/vnd.github+json" \
            "https://api.github.com/repos/${REPO_SLUG}/actions/runs/${TARGET_RUN_ID}/artifacts?per_page=100")
        if command -v jq >/dev/null 2>&1; then
            while IFS=$'\t' read -r art_id art_name art_size art_exp; do
                if [[ -n "${art_name}" && "${art_name}" != "null" ]]; then
                    ARTIFACT_ITEMS+=("${art_id}	${art_name}	${art_size}	${art_exp}")
                fi
            done < <(echo "${REST_JSON}" | jq -r '.artifacts[] | "\(.id)\t\(.name)\t\(.size_in_bytes)\t\(.expired)"' 2>/dev/null || true)
        fi
    fi
fi

# Resolve failed jobs if --failed-only requested
FAILED_JOB_NAMES=()
if [[ "${FAILED_ONLY}" = true && "${HAS_GH}" = true ]]; then
    while read -r job_name; do
        if [[ -n "${job_name}" ]]; then
            FAILED_JOB_NAMES+=("${job_name}")
        fi
    done < <(gh run view "${TARGET_RUN_ID}" ${GH_REPO_ARGS[@]+"${GH_REPO_ARGS[@]}"} --json jobs \
        --jq '.jobs[] | select(.conclusion == "failure" or .conclusion == "cancelled") | .name' 2>/dev/null || true)
fi

# Filter artifacts
SELECTED_ARTIFACTS=()
echo ""
echo "Artifact Inventory (${#ARTIFACT_ITEMS[@]} found):"
printf "  %-40s %-12s %s\n" "Artifact Name" "Size" "Status"
printf "  %-40s %-12s %s\n" "----------------------------------------" "------------" "------"

for item in ${ARTIFACT_ITEMS[@]+"${ARTIFACT_ITEMS[@]}"}; do
    IFS=$'\t' read -r art_id art_name art_size art_exp <<< "${item}"
    formatted_sz=$(format_bytes "${art_size}")
    
    keep=true
    reason="Selected"

    if [[ "${art_exp}" == "true" ]]; then
        keep=false
        reason="Expired"
    fi

    # Explicit artifact list filter
    if [[ ${#ARTIFACT_NAMES[@]} -gt 0 && "$keep" = true ]]; then
        matched=false
        for target in "${ARTIFACT_NAMES[@]}"; do
            if [[ "${art_name}" == "${target}" ]]; then
                matched=true
                break
            fi
        done
        if [[ "${matched}" = false ]]; then
            keep=false
            reason="Excluded (not in --artifact list)"
        fi
    fi

    # CLI only filter
    if [[ "${CLI_ONLY}" = true && "$keep" = true ]]; then
        if [[ ! "${art_name}" =~ ^crorust- ]]; then
            keep=false
            reason="Excluded (not CLI)"
        fi
    fi

    # Application bundle only filter
    if [[ "${APP_ONLY}" = true && "$keep" = true ]]; then
        if [[ ! "${art_name}" =~ ^ruxt- ]]; then
            keep=false
            reason="Excluded (not application bundle)"
        fi
    fi

    # Max size filter
    if [[ -n "${MAX_SIZE_MB}" && "$keep" = true ]]; then
        max_bytes=$(( MAX_SIZE_MB * 1048576 ))
        if (( art_size > max_bytes )); then
            keep=false
            reason="Excluded (> ${MAX_SIZE_MB}MB)"
        fi
    fi

    # Failed only filter
    if [[ "${FAILED_ONLY}" = true && "$keep" = true && ${#FAILED_JOB_NAMES[@]} -gt 0 ]]; then
        matched=false
        for fj in "${FAILED_JOB_NAMES[@]}"; do
            # Check if artifact name hints at host or platform in failed job
            if [[ "${fj}" == *"${art_name}"* ]] || [[ "${art_name}" == *"${fj}"* ]]; then
                matched=true
                break
            fi
        done
        if [[ "${matched}" = false && ! "${art_name}" =~ ^crorust- ]]; then
            keep=false
            reason="Excluded (job succeeded)"
        fi
    fi

    if [[ "$keep" = true ]]; then
        SELECTED_ARTIFACTS+=("${item}")
        printf "  %-40s %-12s [SELECT]\n" "${art_name}" "${formatted_sz}"
    else
        printf "  %-40s %-12s [%s]\n" "${art_name}" "${formatted_sz}" "${reason}"
    fi
done
echo ""

if [[ "${LIST_ONLY}" = true ]]; then
    echo "Dry-run/list mode complete. Exiting without downloading artifacts."
    exit 0
fi

# Download artifacts
download_single_gh() {
    local run_id="$1"
    local name="$2"
    local out_dir="$3"
    local timeout_secs="$4"
    local max_retries=3
    local attempt=1
    local delay=2

    mkdir -p "${out_dir}/${name}"

    while (( attempt <= max_retries )); do
        if timeout "${timeout_secs}" gh run download "${run_id}" \
                ${GH_REPO_ARGS[@]+"${GH_REPO_ARGS[@]}"} \
                --name "${name}" \
                --dir "${out_dir}/${name}" >/dev/null 2>&1; then
            return 0
        fi

        echo "  [Warning] Download attempt ${attempt}/${max_retries} failed for '${name}'. Retrying in ${delay}s..." >&2
        sleep "${delay}"
        delay=$(( delay * 2 ))
        attempt=$(( attempt + 1 ))
    done

    echo "  [Error] Failed to download artifact '${name}' after ${max_retries} attempts." >&2
    return 1
}

download_single_rest() {
    local run_id="$1"
    local art_id="$2"
    local name="$3"
    local out_dir="$4"
    local timeout_secs="$5"
    local max_retries=3
    local attempt=1
    local delay=2

    local dest_dir="${out_dir}/${name}"
    mkdir -p "${dest_dir}"
    local temp_zip="${dest_dir}/bundle.zip"

    # Step 1: Query redirect URL without sending Bearer header to Azure
    local zip_api="https://api.github.com/repos/${REPO_SLUG}/actions/artifacts/${art_id}/zip"
    local signed_url
    signed_url=$(curl -sI -H "Authorization: Bearer ${GITHUB_TOKEN}" \
        -H "Accept: application/vnd.github+json" \
        "${zip_api}" | grep -i '^location:' | tr -d '\r' | awk '{print $2}')

    if [[ -z "${signed_url}" ]]; then
        echo "  [Error] Failed to resolve download redirect for '${name}'" >&2
        return 1
    fi

    while (( attempt <= max_retries )); do
        # Step 2: Download directly from signed storage URL WITHOUT Authorization header
        if curl -sL --max-time "${timeout_secs}" "${signed_url}" -o "${temp_zip}"; then
            if unzip -q -o "${temp_zip}" -d "${dest_dir}"; then
                rm -f "${temp_zip}"
                return 0
            fi
        fi

        echo "  [Warning] Download attempt ${attempt}/${max_retries} failed for '${name}'. Retrying in ${delay}s..." >&2
        sleep "${delay}"
        delay=$(( delay * 2 ))
        attempt=$(( attempt + 1 ))
    done

    rm -f "${temp_zip}"
    echo "  [Error] Failed to download/unpack artifact '${name}' after ${max_retries} attempts." >&2
    return 1
}

if [[ "${SKIP_ARTIFACTS}" = false && ${#SELECTED_ARTIFACTS[@]} -gt 0 ]]; then
    echo "Downloading ${#SELECTED_ARTIFACTS[@]} selected artifact(s) (concurrency=${CONCURRENCY}, timeout=${TIMEOUT_SECS}s per item)..."
    
    active_jobs=0
    failed_downloads=0

    for item in "${SELECTED_ARTIFACTS[@]}"; do
        IFS=$'\t' read -r art_id art_name art_size art_exp <<< "${item}"
        formatted_sz=$(format_bytes "${art_size}")

        (
            echo " [Queued] Downloading: ${art_name} (${formatted_sz})..."
            if [[ "${HAS_GH}" = true ]]; then
                if download_single_gh "${TARGET_RUN_ID}" "${art_name}" "${RUN_DIR}/artifacts" "${TIMEOUT_SECS}"; then
                    echo " [OK] Finished: ${art_name}"
                else
                    exit 1
                fi
            elif [[ -n "${GITHUB_TOKEN:-}" ]]; then
                if download_single_rest "${TARGET_RUN_ID}" "${art_id}" "${art_name}" "${RUN_DIR}/artifacts" "${TIMEOUT_SECS}"; then
                    echo " [OK] Finished: ${art_name}"
                else
                    exit 1
                fi
            fi
        ) &

        active_jobs=$(( active_jobs + 1 ))
        if (( active_jobs >= CONCURRENCY )); then
            if ! wait -n; then
                failed_downloads=$(( failed_downloads + 1 ))
            fi
            active_jobs=$(( active_jobs - 1 ))
        fi
    done

    while (( active_jobs > 0 )); do
        if ! wait -n; then
            failed_downloads=$(( failed_downloads + 1 ))
        fi
        active_jobs=$(( active_jobs - 1 ))
    done

    if (( failed_downloads > 0 )); then
        echo "Warning: ${failed_downloads} artifact(s) failed to download." >&2
    else
        echo "All selected artifacts downloaded successfully."
    fi
fi

# Build Consolidated Local Summary
LOCAL_SUMMARY="${RUN_DIR}/ci-run-summary.md"
LOCAL_REPORT="${RUN_DIR}/ci-run-report.json"
RAW_LOG="${RUN_DIR}/raw-logs/full_workflow.log"

echo "Synthesizing diagnostic summary at: ${LOCAL_SUMMARY}"

{
    echo "# CI Run ${TARGET_RUN_ID}"
    echo
    echo "- **Branch:** \`${TARGET_BRANCH}\`"
    echo "- **Retrieved:** \`$(date -u '+%Y-%m-%d %H:%M:%S UTC')\`"
    echo "- **Run directory:** \`${RUN_DIR}\`"
    echo
} > "${LOCAL_SUMMARY}"

if [[ -s "${RAW_LOG}" ]] && command -v python3 >/dev/null 2>&1; then
    if python3 "${SCRIPT_DIR}/analyze-ci-log.py" "${RAW_LOG}" \
            --md "${RUN_DIR}/.analysis.md" --json "${LOCAL_REPORT}"; then
        sed '1d' "${RUN_DIR}/.analysis.md" >> "${LOCAL_SUMMARY}"
        rm -f "${RUN_DIR}/.analysis.md"
    else
        echo "Warning: analyze-ci-log.py failed; summary has no failure analysis." >&2
        echo "_Analysis failed, see terminal output._" >> "${LOCAL_SUMMARY}"
    fi
else
    echo "Warning: no console log or no python3; summary will be limited." >&2
    echo "_No console log available, so no failure analysis was possible._" >> "${LOCAL_SUMMARY}"
fi

{
    echo
    echo "## Downloaded files"
    echo
} >> "${LOCAL_SUMMARY}"
find "${RUN_DIR}" -type f ! -name 'ci-run-summary.md' ! -name 'ci-run-report.json' | sort | while read -r filepath; do
    REL_PATH="${filepath#"${RUN_DIR}/"}"
    BYTES=$(stat -c%s "$filepath" 2>/dev/null || stat -f%z "$filepath" 2>/dev/null || echo 0)
    echo "- \`${REL_PATH}\` (${BYTES} bytes)" >> "${LOCAL_SUMMARY}"
done

if [[ ! -s "${LOCAL_REPORT}" ]]; then
    printf '{"run_id": "%s", "branch": "%s", "jobs": [], "failed": null}\n' \
        "${TARGET_RUN_ID}" "${TARGET_BRANCH}" > "${LOCAL_REPORT}"
fi

echo "========================================================================"
echo "Diagnostics successfully collected in: ${RUN_DIR}"
echo "Summary file: ${LOCAL_SUMMARY}"
echo "Structured JSON: ${LOCAL_REPORT}"
echo "========================================================================"
