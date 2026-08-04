#![cfg(feature = "slow-tests")]

//! Contract: enrich — dry-run envelope and the required --mode guard.
//!
//! Part of the JSON-contract suite split by GAP-SG-208: the single file held
//! 1393 lines and 41 tests, past the 800-line ceiling this project sets for
//! itself. The shared harness lives in `tests/contract_support/`.
//!
//! Ground truth: `docs/schemas/*.schema.json`. Each test checks the expected
//! exit code, valid JSON, and the presence of the required keys.

#[path = "contract_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{assert_has_keys, Env};
// ---------------------------------------------------------------------------
// 39 — enrich (dry-run, no LLM spawned)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_39_enrich() {
    let env = Env::new();
    env.init();
    // Seed one memory without entity bindings so the scan finds it.
    env.remember(
        "mem-enrich-contract",
        "auth uses JWT with short expiry and refresh tokens",
    );

    // dry-run mode: emits phase events + preview item events + summary without calling LLM.
    let out = env
        .cmd()
        .args([
            "enrich",
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "enrich dry-run failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // Output is NDJSON: one line per event.
    let stdout_str = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout_str
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(
        !lines.is_empty(),
        "enrich must emit at least one NDJSON line"
    );

    // Parse each line and find the summary (last non-empty line typically).
    let mut summary_found = false;
    let mut phase_validate_found = false;
    let mut phase_scan_found = false;
    // GAP-CLI-DRY-01 (v1.1.8): dry-run never resolves a provider binary, so the
    // `validate` phase — which carries `binary_path` and `version` — must NOT be
    // emitted. Announcing a validation that never ran would make the envelope
    // lie. This test used to REQUIRE `validate` and had been red since v1.1.8.

    for line in &lines {
        let val: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("invalid NDJSON line: {e}\n{line}"));

        if val["summary"] == true {
            // Summary line
            assert_has_keys(
                "enrich-summary",
                &val,
                &[
                    "summary",
                    "operation",
                    "items_total",
                    "completed",
                    "failed",
                    "skipped",
                    "cost_usd",
                    "elapsed_ms",
                ],
            );
            assert_eq!(val["summary"], true);
            summary_found = true;
        } else if val.get("phase").is_some() {
            // Phase event
            let phase = val["phase"].as_str().unwrap_or("");
            match phase {
                "validate" => {
                    assert_has_keys("enrich-phase(validate)", &val, &["phase"]);
                    phase_validate_found = true;
                }
                "scan" => {
                    assert_has_keys("enrich-phase(scan)", &val, &["phase"]);
                    phase_scan_found = true;
                }
                _ => panic!("unexpected phase value: {phase}"),
            }
        } else if val.get("item").is_some() {
            // Item event (preview in dry-run)
            assert_has_keys("enrich-item", &val, &["item", "status", "index", "total"]);
            let status = val["status"].as_str().unwrap_or("");
            assert_eq!(
                status, "preview",
                "dry-run items must have status='preview'"
            );
        }
    }

    assert!(
        !phase_validate_found,
        "dry-run must NOT emit a 'validate' phase event: no provider binary is \
         resolved, so there is nothing validated to report (GAP-CLI-DRY-01)"
    );
    assert!(phase_scan_found, "enrich must emit a 'scan' phase event");
    assert!(summary_found, "enrich must emit a summary line");
}

// ---------------------------------------------------------------------------
// 39b — enrich requires --mode (GAP-HEADLESS-DEFAULT: no default provider)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_39b_enrich_requires_mode() {
    let env = Env::new();
    env.init();

    // GAP-HEADLESS-DEFAULT: a WRITE run that omits --mode must be rejected by
    // clap, never silently defaulting to a provider that spawns a headless CLI.
    // This is still enforced and is the case that matters — a write run is the
    // one that would actually spawn something.
    let out = env
        .cmd()
        .args(["enrich", "--operation", "memory-bindings"])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "enrich without --mode must fail; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "enrich without --mode must exit 2 (clap parsing error); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // GAP-CLI-DRY-01 (v1.1.8) carved out --dry-run: it is read-only and spawns
    // nothing, so clap accepts the omission there. This test used to run WITH
    // --dry-run and had been red since that change.
    //
    // Note the asymmetry is what makes the invariant hold: under --dry-run the
    // binary resolution is skipped before the mode is even inspected, so no
    // provider can be reached regardless of the fallback value.
    let dry = env
        .cmd()
        .args(["enrich", "--operation", "memory-bindings", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        dry.status.success(),
        "enrich --dry-run without --mode must succeed (GAP-CLI-DRY-01); stderr: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let dry_out = String::from_utf8_lossy(&dry.stdout);
    for line in dry_out.lines().filter(|l| !l.trim().is_empty()) {
        let val: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("invalid NDJSON line: {e}\n{line}"));
        assert!(
            val.get("binary_path").is_none(),
            "dry-run must never resolve a provider binary; offending line: {line}"
        );
    }
}
