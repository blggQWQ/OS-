#!/usr/bin/env bash
# Compatibility wrapper — prefer scripts/run_boot_matrix.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec bash "${ROOT}/scripts/run_boot_matrix.sh" "$@"
