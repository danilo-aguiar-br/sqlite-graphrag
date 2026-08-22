#![cfg(feature = "slow-tests")]

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// v1.0.76 spawns `claude` or `codex` on every `remember` / `ingest` /
/// `edit`. The bundled mocks under `tests/mock-llm/` return a fixed
/// 64-dim zero vector so the binary finishes without a real OAuth
/// login. The mock directory is leaked (no TempDir cleanup) so the
/// spawned subprocess always finds the mocks.
fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[path = "common/mod.rs"]
mod common;

/// Sandbox config directory for `tmp`.
fn cfg_dir(tmp: &TempDir) -> std::path::PathBuf {
    tmp.path().join("config")
}

/// Sandbox cache directory for `tmp`.
fn cache_dir(tmp: &TempDir) -> std::path::PathBuf {
    tmp.path().join("cache")
}

/// Base command wired to the sandbox, with every language channel neutral.
///
/// GAP-SG-101: this file used to isolate with `SQLITE_GRAPHRAG_DB_PATH` and to
/// drive language with `SQLITE_GRAPHRAG_LANG`. Neither is read by production
/// code, so the database landed in the developer's real XDG data directory and
/// the "language via env" tests were decided by the developer's real
/// `config.toml`. On a machine without `i18n.lang=pt` they would have failed.
///
/// The supported channels are `--lang`, XDG `i18n.lang`, and the POSIX locale
/// (`LC_ALL` > `LC_MESSAGES` > `LANG`) — see `Language::from_env_or_locale`.
/// `--config-dir` / `--cache-dir` work on every OS, unlike `XDG_*`.
fn base_cmd(tmp: &TempDir) -> Command {
    let mut c = sgr_cmd();
    c.env_remove("LC_ALL");
    c.env_remove("LC_MESSAGES");
    c.env_remove("LANG");
    c.arg("--config-dir").arg(cfg_dir(tmp));
    c.arg("--cache-dir").arg(cache_dir(tmp));
    c
}

fn cmd_lang(tmp: &TempDir, lang: &str) -> Command {
    let mut c = base_cmd(tmp);
    c.arg("--lang").arg(lang);
    c
}

/// Command whose language comes from the XDG setting `i18n.lang`.
///
/// This is the real replacement for the retired `SQLITE_GRAPHRAG_LANG` channel.
fn cmd_xdg_lang(tmp: &TempDir, lang_val: &str) -> Command {
    base_cmd(tmp)
        .args(["config", "set", "i18n.lang", lang_val])
        .assert()
        .success();
    base_cmd(tmp)
}

fn cmd_no_lang(tmp: &TempDir) -> Command {
    base_cmd(tmp)
}

/// Points the sandbox config at a database inside `tmp` and initializes it.
///
/// Uses the XDG `db.path` key rather than `--db` so callers can append their own
/// subcommand arguments without having to thread `--db` through every call.
fn init_db(tmp: &TempDir) {
    base_cmd(tmp)
        .args(["config", "set", "db.path"])
        .arg(tmp.path().join("test.sqlite"))
        .assert()
        .success();
    base_cmd(tmp).arg("init").assert().success();
}

// ---------------------------------------------------------------------------
// EN/PT parity — AppError variants through localized_message_for
// ---------------------------------------------------------------------------

#[test]
fn localized_message_parity_all_apperror_variants() {
    use sqlite_graphrag::errors::AppError;
    use sqlite_graphrag::i18n::Language;
    use std::io;

    let variants: Vec<AppError> = vec![
        AppError::Validation("campo x".into()),
        AppError::Duplicate("ns/mem".into()),
        AppError::Conflict("ts mudou".into()),
        AppError::NotFound("mem-x".into()),
        AppError::NamespaceError("sem marcador".into()),
        AppError::LimitExceeded("corpo enorme".into()),
        AppError::Embedding("dim errada".into()),
        AppError::VecExtension("extensao failed".into()),
        AppError::DbBusy("retries esgotados".into()),
        AppError::BatchPartialFailure {
            total: 10,
            failed: 3,
        },
        AppError::Io(io::Error::new(io::ErrorKind::NotFound, "arquivo ausente")),
        AppError::LockBusy("outra instancia ativa".into()),
        AppError::AllSlotsFull {
            max: 4,
            waited_secs: 60,
        },
        AppError::LowMemory {
            available_mb: 100,
            required_mb: 500,
        },
    ];

    for variant in &variants {
        let msg_en = variant.localized_message_for(Language::English);
        let msg_pt = variant.localized_message_for(Language::Portuguese);

        assert!(
            !msg_en.is_empty(),
            "mensagem EN vazia para variante: {variant:?}"
        );
        assert!(
            !msg_pt.is_empty(),
            "mensagem PT vazia para variante: {variant:?}"
        );
        assert_ne!(
            msg_en, msg_pt,
            "mensagem EN e PT identicas para variante {variant:?}: '{msg_en}'"
        );
    }
}

#[test]
fn localized_message_en_every_variant_contains_english_term() {
    use sqlite_graphrag::errors::AppError;
    use sqlite_graphrag::i18n::Language;

    let cases: Vec<(AppError, &str)> = vec![
        (AppError::Validation("campo".into()), "validation error"),
        (AppError::Duplicate("ns/m".into()), "duplicate detected"),
        (AppError::Conflict("ts".into()), "conflict"),
        (AppError::NotFound("m".into()), "not found"),
        (
            AppError::NamespaceError("ns".into()),
            "namespace not resolved",
        ),
        (AppError::LimitExceeded("l".into()), "limit exceeded"),
        (AppError::Embedding("e".into()), "embedding error"),
        (
            AppError::VecExtension("v".into()),
            "sqlite-vec extension failed",
        ),
        (AppError::DbBusy("d".into()), "database busy"),
        (AppError::LockBusy("l".into()), "lock busy"),
    ];

    for (variant, expected) in &cases {
        let msg = variant.localized_message_for(Language::English);
        assert!(
            msg.contains(expected),
            "EN: esperado '{expected}' em '{msg}' (variante: {variant:?})"
        );
    }
}

#[test]
fn localized_message_pt_every_variant_contains_portuguese_term() {
    use sqlite_graphrag::errors::AppError;
    use sqlite_graphrag::i18n::Language;

    let cases: Vec<(AppError, &str)> = vec![
        (AppError::Validation("campo".into()), "erro de validação"),
        (AppError::Duplicate("ns/m".into()), "duplicata detectada"),
        (AppError::Conflict("ts".into()), "conflito"),
        (AppError::NotFound("m".into()), "não encontrado"),
        (
            AppError::NamespaceError("ns".into()),
            "namespace não resolvido",
        ),
        (AppError::LimitExceeded("l".into()), "limite excedido"),
        (AppError::Embedding("e".into()), "erro de embedding"),
        (
            AppError::VecExtension("v".into()),
            "extensão sqlite-vec falhou",
        ),
        (AppError::DbBusy("d".into()), "banco ocupado"),
        (AppError::LockBusy("l".into()), "lock ocupado"),
    ];

    for (variant, expected) in &cases {
        let msg = variant.localized_message_for(Language::Portuguese);
        assert!(
            msg.contains(expected),
            "PT: esperado '{expected}' em '{msg}' (variante: {variant:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// E2E tests through the --lang flag
// ---------------------------------------------------------------------------

#[test]
fn lang_pt_remember_invalid_name_stderr_portuguese() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_lang(&tmp, "pt")
        .args([
            "remember",
            "--name",
            "___",
            "--type",
            "user",
            "--description",
            "descricao de teste",
            "--body",
            "conteudo",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("erro de validação").or(predicate::str::contains("empty")),
        );
}

#[test]
fn lang_en_same_scenario_stderr_english() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_lang(&tmp, "en")
        .args([
            "remember",
            "--name",
            "___",
            "--type",
            "user",
            "--description",
            "test description",
            "--body",
            "conteudo",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("validation error").or(predicate::str::contains("empty")));
}

#[test]
fn lang_pt_not_found_stderr_portuguese() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_lang(&tmp, "pt")
        .args(["read", "--name", "memoria-que-nao-existe"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("não encontrado"));
}

#[test]
fn lang_en_not_found_stderr_english() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_lang(&tmp, "en")
        .args(["read", "--name", "memoria-que-nao-existe"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn lang_pt_body_exceeds_limit_stderr_portuguese() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let huge_body = "x".repeat(512_001);
    let body_path = tmp.path().join("body-grande-pt.txt");
    std::fs::write(&body_path, huge_body).unwrap();
    cmd_lang(&tmp, "pt")
        .args([
            "remember",
            "--name",
            "mem-grande",
            "--type",
            "user",
            "--description",
            "descricao de teste",
            "--body-file",
            body_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("corpo excede")
                .or(predicate::str::contains("limite excedido")),
        );
}

#[test]
fn lang_en_body_exceeds_limit_stderr_english() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let huge_body = "x".repeat(512_001);
    let body_path = tmp.path().join("body-grande-en.txt");
    std::fs::write(&body_path, huge_body).unwrap();
    cmd_lang(&tmp, "en")
        .args([
            "remember",
            "--name",
            "mem-grande",
            "--type",
            "user",
            "--description",
            "test description",
            "--body-file",
            body_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("body exceeds").or(predicate::str::contains("limit exceeded")),
        );
}

// ---------------------------------------------------------------------------
// E2E tests through XDG i18n.lang (the real channel; SQLITE_GRAPHRAG_LANG was never read)
// ---------------------------------------------------------------------------

#[test]
fn xdg_i18n_lang_pt_applies_portuguese() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_xdg_lang(&tmp, "pt")
        .args(["read", "--name", "inexistente"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("não encontrado"));
}

#[test]
fn xdg_i18n_lang_en_applies_english() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_xdg_lang(&tmp, "en")
        .args(["read", "--name", "inexistente"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn xdg_i18n_lang_pt_br_applies_portuguese() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_xdg_lang(&tmp, "pt-BR")
        .args(["read", "--name", "inexistente"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("não encontrado"));
}

// ---------------------------------------------------------------------------
// The --lang flag wins over the XDG i18n.lang key
// ---------------------------------------------------------------------------

#[test]
fn flag_lang_en_overrides_xdg_lang_pt() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let mut c = cmd_xdg_lang(&tmp, "pt");
    c.arg("--lang").arg("en");
    c.args(["read", "--name", "inexistente"]);

    c.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn flag_lang_pt_overrides_xdg_lang_en() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let mut c = cmd_xdg_lang(&tmp, "en");
    c.arg("--lang").arg("pt");
    c.args(["read", "--name", "inexistente"]);

    c.assert()
        .failure()
        .stderr(predicate::str::contains("não encontrado"));
}

// ---------------------------------------------------------------------------
// Default without flag and without env var — English fallback
// ---------------------------------------------------------------------------

#[test]
fn default_without_flag_without_xdg_without_locale_returns_english() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_no_lang(&tmp)
        .args(["read", "--name", "inexistente"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// Locale LC_ALL=pt_BR.UTF-8 without flag and without XDG i18n.lang → Portuguese
// ---------------------------------------------------------------------------

#[test]
fn locale_ptbr_without_flag_without_xdg_applies_portuguese() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let mut c = base_cmd(&tmp);
    c.env("LC_ALL", "pt_BR.UTF-8");
    c.args(["read", "--name", "inexistente"]);

    c.assert()
        .failure()
        .stderr(predicate::str::contains("não encontrado"));
}

// ---------------------------------------------------------------------------
// JSON stdout messages are identical in EN and PT (JSON is deterministic)
// ---------------------------------------------------------------------------

#[test]
fn json_stdout_identical_in_en_and_pt() {
    let tmp_en = TempDir::new().unwrap();
    let tmp_pt = TempDir::new().unwrap();
    init_db(&tmp_en);
    init_db(&tmp_pt);

    let output_en = cmd_lang(&tmp_en, "en")
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_pt = cmd_lang(&tmp_pt, "pt")
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_en: serde_json::Value = serde_json::from_slice(&output_en).unwrap();
    let json_pt: serde_json::Value = serde_json::from_slice(&output_pt).unwrap();

    assert_eq!(
        json_en["status"], json_pt["status"],
        "campo status difere entre EN e PT"
    );
    assert_eq!(
        json_en["integrity"], json_pt["integrity"],
        "campo integrity difere entre EN e PT"
    );
}

// ---------------------------------------------------------------------------
// Alias do idioma — aliases aceitos: english, portugues, pt-BR, pt-br
// ---------------------------------------------------------------------------

#[test]
fn alias_english_accepted_by_cli() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_lang(&tmp, "en")
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok");
}

#[test]
fn alias_pt_br_accepted_by_cli() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_lang(&tmp, "pt")
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok");
}
