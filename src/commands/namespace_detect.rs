//! Handler for the `namespace-detect` CLI subcommand.

use crate::errors::AppError;
use crate::namespace;
use crate::output;
use serde::Serialize;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Resolve namespace using current environment and cwd\n  \
    sqlite-graphrag namespace-detect\n\n  \
    # Override with an explicit namespace flag\n  \
    sqlite-graphrag namespace-detect --namespace my-project\n\n  \
    # Explicit namespace flag\n  \
    sqlite-graphrag namespace-detect --namespace ci-runner")]
/// Namespace detect args.
pub struct NamespaceDetectArgs {
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Explicit database path. Accepted as a no-op to preserve the global contract.
    #[arg(long)]
    pub db: Option<String>,
    /// Explicit JSON flag. Accepted as a no-op because output is already JSON by default.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Serialize)]
struct NamespaceDetectResponse {
    namespace: String,
    source: namespace::NamespaceSource,
    cwd: String,
    /// Total execution time in milliseconds from handler start to serialisation.
    elapsed_ms: u64,
}

/// Run.
pub fn run(args: NamespaceDetectArgs) -> Result<(), AppError> {
    let inicio = std::time::Instant::now();
    let _ = args.db;
    let _ = args.json; // --json is a no-op because output is already JSON by default
    let resolution = namespace::detect_namespace(args.namespace.as_deref())?;
    output::emit_json(&NamespaceDetectResponse {
        namespace: resolution.namespace,
        source: resolution.source,
        cwd: resolution.cwd,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::namespace::NamespaceSource;
    use clap::Parser;
    use serial_test::serial;

    #[test]
    #[serial]
    fn namespace_detect_default_returns_global_via_detect() {
        // Without --namespace and without XDG namespace.default, default is "global".
        // Product env is not read (G-T-XDG-04).
        let resolution = namespace::detect_namespace(None).unwrap();
        // Host may have XDG namespace.default; accept Default or XdgConfig.
        assert!(!resolution.namespace.is_empty());
        if resolution.source == NamespaceSource::Default {
            assert_eq!(resolution.namespace, "global");
        }
    }

    #[test]
    #[serial]
    fn namespace_detect_explicit_flag_wins() {
        // GAP-SG-131: product env is not a config channel. Flag always wins.
        let resolution = namespace::detect_namespace(Some("flag-namespace")).unwrap();
        assert_eq!(resolution.namespace, "flag-namespace");
        assert_eq!(resolution.source, NamespaceSource::ExplicitFlag);
    }

    #[test]
    #[serial]
    fn namespace_detect_default_when_no_flag() {
        // G-T-XDG-04: product env is not read; without --namespace / XDG default,
        // resolution falls back to "global" (or XDG namespace.default if set on host).
        let resolution = namespace::detect_namespace(None).unwrap();
        assert!(!resolution.namespace.is_empty());
        assert!(
            matches!(
                resolution.source,
                NamespaceSource::Default | NamespaceSource::XdgConfig
            ),
            "unexpected source {:?}",
            resolution.source
        );
    }

    #[test]
    fn namespace_detect_response_serializes_all_fields() {
        let resp = NamespaceDetectResponse {
            namespace: "meu-projeto".to_string(),
            source: NamespaceSource::ExplicitFlag,
            cwd: "/home/usuario/projeto".to_string(),
            elapsed_ms: 3,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["namespace"], "meu-projeto");
        assert_eq!(json["source"], "explicit_flag");
        assert!(json["cwd"].is_string());
        assert_eq!(json["elapsed_ms"], 3);
    }

    #[test]
    fn namespace_source_serializes_in_snake_case() {
        let cases = vec![
            (NamespaceSource::ExplicitFlag, "explicit_flag"),
            (NamespaceSource::XdgConfig, "xdg_config"),
            (NamespaceSource::Default, "default"),
        ];
        for (source, expected) in cases {
            let json = serde_json::to_value(source).unwrap();
            assert_eq!(
                json, expected,
                "NamespaceSource::{source:?} must serialize as \"{expected}\""
            );
        }
    }

    #[test]
    fn namespace_detect_accepts_db_as_noop() {
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "namespace-detect",
            "--db",
            "/tmp/graphrag.sqlite",
        ])
        .expect("namespace-detect must accept --db as a no-op");

        match cli.command {
            Some(crate::cli::Commands::NamespaceDetect(args)) => {
                assert_eq!(args.db.as_deref(), Some("/tmp/graphrag.sqlite"));
            }
            _ => unreachable!("unexpected command parsed"),
        }
    }
}
