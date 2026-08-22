//! GAP-SG-283: the entity type vocabulary policy `enrich` was missing.
//!
//! `remember` has `--strict-entity-types` and `link` has `--strict-relations`.
//! `enrich` had neither, and it is the channel that writes type labels in
//! VOLUME: `entity-type-validate` persisted whatever the model returned, with
//! V017 having removed the SQL `CHECK` that used to stand in the way.
//!
//! These assertions are deliberately CONTRACT assertions, driven through the
//! built binary: they establish that a consumer can learn the value set before
//! asking, that the default is the compatible one, and that an unknown value is
//! refused. The decision logic itself is unit-tested next to the code, in
//! `src/commands/enrich/events/entity_type_policy.rs`, because exercising it
//! from here would mean calling the provider — real money for a fact a pure
//! function already proves.

/// Renders `enrich --help` from the built binary.
///
/// The RENDER is what an operator copies, not the doc comment: clap rewraps,
/// renames and hides text on its way to the terminal.
fn enrich_help() -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"))
        .args(["enrich", "--help"])
        .output()
        .expect("failed to run the built binary");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

/// Runs `enrich` with `argv` and returns the exit code.
fn exit_code(argv: &[&str]) -> i32 {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"))
        .args(argv)
        .output()
        .expect("failed to run the built binary");
    output.status.code().unwrap_or(-1)
}

/// Predictable contract: the consumer learns the value set from `--help`.
#[test]
fn help_publishes_both_flags_and_all_three_policy_values() {
    let help = enrich_help();
    assert!(
        help.contains("--allowed-types"),
        "--allowed-types is absent from enrich --help"
    );
    assert!(
        help.contains("--on-unknown-type"),
        "--on-unknown-type is absent from enrich --help"
    );
    for value in ["keep", "fallback", "strict"] {
        assert!(
            help.contains(value),
            "enrich --help never names the `{value}` policy, so a consumer has to \
             guess the value set"
        );
    }
}

/// Operator control: the policy is a project choice, so both precedence layers
/// have to be named where the operator looks.
#[test]
fn help_names_the_xdg_key_behind_each_flag() {
    let help = enrich_help();
    for key in [
        "enrich.entity_type.allowed_types",
        "enrich.entity_type.on_unknown_type",
    ] {
        assert!(
            help.contains(key),
            "enrich --help does not name `{key}`, so the XDG layer is invisible"
        );
    }
}

/// Safe default: `keep` is what an omitted flag resolves to, and `keep` is the
/// v1.2.8 behaviour byte for byte.
#[test]
fn help_declares_keep_as_the_default_policy() {
    let help = enrich_help();
    // Split on the DEFINITION line, not on a bare mention: `--allowed-types`
    // names the sibling flag in its own prose, so the first occurrence of the
    // bare name lands inside the wrong flag's block.
    let block = help
        .split("--on-unknown-type <POLICY>")
        .nth(1)
        .expect("--on-unknown-type must appear in the rendered help");
    assert!(
        block.contains("keep"),
        "the --on-unknown-type help block must state that `keep` is the default: {block}"
    );
}

/// An unknown policy name is refused at parse time, before the database is
/// opened and long before a token is spent.
#[test]
fn an_unknown_policy_value_is_refused_by_the_parser() {
    let code = exit_code(&[
        "enrich",
        "--operation",
        "entity-type-validate",
        "--mode",
        "openrouter",
        "--on-unknown-type",
        "maybe",
    ]);
    assert_ne!(
        code, 0,
        "`--on-unknown-type maybe` must be refused; a policy the product does \
         not implement cannot be accepted silently"
    );
}

/// Compatibility: the previous argv keeps parsing. This is the guarantee that
/// bounds the whole change — an existing caller passes neither flag and must be
/// unaffected, so `--dry-run` (which spends nothing) still reaches the run.
#[test]
fn the_previous_argv_still_parses_without_either_flag() {
    let code = exit_code(&[
        "enrich",
        "--operation",
        "entity-type-validate",
        "--dry-run",
        "--print-schema",
    ]);
    assert_eq!(
        code, 0,
        "an invocation carrying neither new flag must behave exactly as it did \
         in v1.2.8"
    );
}

/// Observability: both keys are settable, so the operator can measure and
/// change the policy without a source edit.
#[test]
fn both_keys_are_declared_in_the_setting_registry() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/registry.rs");
    let source = std::fs::read_to_string(&path).expect("registry must be readable");
    for key in [
        "enrich.entity_type.allowed_types",
        "enrich.entity_type.on_unknown_type",
    ] {
        assert!(
            source.contains(&format!("key: \"{key}\"")),
            "`{key}` is read at runtime but absent from SETTING_KEYS, so \
             `config set` answers exit 1 for a key the product resolves"
        );
    }
}
