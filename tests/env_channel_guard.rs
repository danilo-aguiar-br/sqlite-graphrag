//! The product never reads configuration from the environment, and no text it
//! shows an operator may claim otherwise.
//!
//! `src/config/api_keys.rs` already proves the *behaviour*: `resolve_api_key`
//! ignores `OPENROUTER_API_KEY` even when it is set. What went unguarded was the
//! *text*. Three separate user-facing strings kept advertising that variable as
//! a valid channel — one in an error suggestion, one in a startup failure, one
//! in a `--help` doc comment — so an operator following the product's own
//! instructions would export a variable that does nothing, and a script author
//! reading `--help` would bake that dead channel into a wrapper.
//!
//! An earlier round closed one of the three and called the class done. That is
//! the mistake this file exists to make impossible: it walks the WHOLE help tree
//! rather than sampling a command, because a class is only closed when the sweep
//! is exhaustive.
//!
//! # GAP-SG-232: the sweep had to reach the source too
//!
//! Rendered text was only half of it. The other half is the SOURCE: several
//! embedded test modules kept clearing `SQLITE_GRAPHRAG_*` before an assertion,
//! as hygiene against a channel that no longer exists. Nothing read those
//! variables, so every one of those calls was a no-op — and a reader who found
//! `remove_var("SQLITE_GRAPHRAG_DISABLE_RETRY")` beside `is_kill_switch_active`
//! learnt, reasonably and wrongly, that the variable steers the switch. The two
//! sweeps here are therefore complementary: one walks the help tree, the other
//! walks `src/`, and neither can close the class alone.

use clap::CommandFactory;
use std::path::{Path, PathBuf};

/// Environment-variable names the product must never offer as a channel.
///
/// `SQLITE_GRAPHRAG_` is matched as a prefix because the family is what is
/// banned, not any single member. Constants share that prefix
/// (`SQLITE_GRAPHRAG_VERSION`), which is exactly why this guard reads rendered
/// help and messages rather than grepping the source.
const BANNED_CHANNELS: &[&str] = &["OPENROUTER_API_KEY", "SQLITE_GRAPHRAG_"];

/// Phrases that offer the environment as a configuration channel.
///
/// Kept separate from [`BANNED_CHANNELS`] because a text can promise the channel
/// without naming a variable ("falls back to the environment").
const BANNED_PHRASES: &[&str] = &[
    "env var",
    "environment variable",
    "variável de ambiente",
    "variavel de ambiente",
];

/// Collects the rendered long help of a command and every subcommand it owns.
fn help_texts(cmd: &mut clap::Command, path: String, out: &mut Vec<(String, String)>) {
    out.push((path.clone(), cmd.render_long_help().to_string()));
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in names {
        let child_path = format!("{path} {name}");
        if let Some(child) = cmd.find_subcommand_mut(&name) {
            help_texts(child, child_path, out);
        }
    }
}

/// Markers that turn a mention into a denial rather than an offer.
///
/// Naming the environment in order to rule it out is exactly what the product
/// should do — `--help` says "Product environment variables are not read at
/// runtime" on purpose. A guard that cannot tell an offer from a denial would
/// punish the correct text and pressure the next author into deleting the
/// warning instead of the promise.
const DENIAL_MARKERS: &[&str] = &[
    "not read",
    "never read",
    "no environment variable",
    "no product env",
    "must not be used",
    "não é lida",
    "nao e lida",
    "nenhuma variável de ambiente",
    "nenhuma variavel de ambiente",
];

/// Reports every banned token that is OFFERED rather than denied.
///
/// Works sentence by sentence: a paragraph may legitimately deny the channel in
/// one sentence and discuss precedence in the next, and only the sentence
/// carrying the mention decides.
fn offences(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for sentence in text.split(['.', '\n']) {
        let lowered = sentence.to_lowercase();
        if DENIAL_MARKERS.iter().any(|m| lowered.contains(m)) {
            continue;
        }
        for needle in BANNED_CHANNELS {
            if sentence.contains(needle) && !found.contains(needle) {
                found.push(*needle);
            }
        }
        for needle in BANNED_PHRASES {
            if lowered.contains(&needle.to_lowercase()) && !found.contains(needle) {
                found.push(*needle);
            }
        }
    }
    found
}

#[test]
fn the_guard_tells_an_offer_from_a_denial() {
    // Without this the guard is worthless in both directions: it would flag the
    // warning the product is supposed to print, and an author would "fix" it by
    // deleting the warning.
    assert!(
        offences("Product environment variables are not read at runtime.").is_empty(),
        "denying the channel must not count as offering it"
    );
    assert!(
        offences("No environment variable supplies this value.").is_empty(),
        "an explicit denial must pass"
    );
    assert_eq!(
        offences("Falls back to OPENROUTER_API_KEY env var."),
        vec!["OPENROUTER_API_KEY", "env var"],
        "an offer must be reported, naming every token that made it one"
    );
}

/// GAP-SG-232: source files allowed to write a product variable, with reasons.
///
/// One entry, argued rather than assumed. An allowlist that grows by habit is
/// how the class reopens.
const SOURCE_EXEMPT: &[(&str, &str)] = &[(
    "src/config/api_keys.rs",
    "resolve_api_key_ignores_product_env proves OPENROUTER_API_KEY is IGNORED, \
     and the only way to prove a variable is ignored is to set it and observe \
     that nothing moved. Deleting the write would delete the proof.",
)];

/// Functions that WRITE the process environment.
///
/// Reading is not the offence and is not searched for: `std::env::var` over
/// `LANG` is how the POSIX locale is resolved, and a guard that flagged it would
/// be flagging correct code. Writing a product variable is the offence, because
/// the only reason to write one is to influence a reader that does not exist.
const ENV_WRITERS: &[&str] = &["set_var(", "remove_var("];

/// Source with `//` comments and all whitespace removed, plus a byte-to-line map.
///
/// Both steps are load-bearing, and both are borrowed from the sibling guard in
/// `src/i18n/tests.rs`. Comments go because this very file, and the comments
/// that replaced the removed calls, name the banned variables in prose; a
/// literal search would flag the text that documents the fix. Whitespace goes
/// because rustfmt splits a call the moment its arguments grow, and a guard that
/// only matches the one-line shape passes by not looking.
struct Normalized {
    text: String,
    line_of_byte: Vec<usize>,
}

fn normalize(source: &str) -> Normalized {
    let mut text = String::with_capacity(source.len());
    let mut line_of_byte = Vec::with_capacity(source.len());
    for (idx, raw) in source.lines().enumerate() {
        let code = match raw.find("//") {
            Some(pos) => &raw[..pos],
            None => raw,
        };
        for ch in code.chars().filter(|c| !c.is_whitespace()) {
            text.push(ch);
            line_of_byte.resize(text.len(), idx + 1);
        }
    }
    Normalized { text, line_of_byte }
}

/// GAP-SG-232: every write of a PRODUCT environment variable, as `(line, name)`.
///
/// A system channel is not an offence and must not be reported: `XDG_RUNTIME_DIR`
/// is a real contract with the operating system, and `LC_ALL`, `LANG` and
/// `NO_COLOR` are read by code the product does not own. Only a name matching
/// [`BANNED_CHANNELS`] counts, which is the same vocabulary the text sweeps use.
///
/// # Declared blind spot
///
/// Only a LITERAL argument is judged. A name held in a `const` reaches
/// `set_var` as an identifier and is invisible here —
/// `src/commands/ingest_tests.rs` writes `SQLITE_GRAPHRAG_LOW_MEMORY` exactly
/// that way, through `RETIRED_LOW_MEMORY_ENV`, and this gate does not see it.
/// The hole is written down rather than papered over: resolving constants would
/// mean grepping the source for the family prefix, which the module docs above
/// rule out because `SQLITE_GRAPHRAG_VERSION` and its siblings share it. Closing
/// it needs the call deleted, not the detector widened.
fn offending_env_writes(source: &str) -> Vec<(usize, String)> {
    let normalized = normalize(source);
    let mut found = Vec::new();
    for writer in ENV_WRITERS {
        let mut from = 0;
        while let Some(hit) = normalized.text[from..].find(writer) {
            let at = from + hit;
            let after = at + writer.len();
            from = after;
            // Only a literal argument is judged. A name held in a constant is
            // out of reach here and is covered by the help and message sweeps.
            let rest = &normalized.text[after..];
            let Some(body) = rest.strip_prefix('"') else {
                continue;
            };
            let Some(end) = body.find('"') else {
                continue;
            };
            let name = &body[..end];
            let banned = BANNED_CHANNELS.iter().any(|needle| {
                // `SQLITE_GRAPHRAG_` is a family, so it matches as a prefix;
                // `OPENROUTER_API_KEY` is one name and matches whole.
                if needle.ends_with('_') {
                    name.starts_with(needle)
                } else {
                    name == *needle
                }
            });
            if banned {
                let line = normalized.line_of_byte.get(at).copied().unwrap_or(0);
                found.push((line, name.to_string()));
            }
        }
    }
    found.sort();
    found
}

/// Every `.rs` file under `root`, recursively.
fn rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Repo-relative path with forward slashes, so the message reads the same
/// on every platform.
fn relative(path: &Path, repo: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn the_source_detector_tells_a_product_channel_from_a_system_one() {
    // Without this the gate can pass by not looking, which is exactly how the
    // manipulation inside embedded test modules survived the text sweeps.
    let offence = "std::env::remove_var(\"SQLITE_GRAPHRAG_DISABLE_RETRY\");";
    assert_eq!(
        offending_env_writes(offence),
        vec![(1, "SQLITE_GRAPHRAG_DISABLE_RETRY".to_string())],
        "a write to a product variable must be reported, with its line"
    );
    assert_eq!(
        offending_env_writes("std::env::set_var(\"OPENROUTER_API_KEY\", \"sk-x\");"),
        vec![(1, "OPENROUTER_API_KEY".to_string())],
        "the API key is one name, matched whole"
    );
    assert!(
        offending_env_writes("std::env::set_var(\"XDG_RUNTIME_DIR\", dir);").is_empty(),
        "a system channel must not be flagged: XDG_RUNTIME_DIR is a real contract"
    );
    assert!(
        offending_env_writes(
            "std::env::set_var(\"LC_ALL\", \"C\");\nstd::env::remove_var(\"NO_COLOR\");"
        )
        .is_empty(),
        "POSIX locale and terminal channels are read by code the product does not own"
    );
    assert!(
        offending_env_writes("// std::env::set_var(\"SQLITE_GRAPHRAG_LANG\", \"pt\");").is_empty(),
        "prose documenting the banned call must not trip the gate"
    );
    assert_eq!(
        offending_env_writes(
            "std::env::set_var(\n    \"SQLITE_GRAPHRAG_EMBEDDING_DIM\",\n    \"384\",\n);"
        )
        .len(),
        1,
        "a rustfmt-split call must still be caught"
    );
}

#[test]
fn no_source_file_manipulates_a_product_environment_variable() {
    let repo = repo_root();
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);

    assert!(
        files.len() > 100,
        "the scan found {} files, which is too few to be the real tree — the \
         walk is broken and this gate is passing on an empty set",
        files.len()
    );

    let mut offences = Vec::new();
    for path in &files {
        let rel = relative(path, &repo);
        if SOURCE_EXEMPT.iter().any(|(name, _)| *name == rel) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        for (line, name) in offending_env_writes(&source) {
            offences.push(format!("{rel}:{line} writes {name}"));
        }
    }
    offences.sort();

    assert!(
        offences.is_empty(),
        "GAP-SG-232: source code writes an environment variable the product never \
         reads. \
         Configuration precedence is: CLI flag, then the XDG key via `config \
         set`, then the compiled default — no product environment variable takes \
         part at any layer, so setting or clearing one changes nothing and \
         teaches the next reader that it does. Delete the call; if the test \
         relied on it, the premise it needed comes from the XDG key or from the \
         process-wide setter the code really uses.\n{}",
        offences.join("\n")
    );
}

#[test]
fn every_source_exemption_names_a_file_that_still_offends() {
    // An exemption for a file that no longer writes the variable is worse than
    // none: it silently covers whatever that path does next.
    let repo = repo_root();
    for (name, reason) in SOURCE_EXEMPT {
        assert!(
            reason.len() > 40,
            "the exemption for `{name}` has no real justification: {reason:?}"
        );
        let path = repo.join(name);
        assert!(
            path.is_file(),
            "SOURCE_EXEMPT names `{name}`, which is gone"
        );
        let source = std::fs::read_to_string(&path).expect("exempt file must be readable");
        assert!(
            !offending_env_writes(&source).is_empty(),
            "`{name}` is exempt but writes no product variable any more; delete the entry"
        );
    }
}

#[test]
fn no_help_text_anywhere_offers_the_environment_as_a_channel() {
    let mut root = sqlite_graphrag::cli::Cli::command();
    let mut rendered = Vec::new();
    help_texts(&mut root, "sqlite-graphrag".to_string(), &mut rendered);

    assert!(
        rendered.len() > 30,
        "the walk collected only {} help pages, which means it stopped short of \
         the real subcommand tree and would pass by not looking",
        rendered.len()
    );

    let mut failures = Vec::new();
    for (path, text) in &rendered {
        let found = offences(text);
        if !found.is_empty() {
            failures.push(format!("{path}: {found:?}"));
        }
    }

    assert!(
        failures.is_empty(),
        "help text offers environment variables as a configuration channel, but \
         the product never reads one. Point the operator at `config add-key \
         --from-stdin` or the equivalent flag instead.\n{}",
        failures.join("\n")
    );
}

#[test]
fn no_error_message_or_suggestion_offers_the_environment_as_a_channel() {
    use sqlite_graphrag::errors::AppError;
    use sqlite_graphrag::i18n::Language;

    // One instance per variant that carries a suggestion, plus the ambiguous
    // ones, so both halves of the envelope are covered in both languages.
    let samples: Vec<AppError> = vec![
        AppError::Validation("bad".into()),
        AppError::Duplicate("dup".into()),
        AppError::Conflict("stale".into()),
        AppError::NotFound("missing".into()),
        AppError::NamespaceError("ns".into()),
        AppError::LimitExceeded("cap".into()),
        AppError::Embedding("embed".into()),
        AppError::DbBusy("busy".into()),
        AppError::LockBusy("held".into()),
        AppError::VecExtension("vec".into()),
    ];

    let mut failures = Vec::new();
    for err in &samples {
        for lang in [Language::English, Language::Portuguese] {
            let message = err.localized_message_for(lang);
            let found = offences(&message);
            if !found.is_empty() {
                failures.push(format!("message/{lang:?}/{err:?}: {found:?}"));
            }
            if let Some(hint) = err.suggestion_for(lang) {
                let found = offences(hint);
                if !found.is_empty() {
                    failures.push(format!("suggestion/{lang:?}/{err:?}: {found:?}"));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "an error envelope advertises an environment variable the product never \
         reads:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_suggestion_is_translated_not_merely_present() {
    use sqlite_graphrag::errors::AppError;
    use sqlite_graphrag::i18n::Language;

    // A `suggestion` that renders identically in both languages is the defect
    // this asserts against: the envelope used to ship a Portuguese `message`
    // beside an English `suggestion`, which is worse than either alone because
    // it reads as a partial translation the operator cannot trust.
    let samples: Vec<AppError> = vec![
        AppError::Validation("bad".into()),
        AppError::Duplicate("dup".into()),
        AppError::Conflict("stale".into()),
        AppError::NotFound("missing".into()),
        AppError::NamespaceError("ns".into()),
        AppError::LimitExceeded("cap".into()),
        AppError::Embedding("embed".into()),
        AppError::LockBusy("held".into()),
        AppError::VecExtension("vec".into()),
    ];

    let mut untranslated = Vec::new();
    for err in &samples {
        let en = err.suggestion_for(Language::English);
        let pt = err.suggestion_for(Language::Portuguese);
        assert_eq!(
            en.is_some(),
            pt.is_some(),
            "{err:?} offers a hint in one language only"
        );
        if let (Some(en), Some(pt)) = (en, pt) {
            if en == pt {
                untranslated.push(format!("{err:?}: {en}"));
            }
        }
    }

    assert!(
        untranslated.is_empty(),
        "these suggestions render identically in en and pt-BR, so they were \
         never translated:\n{}",
        untranslated.join("\n")
    );
}

#[test]
fn the_retry_verdict_travels_with_every_classified_error() {
    use sqlite_graphrag::errors::AppError;

    // An agent reads `error_class` instead of memorising the exit-code table, so
    // the vocabulary has to stay closed and has to agree with the predicates it
    // is derived from.
    let cases: Vec<(AppError, &str, bool)> = vec![
        (AppError::DbBusy("busy".into()), "transient", true),
        (AppError::LockBusy("held".into()), "transient", true),
        (AppError::Validation("bad".into()), "permanent", false),
        (AppError::NotFound("missing".into()), "permanent", false),
        (AppError::Duplicate("dup".into()), "permanent", false),
        (AppError::Conflict("stale".into()), "ambiguous", false),
        (AppError::Embedding("embed".into()), "ambiguous", false),
    ];

    for (err, expected_class, expected_retryable) in &cases {
        assert_eq!(
            err.error_class(),
            *expected_class,
            "{err:?} must classify as {expected_class}"
        );
        assert_eq!(
            err.is_retryable(),
            *expected_retryable,
            "{err:?} retryable flag must agree with its class"
        );
        assert_eq!(
            err.error_class() == "transient",
            err.is_retryable(),
            "{err:?}: `retryable` must be true exactly when the class is transient"
        );
    }
}
