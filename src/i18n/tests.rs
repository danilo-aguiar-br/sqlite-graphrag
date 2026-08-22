//! GAP-SG-232: only POSIX locale variables are manipulated here, deliberately.
//!
//! `LC_ALL`, `LC_MESSAGES` and `LANG` are SYSTEM channels that
//! [`Language::from_env_or_locale`] really reads. The product itself reads no
//! environment variable of its own: the language comes from the `--lang` flag,
//! then the XDG key `i18n.lang`, then the compiled default. Clearing a
//! `SQLITE_GRAPHRAG_`-prefixed variable used to precede each case below, which
//! taught the reader that such a channel exists.

use super::*;
use serial_test::serial;

#[test]
#[serial]
fn fallback_english_when_env_absent() {
    std::env::set_var("LC_ALL", "C");
    std::env::set_var("LANG", "C");
    assert_eq!(Language::from_env_or_locale(), Language::English);
    std::env::remove_var("LC_ALL");
    std::env::remove_var("LANG");
}

#[test]
#[serial]
fn flag_pt_parses_portuguese() {
    assert_eq!(Language::from_str_opt("pt"), Some(Language::Portuguese));
}

#[test]
fn flag_pt_br_parses_portuguese() {
    assert_eq!(Language::from_str_opt("pt-BR"), Some(Language::Portuguese));
}

#[test]
#[serial]
fn locale_ptbr_utf8_selects_portuguese() {
    std::env::set_var("LC_ALL", "pt_BR.UTF-8");
    assert_eq!(Language::from_env_or_locale(), Language::Portuguese);
    std::env::remove_var("LC_ALL");
}

#[test]
#[serial]
fn posix_precedence_lc_all_overrides_lang() {
    std::env::remove_var("LC_MESSAGES");
    std::env::set_var("LC_ALL", "en_US.UTF-8");
    std::env::set_var("LANG", "pt_BR.UTF-8");
    assert_eq!(
        Language::from_env_or_locale(),
        Language::English,
        "LC_ALL=en_US must override LANG=pt_BR per POSIX"
    );
    std::env::remove_var("LC_ALL");
    std::env::remove_var("LANG");
}

#[test]
#[serial]
fn posix_precedence_lc_all_unrecognized_stops_iteration() {
    std::env::remove_var("LC_MESSAGES");
    std::env::set_var("LC_ALL", "ja_JP.UTF-8");
    std::env::set_var("LANG", "pt_BR.UTF-8");
    assert_eq!(
        Language::from_env_or_locale(),
        Language::English,
        "LC_ALL=ja_JP set must stop iteration; falls back to English default"
    );
    std::env::remove_var("LC_ALL");
    std::env::remove_var("LANG");
}

#[test]
#[serial]
fn lang_pt_selects_portuguese_when_lc_all_unset() {
    std::env::remove_var("LC_ALL");
    std::env::remove_var("LC_MESSAGES");
    std::env::set_var("LANG", "pt_BR.UTF-8");
    assert_eq!(Language::from_env_or_locale(), Language::Portuguese);
    std::env::remove_var("LANG");
}

mod validation_tests {
    use super::*;

    #[test]
    fn name_length_en() {
        let msg = match Language::English {
            Language::English => format!("name must be 1-{} chars", 80),
            Language::Portuguese => format!("nome deve ter entre 1 e {} caracteres", 80),
        };
        assert!(msg.contains("name must be 1-80 chars"), "obtido: {msg}");
    }

    #[test]
    fn name_length_pt() {
        let msg = match Language::Portuguese {
            Language::English => format!("name must be 1-{} chars", 80),
            Language::Portuguese => format!("nome deve ter entre 1 e {} caracteres", 80),
        };
        assert!(
            msg.contains("nome deve ter entre 1 e 80 caracteres"),
            "obtido: {msg}"
        );
    }

    #[test]
    fn name_kebab_en() {
        let name = "Invalid_Name";
        let msg = match Language::English {
            Language::English => format!(
                "name must be kebab-case slug (lowercase letters, digits, hyphens): '{name}'"
            ),
            Language::Portuguese => {
                format!("nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{name}'")
            }
        };
        assert!(msg.contains("kebab-case slug"), "obtido: {msg}");
        assert!(msg.contains("Invalid_Name"), "obtido: {msg}");
    }

    #[test]
    fn name_kebab_pt() {
        let name = "Invalid_Name";
        let msg = match Language::Portuguese {
            Language::English => format!(
                "name must be kebab-case slug (lowercase letters, digits, hyphens): '{name}'"
            ),
            Language::Portuguese => {
                format!("nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{name}'")
            }
        };
        assert!(msg.contains("kebab-case"), "obtido: {msg}");
        assert!(msg.contains("minúsculas"), "obtido: {msg}");
        assert!(msg.contains("Invalid_Name"), "obtido: {msg}");
    }

    #[test]
    fn description_exceeds_en() {
        let msg = match Language::English {
            Language::English => format!("description must be <= {} chars", 500),
            Language::Portuguese => format!("descrição deve ter no máximo {} caracteres", 500),
        };
        assert!(msg.contains("description must be <= 500"), "obtido: {msg}");
    }

    #[test]
    fn description_exceeds_pt() {
        let msg = match Language::Portuguese {
            Language::English => format!("description must be <= {} chars", 500),
            Language::Portuguese => format!("descrição deve ter no máximo {} caracteres", 500),
        };
        assert!(
            msg.contains("descrição deve ter no máximo 500"),
            "obtido: {msg}"
        );
    }

    #[test]
    fn body_exceeds_en() {
        let cap = crate::constants::MAX_MEMORY_BODY_LEN;
        let msg = match Language::English {
            Language::English => format!("body exceeds {cap} bytes"),
            Language::Portuguese => format!("corpo excede {cap} bytes"),
        };
        assert!(msg.contains("body exceeds 512000"), "obtido: {msg}");
    }

    #[test]
    fn body_exceeds_pt() {
        let cap = crate::constants::MAX_MEMORY_BODY_LEN;
        let msg = match Language::Portuguese {
            Language::English => format!("body exceeds {cap} bytes"),
            Language::Portuguese => format!("corpo excede {cap} bytes"),
        };
        assert!(msg.contains("corpo excede 512000"), "obtido: {msg}");
    }

    #[test]
    fn new_name_length_en() {
        let msg = match Language::English {
            Language::English => format!("new-name must be 1-{} chars", 80),
            Language::Portuguese => format!("novo nome deve ter entre 1 e {} caracteres", 80),
        };
        assert!(msg.contains("new-name must be 1-80"), "obtido: {msg}");
    }

    #[test]
    fn new_name_length_pt() {
        let msg = match Language::Portuguese {
            Language::English => format!("new-name must be 1-{} chars", 80),
            Language::Portuguese => format!("novo nome deve ter entre 1 e {} caracteres", 80),
        };
        assert!(
            msg.contains("novo nome deve ter entre 1 e 80"),
            "obtido: {msg}"
        );
    }

    #[test]
    fn new_name_kebab_en() {
        let name = "Bad Name";
        let msg = match Language::English {
            Language::English => format!(
                "new-name must be kebab-case slug (lowercase letters, digits, hyphens): '{name}'"
            ),
            Language::Portuguese => format!(
                "novo nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{name}'"
            ),
        };
        assert!(msg.contains("new-name must be kebab-case"), "obtido: {msg}");
    }

    #[test]
    fn new_name_kebab_pt() {
        let name = "Bad Name";
        let msg = match Language::Portuguese {
            Language::English => format!(
                "new-name must be kebab-case slug (lowercase letters, digits, hyphens): '{name}'"
            ),
            Language::Portuguese => format!(
                "novo nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{name}'"
            ),
        };
        assert!(
            msg.contains("novo nome deve estar em kebab-case"),
            "obtido: {msg}"
        );
    }

    #[test]
    fn reserved_name_en() {
        let msg = match Language::English {
            Language::English => {
                "names and namespaces starting with __ are reserved for internal use".to_string()
            }
            Language::Portuguese => {
                "nomes e namespaces iniciados com __ são reservados para uso interno".to_string()
            }
        };
        assert!(msg.contains("reserved for internal use"), "obtido: {msg}");
    }

    #[test]
    fn reserved_name_pt() {
        let msg = match Language::Portuguese {
            Language::English => {
                "names and namespaces starting with __ are reserved for internal use".to_string()
            }
            Language::Portuguese => {
                "nomes e namespaces iniciados com __ são reservados para uso interno".to_string()
            }
        };
        assert!(msg.contains("reservados para uso interno"), "obtido: {msg}");
    }
}

mod app_error_pt_translation_tests {
    use crate::errors::AppError;

    #[test]
    fn localized_message_pt_not_found_fully_translated() {
        let err = AppError::NotFound("memory 'test-mem' not found in namespace 'global'".into());
        let pt = err.localized_message_for(crate::i18n::Language::Portuguese);
        assert!(
            pt.contains("memória"),
            "PT must translate 'memory' to 'memória': {pt}"
        );
        assert!(
            pt.contains("não encontrada no namespace"),
            "PT must translate full phrase: {pt}"
        );
        assert!(
            !pt.contains("not found in namespace"),
            "PT must not contain English phrase: {pt}"
        );
    }

    #[test]
    fn localized_message_pt_duplicate_fully_translated() {
        let err = AppError::Duplicate(
            "memory 'x' already exists in namespace 'global'. Use --force-merge to update.".into(),
        );
        let pt = err.localized_message_for(crate::i18n::Language::Portuguese);
        assert!(pt.contains("memória"), "PT must translate 'memory': {pt}");
        assert!(
            pt.contains("já existe no namespace"),
            "PT must translate 'already exists': {pt}"
        );
    }
}

/// GAP-SG-143: guards for the `NotFound` message catalog and for the rule that
/// user-facing error text is never assembled outside `crate::i18n::validation`.
mod not_found_catalog_tests {
    use crate::errors::AppError;
    use crate::i18n::validation::catalog_samples;
    use crate::i18n::Language;

    /// English fragments that must never survive into the Portuguese rendering.
    ///
    /// Each one is a substring the catalog actually emits, so a miss here means
    /// a real bilingual hybrid reached the operator — not a hypothetical one.
    const ENGLISH_RESIDUE: &[&str] = &[
        "not found",
        "memory",
        "entity",
        "relationship",
        "edge '",
        "Did you mean",
        "Re-run with",
        "is not held",
        "no key with fingerprint",
        "exists but belongs",
        "in namespace '",
        ", not '",
    ];

    /// Every catalog entry, rendered through the real error path, must come out
    /// of `pt-BR` with zero English left.
    ///
    /// This is what makes the split safe: the builders own the English, the
    /// `app_error_pt::not_found` chain owns the Portuguese, and neither side can
    /// be extended alone without this failing.
    #[test]
    fn not_found_catalog_has_no_english_residue_in_pt() {
        for english in catalog_samples() {
            let pt = AppError::NotFound(english.clone())
                .localized_message_for(Language::Portuguese)
                .to_lowercase();
            for residue in ENGLISH_RESIDUE {
                assert!(
                    !pt.contains(&residue.to_lowercase()),
                    "pt-BR rendering still carries the English fragment {residue:?}\n  \
                     english: {english}\n  pt:      {pt}\n  \
                     fix: extend the replace chain in \
                     src/i18n/validation/app_error_pt.rs::not_found"
                );
            }
        }
    }

    /// The builders must never read the process locale.
    ///
    /// Checked at the SOURCE, not by flipping the language: `i18n::init` is a
    /// `OnceLock`, so a test cannot switch locales mid-process. A `current()`
    /// call in this module would make the English payload depend on the host
    /// locale, which breaks `localized_message_for(English)` and feeds the
    /// Portuguese replace-chain text it can no longer match.
    #[test]
    fn not_found_catalog_never_reads_the_locale() {
        let source = include_str!("validation/messages_not_found.rs");
        let code: String = source
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("current()"),
            "messages_not_found must stay locale-independent; \
             Portuguese belongs in app_error_pt::not_found"
        );
    }

    /// `catalog_samples` must exercise EVERY builder in the module.
    ///
    /// Without this, someone adds a builder, forgets the sample, and the
    /// residue test above silently stops covering it.
    #[test]
    fn not_found_catalog_covers_every_builder() {
        let source = include_str!("validation/messages_not_found.rs");
        let builders = source.lines().filter(|l| l.starts_with("pub fn ")).count();
        assert_eq!(
            builders,
            catalog_samples().len(),
            "src/i18n/validation/messages_not_found.rs has {builders} builders but \
             catalog_samples() lists {}; add the missing entry",
            catalog_samples().len()
        );
    }
}

/// GAP-SG-143: the anti-regression guard. Without it the migration undoes
/// itself the next time someone reaches for `format!` in a hurry.
mod error_message_source_guard {
    /// Every `AppError` variant whose payload is VISIBLE TO THE OPERATOR and
    /// must therefore come from the `crate::i18n` catalogs.
    ///
    /// The list describes the variants the operator can READ, not the ones a
    /// migration happened to reach first. Calibrating it against the migrated
    /// sample is exactly how the original two-entry version passed while seven
    /// further variants — `DbBusy`, `LockBusy`, `Conflict`, `Duplicate`,
    /// `NamespaceError`, `LimitExceeded`, `Embedding` — still built their text
    /// inline. Any new variant that reaches stderr belongs here on the day it
    /// is added.
    ///
    /// Indices 0 and 1 are load-bearing: the two self-tests below quote
    /// `BANNED[0]` and `BANNED[1]`.
    const BANNED: &[&str] = &[
        "AppError::Validation(format!(",
        "AppError::NotFound(format!(",
        "AppError::DbBusy(format!(",
        "AppError::LockBusy(format!(",
        "AppError::Conflict(format!(",
        "AppError::Duplicate(format!(",
        "AppError::NamespaceError(format!(",
        "AppError::LimitExceeded(format!(",
        "AppError::Embedding(format!(",
    ];

    /// One source character plus the 1-based line it came from.
    struct Normalized {
        text: String,
        line_of_byte: Vec<usize>,
    }

    /// Strips `//` line comments and ALL whitespace, keeping a byte-to-line map.
    ///
    /// Both steps are load-bearing. Comments are stripped because
    /// `messages_naming.rs` and its siblings document the banned pattern in prose and
    /// a literal match
    /// would flag the very file that fixes it. Whitespace is stripped because
    /// rustfmt splits these constructors across lines the moment the arguments
    /// grow, so a literal search finds nothing and the guard passes while
    /// broken — the failure mode the config track already hit on its own
    /// anti-drift test.
    ///
    /// String literals are NOT parsed. A literal containing `//` would be
    /// truncated here; that only ever produces a false NEGATIVE on the rest of
    /// the line, never a false positive, and no banned pattern is preceded by
    /// one in this crate.
    fn normalize(source: &str) -> Normalized {
        let mut text = String::with_capacity(source.len());
        let mut line_of_byte = Vec::with_capacity(source.len());
        for (idx, raw) in source.lines().enumerate() {
            let code = match raw.find("//") {
                Some(pos) => &raw[..pos],
                None => raw,
            };
            for ch in code.chars().filter(|c| !c.is_whitespace()) {
                let before = text.len();
                text.push(ch);
                line_of_byte.resize(text.len(), idx + 1);
                debug_assert!(text.len() > before);
            }
        }
        Normalized { text, line_of_byte }
    }

    /// Collects every `.rs` file under `dir`, recursively.
    fn rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// `src/i18n/**` defines the catalog and documents the banned pattern, so it
    /// is the one place allowed to mention it. Files whose name carries `test`
    /// are fixtures and assertions, not user-facing paths.
    fn is_exempt(path: &std::path::Path) -> bool {
        let as_str = path.to_string_lossy().replace('\\', "/");
        if as_str.contains("/src/i18n/") {
            return true;
        }
        path.file_name()
            .map(|n| n.to_string_lossy().contains("test"))
            .unwrap_or(false)
    }

    #[test]
    fn user_facing_errors_are_not_built_with_inline_format() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_sources(&root, &mut files);
        assert!(files.len() > 50, "source walk found only {}", files.len());

        let mut offenders: Vec<String> = Vec::new();
        for path in files {
            if is_exempt(&path) {
                continue;
            }
            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let normalized = normalize(&source);
            for banned in BANNED {
                let needle: String = banned.chars().filter(|c| !c.is_whitespace()).collect();
                let mut from = 0;
                while let Some(hit) = normalized.text[from..].find(&needle) {
                    let at = from + hit;
                    let line = normalized.line_of_byte.get(at).copied().unwrap_or(0);
                    offenders.push(format!(
                        "{}:{line} builds `{banned}...)` inline",
                        path.strip_prefix(&root).unwrap_or(&path).display()
                    ));
                    from = at + needle.len();
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "GAP-SG-143: user-facing error text must come from \
             crate::i18n::validation so pt-BR stays covered.\n{}\n\
             Add a builder in src/i18n/validation/messages_not_found.rs, the \
             Validation catalogs under src/i18n/validation/, or \
             crate::i18n::errors_ops for operational payloads, and call it here.",
            offenders.join("\n")
        );
    }

    /// The guard must actually detect the pattern, including the rustfmt-split
    /// shape. A guard that cannot fail is not a guard.
    #[test]
    fn the_guard_detects_a_line_split_constructor() {
        let split = "let e = AppError::NotFound(format!(\n    \"memory '{x}' not found\"\n));";
        let normalized = normalize(split);
        let needle: String = BANNED[1].chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            normalized.text.contains(&needle),
            "normalisation must survive a rustfmt line split: {}",
            normalized.text
        );
    }

    /// ...and must NOT fire on prose that merely mentions the pattern.
    #[test]
    fn the_guard_ignores_the_pattern_inside_a_comment() {
        let prose =
            "// Prefer these over ad-hoc AppError::Validation(format!(...)) so EN/PT\nlet x = 1;";
        let normalized = normalize(prose);
        let needle: String = BANNED[0].chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            !normalized.text.contains(&needle),
            "a comment must not trip the guard: {}",
            normalized.text
        );
    }
}
