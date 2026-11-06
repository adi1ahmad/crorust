# Developer & CI Scripts

Internal utilities for local CI simulation, diagnostics retrieval, log analysis, and asset validation.

| Script | Purpose |
| :--- | :--- |
| `local_ci_dogfood_test.sh` | Runs the full multi-tier CI pipeline locally (Tier 1 lint/unit tests, Tier 2 binary staging, Tier 3 packaging, and Tier 4 cross-target builds). Supports `--doctor` to inspect required host tools. |
| `fetch-ci-logs.sh` | Retrieves workflow run logs, failed step logs, and platform build artifacts from GitHub Actions via `gh` or `GITHUB_TOKEN`. |
| `analyze-ci-log.py` | Parses raw GitHub Actions logs (`gh run view --log`) to extract failing steps, rule-based diagnoses, and test summaries into Markdown or JSON. |
| `check-release-assets.sh` | Periodically tests that all external toolchain download URLs in `catalog.rs` are reachable and resolve with HTTP 200. |

### Running the Local CI Runner

```sh
# Pre-flight environment check
./scripts/local_ci_dogfood_test.sh --doctor

# Run unit checks only (Tier 1)
./scripts/local_ci_dogfood_test.sh --tier 1

# Run full multi-tier local CI
./scripts/local_ci_dogfood_test.sh --all
```
