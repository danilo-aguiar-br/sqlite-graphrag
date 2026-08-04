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
//! The single legitimate mention lives in the guard test that proves the
//! variable is ignored, which is source code and not user-facing text, so it is
//! out of scope here by construction.

use clap::CommandFactory;

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
