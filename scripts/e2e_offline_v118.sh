#!/usr/bin/env bash
# GAP-CLI / Wave 0: offline E2E gate for sqlite-graphrag v1.1.8
# No product env, no OpenRouter key, XDG isolated. Local only (no GA).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/release/sqlite-graphrag}"
if [[ ! -x "$BIN" ]]; then
  echo "building release binary..." >&2
  (cd "$ROOT" && cargo build --release)
fi
E2E="${TMPDIR:-/tmp}/graphrag-e2e-offline-$$"
mkdir -p "$E2E"/{xdg-config,xdg-data,xdg-cache,xdg-state,xdg-runtime}
export XDG_CONFIG_HOME="$E2E/xdg-config"
export XDG_DATA_HOME="$E2E/xdg-data"
export XDG_CACHE_HOME="$E2E/xdg-cache"
export XDG_STATE_HOME="$E2E/xdg-state"
export XDG_RUNTIME_DIR="$E2E/xdg-runtime"
# No product SQLITE_GRAPHRAG_* exports — use --db flags only (G-T-XDG-04).
unset OPENROUTER_API_KEY || true
DB="$E2E/graphrag.sqlite"
pass=0; fail=0
check() {
  local name="$1" expect="$2"; shift 2
  set +e
  local out code
  out=$("$@" 2>&1); code=$?
  set -e
  if [[ "$code" == "$expect" ]]; then
    echo "PASS $name (exit $code)"
    pass=$((pass+1))
  else
    echo "FAIL $name expect=$expect got=$code"
    echo "$out" | tail -20
    fail=$((fail+1))
  fi
}
"$BIN" init --db "$DB" --llm-backend none --quiet >/dev/null
check remember_none 0 "$BIN" remember --db "$DB" --name t-none --type note --description d --body "offline body" --llm-backend none --quiet
printf '%s' '{"body":"ICMS fiscal body","entities":[{"name":"icms-p05","type":"concept"}],"relationships":[]}' >"$E2E/g.json"
check graph_no_strength 0 "$BIN" remember --db "$DB" --name fiscal --type note --description f --graph-file "$E2E/g.json" --llm-backend none --quiet
sqlite3 "$DB" "UPDATE entities SET description='x is a configuration file for y' WHERE name='icms-p05';"
# Link entity to memory body for grounding sample (Wave 2).
sqlite3 "$DB" "INSERT OR IGNORE INTO memory_entities (memory_id, entity_id)
  SELECT m.id, e.id FROM memories m, entities e
  WHERE m.name='fiscal' AND e.name='icms-p05';"
check ed_status_force 0 "$BIN" enrich --db "$DB" --operation entity-descriptions --status --force-redescribe --quiet
STATUS_OUT=$("$BIN" enrich --db "$DB" --operation entity-descriptions --status --quality-sample 10 --quiet 2>/dev/null || true)
if echo "$STATUS_OUT" | grep -q 'quality_pct'; then
  echo "PASS ed_quality_pct (field present)"
  pass=$((pass+1))
else
  echo "FAIL ed_quality_pct missing in: $STATUS_OUT"
  fail=$((fail+1))
fi
check dry_run_no_mode 0 "$BIN" enrich --db "$DB" --operation entity-descriptions --dry-run --quiet
check me_desc 0 "$BIN" memory-entities --db "$DB" --name fiscal --quiet
check dr_o 0 "$BIN" deep-research --db "$DB" "ICMS" -o "$E2E/dr.json" --llm-backend none --quiet
[[ -f "$E2E/dr.json" ]] || { echo "FAIL dr file missing"; fail=$((fail+1)); }
check cfg_set 0 "$BIN" config set enrich.entity_description.domain fiscal --quiet
check cfg_doctor 0 "$BIN" config doctor --quiet
# Additional seal checks (v1.1.8 Wave D)
check pe_status 0 "$BIN" pending-embeddings status --db "$DB" --quiet
check cache_stats 0 "$BIN" cache stats --quiet
check purge_now_dry 0 "$BIN" purge --db "$DB" --now --dry-run --quiet
# Help must not advertise product env or clippy Box about
HELP_ROOT=$("$BIN" --help 2>&1 || true)
HELP_ENRICH=$("$BIN" enrich --help 2>&1 || true)
if echo "$HELP_ROOT$HELP_ENRICH" | grep -E 'SQLITE_GRAPHRAG_|Honors env|Boxed to keep' >/dev/null; then
  echo "FAIL help_contract (product env or Box about leaked)"
  fail=$((fail+1))
else
  echo "PASS help_contract"
  pass=$((pass+1))
fi
# Auto backend offline should not hang ~20s
START=$(date +%s)
check recall_auto_fast 0 "$BIN" recall --db "$DB" "offline" --llm-backend auto --quiet
END=$(date +%s)
ELAPSED=$((END-START))
if [[ "$ELAPSED" -lt 10 ]]; then
  echo "PASS recall_auto_wallclock (${ELAPSED}s < 10s)"
  pass=$((pass+1))
else
  echo "FAIL recall_auto_wallclock (${ELAPSED}s >= 10s)"
  fail=$((fail+1))
fi
check rename_none 0 "$BIN" rename-entity --db "$DB" --name icms-p05 --new-name icms-p05-renamed --llm-backend none --quiet
echo "SUMMARY pass=$pass fail=$fail workdir=$E2E"
exit "$fail"
