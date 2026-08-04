//! GAP-SG-141 residual (v1.2.4): re-embed dry-run reports a live backlog on an
//! isolated TempDir database — no monorepo job singleton, no network.

use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"));
    c.env_remove("OPENROUTER_API_KEY");
    c.env_remove("SQLITE_GRAPHRAG_API_KEY");
    c
}

#[test]
fn reembed_dry_run_reports_items_total_on_isolated_db() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t.sqlite");
    let init = bin()
        .args(["init", "--db"])
        .arg(&db)
        .arg("-q")
        .status()
        .expect("init");
    assert!(init.success(), "init must succeed");

    // Remember without embedding backend produces missing vectors.
    for i in 0..5 {
        let body = format!("body for memory {i} with enough text to store");
        let out = bin()
            .args(["remember", "--db"])
            .arg(&db)
            .args([
                "--name",
                &format!("m-{i}"),
                "--type",
                "note",
                "--description",
                &format!("desc {i}"),
                "--body",
                &body,
                "--llm-backend",
                "none",
                "--embedding-backend",
                "auto",
                "--skip-embedding-on-failure",
                "-q",
                "--no-input",
            ])
            .output()
            .expect("remember");
        assert!(
            out.status.success(),
            "remember {i}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let out = bin()
        .args(["enrich", "--db"])
        .arg(&db)
        .args([
            "--operation",
            "re-embed",
            "--target",
            "memories",
            "--dry-run",
            "--llm-backend",
            "none",
            "--embedding-backend",
            "auto",
            "-q",
            "--no-input",
            "--scan-page-size",
            "2",
        ])
        .output()
        .expect("enrich dry-run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "dry-run must succeed: stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.contains("items_total") || stdout.contains("\"total\""),
        "expected items_total in dry-run output: {stdout}"
    );
    // At least one preview / total > 0
    let has_positive = stdout.lines().any(|l| {
        l.contains("items_total")
            && (l.contains(":5")
                || l.contains(": 5")
                || l.contains("\"items_total\":5")
                || l.contains("\"items_total\": 5"))
            || (l.contains("\"status\":\"preview\"") || l.contains("\"status\": \"preview\""))
    });
    assert!(
        has_positive || stdout.matches("preview").count() >= 1,
        "expected non-zero re-embed backlog in dry-run: {stdout}"
    );
    eprintln!("GAP-SG-141 isolated dry-run stdout_len={}", stdout.len());
}
