#!/usr/bin/env bash
# Scheduled check that every download URL pinned in the tool catalog still resolves.
# Meant for scheduled runs, not per-push CI: these are third-party services and
# should not gate a PR on a transient CDN error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CATALOG="${ROOT}/crates/crorust/src/download/catalog.rs"

fail=0
checked=0

while read -r url; do
    [[ -z "${url}" ]] && continue
    # Range GET instead of HEAD: some CDNs reject HEAD (403/405) while serving GET.
    code="$(curl -sS -L --retry 3 --retry-all-errors --max-time 60 \
        -r 0-0 -o /dev/null -w '%{http_code}' "${url}" 2>/dev/null || true)"
    code="${code:-000}"
    checked=$((checked + 1))
    if [[ "${code}" =~ ^2 ]]; then
        printf 'ok   %s\n' "${url}"
    else
        printf 'FAIL %s (HTTP %s)\n' "${url}" "${code}"
        fail=1
    fi
done < <(grep -oE 'https://[^"[:space:]]+' "${CATALOG}" | sort -u)

if [[ "${checked}" -eq 0 ]]; then
    echo "FAIL no download URLs found in ${CATALOG}" >&2
    exit 1
fi

printf '\nChecked %d URL(s): %s\n' "${checked}" \
    "$([[ "${fail}" -eq 0 ]] && echo 'all reachable' || echo 'failures detected')"

exit "${fail}"
