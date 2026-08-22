//! G28: Reaper for orphan external processes.
//!
//! When the CLI crashes or is killed (SIGKILL, OOM, machine reset), child
//! processes spawned by `claude -p` or `codex exec` may be left running.
//! Without cleanup they accumulate as zombies that consume CPU, RAM, and
//! MCP-spawned subprocess trees (the 2026-06-03 incident: 1.877 processes
//! total, load average 276 on a 10-CPU host).
//!
//! [`crate::reaper::scan_and_kill_orphans`] walks the process table at startup and
//! terminates any invocation whose `PPID` is `1`
//! (reparented to `init`/`launchd` after the parent died) and that is
//! older than the `ORPHAN_MIN_AGE_SECS` constant. The scan is conservative: it only
//! kills processes that (a) match a known target name, AND (b) are
//! orphaned, AND (c) are older than the threshold. A short-lived CLI
//! that is just starting up is left alone.
//!
//! # Portability
//!
//! GAP-SG-261: the walk reads the process table through `sysinfo`, not through
//! `/proc`. The `/proc` implementation was gated on `#[cfg(unix)]`, which is
//! TRUE on macOS — a platform with no `/proc` — so there the very first
//! `read_dir` failed and the caller was told "no orphan subprocesses detected".
//! A verdict reported without a measurement is worse than an error, because it
//! reads like one.
//!
//! Only `terminate_pid` still splits by platform, because asking a process to
//! stop is where the systems genuinely differ: `SIGTERM` on Unix, and
//! `sysinfo`'s own request on Windows. The split is at that one call rather
//! than around the whole scan.

// GAP-SG-261: the constants used to be gated behind `cfg(unix)` because the
// scan itself was, and on Windows they would have been dead code. The scan is
// portable now, so the gate would be the thing making them dead.
const ORPHAN_MIN_AGE_SECS: u64 = 60;

const ORPHAN_SCAN_TARGETS: &[&str] = &["sqlite-graphrag"];

/// The PPID an orphan is reparented to once its parent dies.
///
/// `1` is `init` on Linux and `launchd` on macOS. Windows has no reparenting
/// contract of this shape, which is why [`orphan_pids`] answers with an empty
/// set there rather than pretending otherwise.
const REPARENTED_PPID: u32 = 1;

/// Reaper report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaperReport {
    /// Number of orphan processes detected.
    pub found: usize,
    /// Number of orphan processes successfully terminated.
    pub killed: usize,
    /// Number that we could not terminate (permission, ESRCH, etc).
    pub failed: usize,
    /// Elapsed wall time of the scan.
    pub elapsed_ms: u64,
}

/// Walks the process table and kills orphan LLM invocations.
///
/// The scan is best-effort and never panics: on any unexpected error it
/// logs the failure and returns a report with `killed = 0`.
pub fn scan_and_kill_orphans() -> ReaperReport {
    let start = std::time::Instant::now();
    let mut report = ReaperReport {
        found: 0,
        killed: 0,
        failed: 0,
        elapsed_ms: 0,
    };

    for (pid, name) in orphan_pids(ORPHAN_MIN_AGE_SECS) {
        report.found += 1;
        match terminate_pid(pid) {
            Ok(()) => {
                report.killed += 1;
                tracing::info!(target: "reaper", pid, comm = %name, "killed orphan LLM subprocess");
            }
            Err(e) => {
                report.failed += 1;
                tracing::warn!(target: "reaper", pid, comm = %name, error = %e, "failed to kill orphan");
            }
        }
    }

    let max = crate::llm_slots::default_max_concurrency();
    let stale = crate::llm_slots::find_stale_slots(max);
    for slot_id in &stale {
        let _ = crate::llm_slots::force_release(*slot_id);
        tracing::info!(target: "reaper", slot_id, "released stale LLM slot (PID dead)");
    }

    report.elapsed_ms = start.elapsed().as_millis() as u64;
    if report.killed > 0 {
        tracing::warn!(
            target: "reaper",
            found = report.found,
            killed = report.killed,
            failed = report.failed,
            "reaped orphan LLM subprocesses"
        );
    } else {
        tracing::info!(target: "reaper", found = report.found, "no orphan LLM subprocesses detected");
    }
    report
}

/// Every PID that matches a scan target, is reparented, and is old enough.
///
/// GAP-SG-261: this used to read `/proc` under `#[cfg(unix)]`. That gate is
/// WRONG for macOS, which is `unix` and has no `/proc`, so `read_dir` failed at
/// the first call and the reaper reported zero orphans on a host it had never
/// actually looked at. Reporting "no orphans detected" without having read the
/// process table is worse than reporting an error, because the log line reads
/// like a measurement.
///
/// `sysinfo` is already a dependency of this crate — `system_load` and
/// `llm_slots` use it — and it is pure Rust, so the portable path costs no new
/// crate and no C toolchain. It also replaces a heuristic with the real value:
/// process age came from the mtime of `/proc/<pid>/stat`, which is a proxy,
/// while [`sysinfo::Process::run_time`] is the elapsed running time itself.
///
/// Returns an empty vector rather than an error when nothing matches, so the
/// caller cannot tell "scan failed" from "scan found nothing" by accident —
/// that distinction lives in the `Result` this does not return, and the scan is
/// documented as best-effort.
fn orphan_pids(min_age_secs: u64) -> Vec<(u32, String)> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

    // Only the process list is refreshed: the reaper never asks about CPU,
    // memory or disk, and refreshing everything would walk data this function
    // discards on every host it runs on.
    let mut system =
        System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
    system.refresh_processes(ProcessesToUpdate::All, true);

    let own_pid = std::process::id();
    let mut out = Vec::new();
    for (pid, process) in system.processes() {
        let pid = pid.as_u32();
        if pid == own_pid {
            continue;
        }
        // Reparented to init/launchd is what makes a process an ORPHAN rather
        // than a peer someone is still supervising.
        if process.parent().map(sysinfo::Pid::as_u32) != Some(REPARENTED_PPID) {
            continue;
        }
        let name = process.name().to_string_lossy().to_string();
        if !ORPHAN_SCAN_TARGETS.iter().any(|target| name == *target) {
            continue;
        }
        // Never race a peer that just started. The threshold is the safety
        // margin, not an optimisation.
        if process.run_time() < min_age_secs {
            continue;
        }
        out.push((pid, name));
    }
    out
}

/// Asks one process to terminate.
///
/// Unix sends `SIGTERM` and returns without waiting: a follow-up sweep can
/// escalate to `SIGKILL` if the process ignores it. Windows has no signal of
/// this shape, and `sysinfo::Process::kill` is the portable request there —
/// which is why the platform split lives HERE, at the one call that genuinely
/// differs, instead of around the whole scan as it used to.
fn terminate_pid(pid: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        // SAFETY: `kill` with a PID this scan just read from the process table
        // and a constant signal number. It cannot violate memory safety; the
        // worst outcome is `ESRCH` for a process that exited in between, which
        // is returned as an error rather than ignored.
        let rc = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if rc == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(unix))]
    {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );
        system.refresh_processes(ProcessesToUpdate::All, true);
        match system.process(sysinfo::Pid::from_u32(pid)) {
            Some(process) if process.kill() => Ok(()),
            Some(_) => Err(std::io::Error::other("the platform refused the request")),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "the process exited before the request reached it",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaper_report_starts_zeroed() {
        let r = ReaperReport {
            found: 0,
            killed: 0,
            failed: 0,
            elapsed_ms: 0,
        };
        assert_eq!(r.found, 0);
        assert_eq!(r.killed, 0);
        assert_eq!(r.failed, 0);
    }

    #[test]
    fn orphan_min_age_is_one_minute() {
        // G28: the threshold of 60s is the safety margin that prevents
        // a CLI invocation from killing a concurrent peer that just
        // started 5s ago.
        assert_eq!(ORPHAN_MIN_AGE_SECS, 60);
    }

    #[test]
    fn orphan_targets_include_sqlite_graphrag() {
        assert!(ORPHAN_SCAN_TARGETS.contains(&"sqlite-graphrag"));
    }

    #[test]
    fn scan_completes_without_panic() {
        // Just ensure the function returns a ReaperReport on the test host.
        // In containers we may be PID 1; the report will simply have found=0.
        let r = scan_and_kill_orphans();
        assert!(r.elapsed_ms < 30_000, "scan must finish in <30s");
    }

    #[test]
    fn the_scan_reads_the_process_table_without_proc() {
        // GAP-SG-261. The previous implementation opened `/proc` under
        // `#[cfg(unix)]`, which is true on macOS, where `/proc` does not exist
        // — so the scan failed at its first call and the caller was told "no
        // orphans detected". This asserts the portable path instead: the table
        // is read through `sysinfo`, and the answer is a real measurement on
        // every platform this crate builds for.
        //
        // The invariant that survives a host with no orphans: the scan must
        // SEE processes. A run that enumerates nothing would satisfy any
        // assertion about the result being empty, which is exactly the blindness
        // the old gate produced.
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );
        system.refresh_processes(ProcessesToUpdate::All, true);
        assert!(
            !system.processes().is_empty(),
            "the process table must be readable, or the reaper reports a verdict \
             it never measured"
        );

        // This process is in the table and is NOT a candidate: it is neither
        // reparented nor foreign, so the filter must exclude it.
        let own = std::process::id();
        assert!(
            !orphan_pids(0).iter().any(|(pid, _)| *pid == own),
            "the scan must never target the running process, at any age threshold"
        );
    }
}
