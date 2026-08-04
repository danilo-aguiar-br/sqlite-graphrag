//! Messages about NAMES, namespaces and filesystem paths (GAP-SG-146).
//!
//! The kebab-case contract for memory and entity names, namespace shape, and
//! the path guards that keep writes inside their intended root.

use crate::i18n::{current, Language};

/// Localized message for `name_length`.
pub fn name_length(max: usize) -> String {
    match current() {
        Language::English => format!("name must be 1-{max} chars"),
        Language::Portuguese => format!("nome deve ter entre 1 e {max} caracteres"),
    }
}

/// Localized message for `name_kebab`.
pub fn name_kebab(nome: &str) -> String {
    match current() {
        Language::English => {
            format!("name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'")
        }
        Language::Portuguese => {
            format!("nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'")
        }
    }
}

/// Name normalized to empty (only hyphens/underscores/spaces).
pub fn name_empty_after_normalization() -> String {
    match current() {
        Language::English => {
            "name cannot be empty after normalization (input was blank or contained only hyphens/underscores/spaces)"
                .to_string()
        }
        Language::Portuguese => {
            "nome não pode ficar vazio após normalização (entrada em branco ou só com hífens/sublinhados/espaços)"
                .to_string()
        }
    }
}

/// Empty name rejected.
pub fn name_must_not_be_empty() -> String {
    match current() {
        Language::English => "name must not be empty".to_string(),
        Language::Portuguese => "nome não pode ser vazio".to_string(),
    }
}

/// `--name-prefix` must be kebab-case starting with a letter.
pub fn name_prefix_kebab(prefix: &str) -> String {
    match current() {
        Language::English => format!(
            "--name-prefix '{prefix}' must start with a lowercase letter and contain              only lowercase letters, digits and hyphens (kebab-case)"
        ),
        Language::Portuguese => format!(
            "--name-prefix '{prefix}' deve começar com letra minúscula e conter              apenas letras minúsculas, dígitos e hífens (kebab-case)"
        ),
    }
}

/// Localized message for `reserved_name`.
pub fn reserved_name() -> String {
    match current() {
        Language::English => {
            "names and namespaces starting with __ are reserved for internal use".to_string()
        }
        Language::Portuguese => {
            "nomes e namespaces iniciados com __ são reservados para uso interno".to_string()
        }
    }
}

/// Localized message for `new_name_kebab`.
pub fn new_name_kebab(nome: &str) -> String {
    match current() {
        Language::English => format!(
            "new-name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
        ),
        Language::Portuguese => {
            format!("novo nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'")
        }
    }
}

/// Localized message for `new_name_length`.
pub fn new_name_length(max: usize) -> String {
    match current() {
        Language::English => format!("new-name must be 1-{max} chars"),
        Language::Portuguese => format!("novo nome deve ter entre 1 e {max} caracteres"),
    }
}

/// `--strict-name` refused auto-normalization.
pub fn strict_name_not_canonical(original: &str, normalized: &str) -> String {
    match current() {
        Language::English => format!(
            "--strict-name is set but '{original}' is not canonical kebab-case; \
             re-run with --name '{normalized}' (or drop --strict-name to allow auto-normalization)"
        ),
        Language::Portuguese => format!(
            "--strict-name está ativo mas '{original}' não é kebab-case canônico; \
             reexecute com --name '{normalized}' (ou remova --strict-name para permitir auto-normalização)"
        ),
    }
}

/// Child name derived from parent exceeds MAX_MEMORY_NAME_LEN.
pub fn child_name_exceeds_max(child: &str, parent: &str, max: usize) -> String {
    match current() {
        Language::English => format!(
            "child name '{child}' derived from '{parent}' exceeds MAX_MEMORY_NAME_LEN ({max})"
        ),
        Language::Portuguese => format!(
            "nome filho '{child}' derivado de '{parent}' excede MAX_MEMORY_NAME_LEN ({max})"
        ),
    }
}

/// Child name is not kebab-case ASCII.
pub fn child_name_not_kebab(child: &str) -> String {
    match current() {
        Language::English => {
            format!("child name '{child}' is not kebab-case ASCII; rename the parent memory")
        }
        Language::Portuguese => {
            format!("nome filho '{child}' não é kebab-case ASCII; renomeie a memória pai")
        }
    }
}

/// Localized message for `namespace_format`.
pub fn namespace_format() -> String {
    match current() {
        Language::English => "namespace must be alphanumeric + hyphens/underscores".to_string(),
        Language::Portuguese => {
            "namespace deve ser alfanumérico com hífens/sublinhados".to_string()
        }
    }
}

/// Localized message for `namespace_length`.
pub fn namespace_length() -> String {
    match current() {
        Language::English => "namespace must be 1-80 chars".to_string(),
        Language::Portuguese => "namespace deve ter entre 1 e 80 caracteres".to_string(),
    }
}

/// Short ALL_CAPS entity name rejected as NER noise.
pub fn entity_name_all_caps_noise(name: &str) -> String {
    match current() {
        Language::English => format!(
            "entity name '{name}' rejected: short ALL_CAPS names are typically NER noise"
        ),
        Language::Portuguese => format!(
            "nome de entidade '{name}' rejeitado: nomes curtos em CAIXA ALTA são tipicamente ruído de NER"
        ),
    }
}

/// Entity rename target already exists.
pub fn entity_name_already_exists(name: &str, namespace: &str) -> String {
    match current() {
        Language::English => {
            format!("entity with name '{name}' already exists in namespace '{namespace}'")
        }
        Language::Portuguese => {
            format!("entidade com nome '{name}' já existe no namespace '{namespace}'")
        }
    }
}

/// Entity name normalizes too short.
pub fn entity_name_normalizes_too_short(original: &str, normalized: &str) -> String {
    match current() {
        Language::English => format!(
            "entity name '{original}' normalizes to '{normalized}' which is too short (minimum 2 characters)"
        ),
        Language::Portuguese => format!(
            "nome de entidade '{original}' normaliza para '{normalized}' que é curto demais (mínimo 2 caracteres)"
        ),
    }
}

/// Purely numeric entity name rejected.
pub fn entity_name_purely_numeric(name: &str) -> String {
    match current() {
        Language::English => format!(
            "entity name '{name}' rejected: purely numeric names look like entity IDs — \
             use --from-id/--to-id for ID-based linking, or pass a non-numeric name"
        ),
        Language::Portuguese => format!(
            "nome de entidade '{name}' rejeitado: nomes puramente numéricos parecem IDs — \
             use --from-id/--to-id para link por ID, ou passe um nome não numérico"
        ),
    }
}

/// Entity name too short.
pub fn entity_name_too_short(name: &str) -> String {
    match current() {
        Language::English => format!("entity name '{name}' must be at least 2 characters"),
        Language::Portuguese => {
            format!("nome de entidade '{name}' deve ter pelo menos 2 caracteres")
        }
    }
}

/// Too many name collisions while generating a unique memory name.
pub fn too_many_name_collisions(base: &str, max: usize) -> String {
    match current() {
        Language::English => format!(
            "too many name collisions for base '{base}' (>{max}); rename source files to disambiguate"
        ),
        Language::Portuguese => format!(
            "muitas colisões de nome para a base '{base}' (>{max}); renomeie os arquivos de origem para desambiguar"
        ),
    }
}

/// Path has no valid parent component.
pub fn path_no_valid_parent(path: &str) -> String {
    match current() {
        Language::English => format!("path '{path}' has no valid parent component"),
        Language::Portuguese => format!("caminho '{path}' não tem componente pai válido"),
    }
}

/// Localized message for `path_traversal`.
pub fn path_traversal(p: &str) -> String {
    match current() {
        Language::English => format!("path traversal rejected: {p}"),
        Language::Portuguese => format!("traversal de caminho rejeitado: {p}"),
    }
}

/// Directory path does not exist.
pub fn directory_not_found(path: &str) -> String {
    match current() {
        Language::English => format!("directory not found: {path}"),
        Language::Portuguese => format!("diretório não encontrado: {path}"),
    }
}

/// Path exists but is not a directory.
pub fn not_a_directory(path: &str) -> String {
    match current() {
        Language::English => format!("path is not a directory: {path}"),
        Language::Portuguese => format!("caminho não é um diretório: {path}"),
    }
}
