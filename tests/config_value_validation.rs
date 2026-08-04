//! GAP-SG-200 / GAP-SG-201 / GAP-SG-202: what `config set` accepts, what it
//! refuses, and what `config get` says about a key that does not exist.
//!
//! Three defects with one root: the registry answered "does this key exist?"
//! and nothing answered "can this value work?".
//!
//! * `config set` stored ANY value for ANY known key with exit 0, so
//!   `embedding.dim nao-numero` and `llm.backend codex` persisted and only
//!   misbehaved on a later invocation, far from the command that caused them.
//! * `display.tz 0` was the extreme case: `tz::init` propagated the error and
//!   `main` runs it before dispatching, so the binary was bricked — including
//!   the `config unset display.tz` that would have undone it.
//! * `config get` answered `found:false` with exit 0 for an unknown key while
//!   `config set` answered exit 1 with a did-you-mean for the same string, so a
//!   typo read as "the key exists and is empty".
//!
//! The brick test below deliberately writes the bad value STRAIGHT INTO the
//! TOML rather than through `config set`. Going through the CLI would only
//! prove the validation works; planting it proves the degradation works too,
//! which is what protects a config file written before the validation existed.

#[path = "common/mod.rs"]
mod common;

/// Appends one raw setting to the sandbox config, bypassing `config set`.
///
/// Appends rather than overwrites: `isolated_env` already wrote the offline
/// OpenRouter stub into this file, and replacing it would break every command
/// the test then asserts on — for reasons that have nothing to do with the
/// defect under test.
fn plant_setting(env: &common::IsolatedEnv, key: &str, value: &str) {
    let path = env.config().join("config.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        std::fs::create_dir_all(env.config()).expect("config dir must be creatable");
        "schema_version = 1\nkeys = []\n\n[settings]\n".to_string()
    });

    // Insert immediately AFTER the `[settings]` header, never at end of file.
    // The sandbox config ends with a `[[keys]]` table, so appending would file
    // the setting under the API-key entry, where nothing reads it — the binary
    // then behaves as if the value had never been planted and the test passes
    // while proving nothing. That is precisely what happened on the first run.
    let mut out = String::with_capacity(text.len() + 64);
    let mut inserted = false;
    for line in text.lines() {
        out.push_str(line);
        out.push('\n');
        if !inserted && line.trim() == "[settings]" {
            out.push_str(&format!("\"{key}\" = \"{value}\"\n"));
            inserted = true;
        }
    }
    assert!(
        inserted,
        "sandbox config has no [settings] table to plant into:\n{text}"
    );
    std::fs::write(&path, out).expect("config.toml must be writable");
}

#[test]
fn a_value_outside_the_key_domain_is_refused() {
    let env = common::isolated_env();

    // One per ValueKind that has a checkable domain. `Text` keys are absent on
    // purpose: they accept anything by construction, so asserting on them would
    // pin behaviour the type does not promise.
    let rejected: &[(&str, &str)] = &[
        ("display.tz", "0"),                 // Tz
        ("embedding.dim", "nao-numero"),     // Unsigned
        ("system.max_load_per_ncpu", "abc"), // Float
        ("llm.slot_no_wait", "talvez"),      // Bool
        ("log.format", "xml"),               // OneOf
        ("i18n.lang", "klingon"),            // OneOf
        ("llm.backend", "codex"),            // OneOf, names a removed backend
        ("embedding.backend", "ollama"),     // OneOf
        ("network.chat_url", "nao-url"),     // Url
        ("log.level", "a=b=c=d"),            // LogDirective, broken syntax
    ];

    for (key, value) in rejected {
        let output = env
            .cmd()
            .args(["config", "set", key, value, "--json"])
            .output()
            .expect("config set must run");
        assert!(
            !output.status.success(),
            "`config set {key} {value}` succeeded. Storing a value no reader can \
             use turns the command into a delayed failure: the operator sees \
             exit 0 here and a broken binary later, with nothing connecting the \
             two."
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.contains(value),
            "the refusal for `{key}` must quote the offending value so the \
             operator can see what was rejected:\n{text}"
        );
    }
}

#[test]
fn a_value_inside_the_key_domain_is_stored() {
    let env = common::isolated_env();

    // The other half of the guard. Without it, a validator that rejects
    // EVERYTHING would pass the test above and break the product.
    let accepted: &[(&str, &str)] = &[
        ("display.tz", "America/Sao_Paulo"),
        ("embedding.dim", "1024"),
        ("system.max_load_per_ncpu", "2.5"),
        ("log.format", "json"),
        ("i18n.lang", "pt-BR"),
        ("llm.backend", "none"),
        ("embedding.backend", "openrouter"),
        (
            "network.chat_url",
            "https://openrouter.ai/api/v1/chat/completions",
        ),
        // Bare level and per-target directive: both are valid EnvFilter syntax.
        ("log.level", "warn"),
        ("log.level", "sqlite_graphrag=debug"),
        // GAP-SG-201 regression: the first version of `ValueKind::Bool` took
        // only `true|false`, and `--low-memory --help` has always advertised
        // `config set ingest.low_memory 1`. A validator narrower than the
        // documented spelling is a regression wearing a check's clothes.
        ("ingest.low_memory", "1"),
        ("ingest.low_memory", "true"),
        ("llm.slot_no_wait", "yes"),
        ("retry.disable", "on"),
    ];

    for (key, value) in accepted {
        env.cmd()
            .args(["config", "set", key, value, "--json"])
            .assert()
            .success();
    }
}

#[test]
fn an_invalid_timezone_already_in_the_config_does_not_brick_the_binary() {
    // GAP-SG-200. The value is planted directly in the TOML, standing in for a
    // config written before the validation above existed. Every command here
    // must still answer, ESPECIALLY the one that removes the bad key.
    let env = common::isolated_env();

    // The database has to exist BEFORE the bad value is planted: `health`
    // against a missing database exits 4 for its own reasons, and that would
    // make this test pass for the wrong one.
    let db = env.db().display().to_string();
    env.cmd()
        .args(["init", "--db", &db, "--json"])
        .assert()
        .success();

    plant_setting(&env, "display.tz", "0");

    // Sanity: without this the loop below can pass by never seeing the bad
    // value at all, which is exactly how the first version of this test
    // reported green while the brick was reintroduced on purpose.
    let planted = env
        .cmd()
        .args(["config", "get", "display.tz", "--json"])
        .output()
        .expect("config get must run");
    let planted: serde_json::Value =
        serde_json::from_slice(&planted.stdout).expect("config get must emit JSON");
    assert_eq!(
        planted["value"],
        serde_json::json!("0"),
        "the invalid timezone never reached the binary, so nothing below is \
         being tested"
    );

    for args in [
        vec!["config", "get", "display.tz", "--json"],
        vec!["config", "list", "--json"],
        vec!["config", "doctor", "--json"],
        vec!["health", "--db", &db, "--json"],
        vec!["list", "--db", &db, "--json"],
    ] {
        let output = env.cmd().args(&args).output().expect("command must run");
        assert!(
            output.status.success(),
            "`{}` exited {:?} with an invalid `display.tz` in the config. A knob \
             whose wrong value disables the command that fixes it is not a knob.\n{}",
            args.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // The recovery path itself.
    env.cmd()
        .args(["config", "unset", "display.tz", "--json"])
        .assert()
        .success();
}

#[test]
fn config_get_separates_an_unknown_key_from_an_empty_one() {
    // GAP-SG-202. `found:false` alone conflated the two, and the exit code
    // stays 0 for both because scripts probe presence with this command.
    let env = common::isolated_env();

    let unknown = env
        .cmd()
        .args(["config", "get", "display.tzz", "--json"])
        .output()
        .expect("config get must run");
    assert!(
        unknown.status.success(),
        "an unknown key must not fail the probe"
    );
    let unknown: serde_json::Value =
        serde_json::from_slice(&unknown.stdout).expect("config get must emit JSON");
    assert_eq!(unknown["known"], serde_json::json!(false));
    assert_eq!(
        unknown["suggestion"],
        serde_json::json!("display.tz"),
        "a near miss must name the key the operator meant; without it the \
         envelope reports the typo as a real key that happens to be empty"
    );

    let empty = env
        .cmd()
        .args(["config", "get", "llm.model", "--json"])
        .output()
        .expect("config get must run");
    let empty: serde_json::Value =
        serde_json::from_slice(&empty.stdout).expect("config get must emit JSON");
    assert_eq!(empty["found"], serde_json::json!(false));
    assert_eq!(
        empty["known"],
        serde_json::json!(true),
        "a real key with no stored value must be distinguishable from one that \
         does not exist — that distinction is the whole entry"
    );
}

#[test]
fn config_set_still_refuses_a_key_that_is_not_in_the_registry() {
    // The pre-existing guarantee this entry must not have weakened: value
    // validation runs AFTER key validation, so an unknown key still fails on
    // the key, with its own message.
    let env = common::isolated_env();
    let output = env
        .cmd()
        .args(["config", "set", "llm.concurrency", "4", "--json"])
        .output()
        .expect("config set must run");
    assert!(
        !output.status.success(),
        "`llm.concurrency` never existed; it must keep answering non-zero"
    );
}
