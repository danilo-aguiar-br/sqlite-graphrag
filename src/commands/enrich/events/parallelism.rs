//! Drain fan-out sizing and its concurrency warnings (G19/G28-D/G34).
//!
//! Resolves how many workers the drain runs with, reading `--rest-concurrency`
//! under `--mode openrouter`.

use super::super::args::EnrichArgs;

/// Resolve drain worker count and emit concurrency warnings (G19/G28-D/G34).
pub(crate) fn resolve_drain_parallelism(args: &EnrichArgs) -> usize {
    if args.llm_parallelism.is_some() {
        tracing::warn!(
            target: "enrich",
            effective_source = "rest_concurrency",
            "--llm-parallelism is inert with --mode openrouter; \
             use --rest-concurrency to size the REST drain fan-out"
        );
    }
    let parallelism = args.rest_concurrency.clamp(1, 16) as usize;
    tracing::info!(
        target: "enrich",
        concurrency = parallelism,
        source = "rest_concurrency",
        "OpenRouter REST concurrency (clamp 1..=16)"
    );
    if parallelism > 1 {
        tracing::info!(
            target: "enrich",
            concurrency = parallelism,
            source = "rest_concurrency",
            "parallel LLM processing with bounded thread pool"
        );
    }
    parallelism
}

#[cfg(test)]
mod parallelism_tests {
    use super::*;
    use crate::constants::DEFAULT_ENRICH_REST_CONCURRENCY;
    use clap::{Args as _, Parser};

    /// Minimal test harness: `EnrichArgs` derives `clap::Args`, so parsing a
    /// synthetic argv is the only way to build one without a `Default` impl.
    #[derive(Parser)]
    struct Harness {
        #[command(flatten)]
        enrich: EnrichArgs,
    }

    fn parse(argv: &[&str]) -> EnrichArgs {
        let mut full = vec!["enrich"];
        full.extend_from_slice(argv);
        Harness::parse_from(full).enrich
    }

    #[test]
    fn help_documents_llm_parallelism_inertia_under_openrouter() {
        let mut cmd = EnrichArgs::augment_args(clap::Command::new("enrich"));
        let help = cmd.render_long_help().to_string();
        // Isolate the flag's OWN help block (the definition line), not a
        // mention of it inside another flag's description.
        let flag = help
            .split("--llm-parallelism <N>")
            .nth(1)
            .expect("--llm-parallelism must appear in the rendered help");
        // The help text for the flag must state that it does nothing under
        // `--mode openrouter`; hooks copied the old (wrong) promise verbatim.
        assert!(
            flag.to_ascii_lowercase().contains("inert"),
            "--llm-parallelism help must declare it is INERT with --mode openrouter, got: {flag}"
        );
        assert!(
            flag.contains("--rest-concurrency"),
            "--llm-parallelism help must redirect to --rest-concurrency, got: {flag}"
        );
        // GAP-SG-141: no corpus-specific literal may leak into public help.
        assert!(
            !help.contains("2321"),
            "public help must not embed a corpus-specific entity count"
        );
    }

    #[test]
    fn openrouter_branch_sizes_drain_from_rest_concurrency() {
        // Default: the named constant, not a magic `unwrap_or`.
        let args = parse(&["--operation", "memory-bindings", "--mode", "openrouter"]);
        assert_eq!(
            resolve_drain_parallelism(&args),
            DEFAULT_ENRICH_REST_CONCURRENCY as usize
        );

        // Explicit --rest-concurrency wins.
        let args = parse(&[
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
            "--rest-concurrency",
            "3",
        ]);
        assert_eq!(resolve_drain_parallelism(&args), 3);

        // --llm-parallelism is inert here: it must NOT change the fan-out.
        let args = parse(&[
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
            "--rest-concurrency",
            "3",
            "--llm-parallelism",
            "16",
        ]);
        assert_eq!(resolve_drain_parallelism(&args), 3);
    }
}
