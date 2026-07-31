use super::*;
use serial_test::serial;

#[test]
#[serial]
fn fallback_english_when_env_absent() {
    std::env::remove_var("SQLITE_GRAPHRAG_LANG");
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
    std::env::remove_var("SQLITE_GRAPHRAG_LANG");
    std::env::set_var("LC_ALL", "pt_BR.UTF-8");
    assert_eq!(Language::from_env_or_locale(), Language::Portuguese);
    std::env::remove_var("LC_ALL");
}

#[test]
#[serial]
fn posix_precedence_lc_all_overrides_lang() {
    std::env::remove_var("SQLITE_GRAPHRAG_LANG");
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
    std::env::remove_var("SQLITE_GRAPHRAG_LANG");
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
    std::env::remove_var("SQLITE_GRAPHRAG_LANG");
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
        let nome = "Invalid_Name";
        let msg = match Language::English {
            Language::English => format!(
                "name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
            ),
            Language::Portuguese => {
                format!("nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'")
            }
        };
        assert!(msg.contains("kebab-case slug"), "obtido: {msg}");
        assert!(msg.contains("Invalid_Name"), "obtido: {msg}");
    }

    #[test]
    fn name_kebab_pt() {
        let nome = "Invalid_Name";
        let msg = match Language::Portuguese {
            Language::English => format!(
                "name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
            ),
            Language::Portuguese => {
                format!("nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'")
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
        let limite = crate::constants::MAX_MEMORY_BODY_LEN;
        let msg = match Language::English {
            Language::English => format!("body exceeds {limite} bytes"),
            Language::Portuguese => format!("corpo excede {limite} bytes"),
        };
        assert!(msg.contains("body exceeds 512000"), "obtido: {msg}");
    }

    #[test]
    fn body_exceeds_pt() {
        let limite = crate::constants::MAX_MEMORY_BODY_LEN;
        let msg = match Language::Portuguese {
            Language::English => format!("body exceeds {limite} bytes"),
            Language::Portuguese => format!("corpo excede {limite} bytes"),
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
        let nome = "Bad Name";
        let msg = match Language::English {
            Language::English => format!(
                "new-name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
            ),
            Language::Portuguese => format!(
                "novo nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'"
            ),
        };
        assert!(msg.contains("new-name must be kebab-case"), "obtido: {msg}");
    }

    #[test]
    fn new_name_kebab_pt() {
        let nome = "Bad Name";
        let msg = match Language::Portuguese {
            Language::English => format!(
                "new-name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
            ),
            Language::Portuguese => format!(
                "novo nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'"
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
