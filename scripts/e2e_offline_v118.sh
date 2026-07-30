#!/usr/bin/env bash
# Historical name kept as a thin wrapper. Canonical gate is e2e_offline_v120.sh (v1.2.0).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec bash "$ROOT/scripts/e2e_offline_v120.sh" "$@"
