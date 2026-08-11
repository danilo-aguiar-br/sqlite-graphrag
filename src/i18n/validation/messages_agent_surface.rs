//! Localized messages for the agent-native output surface.
//!
//! Split out of `messages_cli` in v1.2.6: that catalogue already held 397 lines
//! of unrelated CLI guards, and the refusals GAP-SG-201 through GAP-SG-204
//! introduce eight more messages in one domain.
//!
//! Every message names the flag the caller typed and the way out, because the
//! error-handling rules require a refusal to carry a corrective action rather
//! than only a verdict. What is NEVER localized is the JSON around them: field
//! names stay English so a `pt-BR` operator and an English one parse the same
//! envelope.

use crate::i18n::{current, Language};

/// Joins alternatives for the "did you mean" tail, or an empty string when the
/// vocabulary offered nothing close.
fn did_you_mean(suggestions: &[String]) -> String {
    if suggestions.is_empty() {
        return String::new();
    }
    let list = suggestions.join(", ");
    match current() {
        Language::English => format!("; did you mean: {list}"),
        Language::Portuguese => format!("; você quis dizer: {list}"),
    }
}

/// GAP-SG-207: a subcommand that changes durable state reached path resolution
/// without naming its target in the argv.
///
/// Names both ways out, because a refusal that only states a verdict leaves the
/// operator guessing: designate the target, or accept the ambient one on
/// purpose. The flag spellings stay English in both languages, since they are
/// what the caller must type.
pub fn target_not_designated() -> String {
    match current() {
        Language::English => String::from(
            "this subcommand changes durable state and NOTHING named its target: \
             no --db on the command line and no `db.path` in the configuration, \
             so the write would land in the compiled default database. Name it \
             with --db, set it once with `config set db.path <PATH>`, or accept \
             the default on purpose with --use-active",
        ),
        Language::Portuguese => String::from(
            "este subcomando altera estado durável e NADA nomeou o alvo: nenhum \
             --db na linha de comando e nenhum `db.path` na configuração, então \
             a escrita cairia no banco padrão compilado. Nomeie com --db, defina \
             uma vez com `config set db.path <PATH>`, ou aceite o padrão de \
             propósito com --use-active",
        ),
    }
}

/// GAP-SG-202: a key given to `--filter`, `--sort` or `--dedupe-by` exists in no
/// result element.
pub fn key_absent(flag: &str, key: &str, suggestions: &[String]) -> String {
    let tail = did_you_mean(suggestions);
    match current() {
        Language::English => format!(
            "{flag} names '{key}', which no result element carries, so the \
             predicate would reject every row and the empty answer would be \
             indistinguishable from missing data{tail}. Pass \
             --allow-unknown-keys to accept an unresolvable key"
        ),
        Language::Portuguese => format!(
            "{flag} nomeia '{key}', que nenhum elemento de resultado carrega, \
             então o predicado rejeitaria toda linha e a resposta vazia seria \
             indistinguível de ausência de dado{tail}. Passe \
             --allow-unknown-keys para aceitar uma chave irresolvível"
        ),
    }
}

/// GAP-SG-203: the key names a member of the envelope, not a field of the
/// elements the predicate would be applied to.
pub fn key_is_envelope_only(flag: &str, key: &str, array: &str) -> String {
    match current() {
        Language::English => format!(
            "{flag} names '{key}', which is a member of the envelope and not a \
             field of the '{array}' elements the predicate would run over. \
             Applying it would empty '{array}' while '{key}' survived beside the \
             result, contradicting the predicate. Filter on a field the elements \
             carry, or read '{key}' from the unshaped envelope"
        ),
        Language::Portuguese => format!(
            "{flag} nomeia '{key}', que é membro do envelope e não campo dos \
             elementos de '{array}' sobre os quais o predicado rodaria. \
             Aplicá-lo esvaziaria '{array}' enquanto '{key}' sobreviveria ao lado \
             do resultado, contradizendo o predicado. Filtre por um campo que os \
             elementos carreguem, ou leia '{key}' do envelope sem reshaping"
        ),
    }
}

/// GAP-SG-204: a knob was declared against an envelope that carries no result
/// array, so it can have no effect at all.
pub fn knob_without_target(flags: &[String]) -> String {
    let list = flags.join(", ");
    match current() {
        Language::English => format!(
            "{list} was given, but this envelope carries no result array, so the \
             flag can have no effect. Returning success while silently ignoring \
             an argument the caller typed is what this refusal exists to prevent"
        ),
        Language::Portuguese => format!(
            "{list} foi passado, mas este envelope não carrega array de \
             resultado, então a flag não pode ter efeito algum. Retornar sucesso \
             ignorando em silêncio um argumento que o chamador digitou é \
             exatamente o que esta recusa existe para impedir"
        ),
    }
}

/// GAP-SG-202: every key given to `--select` is unresolvable, so the projection
/// would emit empty objects.
pub fn select_fully_unresolved(keys: &[String], suggestions: &[String]) -> String {
    let list = keys.join(", ");
    let tail = did_you_mean(suggestions);
    match current() {
        Language::English => format!(
            "--select names only keys this envelope does not carry ({list}), so \
             the projection would emit empty objects{tail}. Pass \
             --allow-unknown-keys to accept that"
        ),
        Language::Portuguese => format!(
            "--select nomeia apenas chaves que este envelope não carrega \
             ({list}), então a projeção emitiria objetos vazios{tail}. Passe \
             --allow-unknown-keys para aceitar isso"
        ),
    }
}

/// GAP-SG-201: the predicate would observe only the page the query returned.
pub fn filter_scope_is_a_page(observed: usize, total: usize, source: &str) -> String {
    match current() {
        Language::English => format!(
            "the query returned {observed} of {total} rows, so --filter would \
             judge only those {observed} and report an answer about a set it \
             never saw (the ceiling came from the {source}). Raise the limit to \
             cover the universe, or declare the narrower intent with \
             --filter-scope page"
        ),
        Language::Portuguese => format!(
            "a consulta devolveu {observed} de {total} linhas, então --filter \
             julgaria apenas essas {observed} e reportaria uma resposta sobre um \
             conjunto que nunca observou (o teto veio do {source}). Amplie o \
             limite para cobrir o universo, ou declare a intenção mais estreita \
             com --filter-scope page"
        ),
    }
}

/// GAP-SG-201: `--count-only` over an incomplete universe emits a bare number
/// that reads as an inventory.
pub fn count_only_over_a_page(observed: usize, total: usize) -> String {
    match current() {
        Language::English => format!(
            "--count-only would emit a bare number counted over {observed} of \
             {total} rows, which a caller reads as the inventory. Raise the \
             limit, or declare --filter-scope page to accept a count of the page"
        ),
        Language::Portuguese => format!(
            "--count-only emitiria um número isolado contado sobre {observed} de \
             {total} linhas, que o chamador lê como o inventário. Amplie o \
             limite, ou declare --filter-scope page para aceitar a contagem da \
             página"
        ),
    }
}
