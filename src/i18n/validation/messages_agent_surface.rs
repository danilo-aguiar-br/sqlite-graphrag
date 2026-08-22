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
///
/// It no longer offers `config set db.path` as a remedy. That key is a HOST
/// setting, so a write inheriting it is refused by the same fence — pointing the
/// operator at it would send them one command further into the same refusal.
pub fn target_not_designated() -> String {
    match current() {
        Language::English => String::from(
            "this subcommand changes durable state and NOTHING named its target: \
             no --db on the command line and no `db.path` in the configuration, \
             so the write would land in the compiled default database. Name it \
             with --db, or accept the default on purpose with --use-active",
        ),
        Language::Portuguese => String::from(
            "este subcomando altera estado durável e NADA nomeou o alvo: nenhum \
             --db na linha de comando e nenhum `db.path` na configuração, então \
             a escrita cairia no banco padrão compilado. Nomeie com --db, ou \
             aceite o padrão de propósito com --use-active",
        ),
    }
}

/// A subcommand that changes durable state inherited its target from the XDG
/// key `db.path` instead of the argv.
///
/// Separate from [`target_not_designated`] because the operator's situation is
/// different in the way that matters: something DID name a database, it just was
/// not this invocation. The message therefore has to explain why a value that
/// looks like a designation is not accepted as one, and the reason is SCOPE —
/// `db.path` is a host setting, so it names one database for every directory on
/// the machine rather than the one this command means.
///
/// The resolved path is deliberately absent from the text. It reaches the caller
/// through `db_path_resolved` in the envelope, and repeating it here would
/// suggest the refusal is about which value was found rather than about which
/// layer supplied it.
pub fn target_inherited_from_config() -> String {
    match current() {
        Language::English => String::from(
            "this subcommand changes durable state and its target came from the \
             `db.path` configuration key, not from this command line. That key is \
             a HOST setting: it names one database for every directory on this \
             machine, so it cannot designate the target of a single write. Name \
             the database with --db, or accept the configured one on purpose with \
             --use-active",
        ),
        Language::Portuguese => String::from(
            "este subcomando altera estado durável e o alvo dele veio da chave de \
             configuração `db.path`, não desta linha de comando. Essa chave é do \
             HOST: ela nomeia um banco para todos os diretórios desta máquina, \
             então não designa o alvo de uma escrita específica. Nomeie o banco \
             com --db, ou aceite o configurado de propósito com --use-active",
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

/// GAP-SG-209: a knob that needs the whole set was aimed at a stream.
///
/// `export` emits one self-contained JSON record per line, and the surface is
/// defined over a COMPLETE envelope. Applying a whole-set knob there ran it once
/// per line: `--count-only export --limit 10` answered with eleven separate
/// `{"count":1}` lines instead of one count. Naming the flags as data matters
/// here as much as elsewhere, so `discarded_flags` carries them.
///
/// GAP-SG-215 added `--max-items` to the callers of this message and made the
/// corrective action name `--limit`. "Narrow the query itself" was true and
/// unactionable: a caller who reached for an output ceiling needs to be told
/// which flag is the QUERY ceiling, not merely that one exists.
pub fn knob_needs_a_whole_set(flags: &[String]) -> String {
    let list = flags.join(", ");
    match current() {
        Language::English => format!(
            "{list} needs a complete result set, but this subcommand emits one \
             self-contained record per line. Applied here it would run once per \
             record and answer about a single line instead of the stream. Narrow \
             the query itself with --limit, or pipe the output to a tool that \
             spans lines"
        ),
        Language::Portuguese => format!(
            "{list} precisa de um conjunto de resultados completo, mas este \
             subcomando emite um registro autocontido por linha. Aplicado aqui, \
             rodaria uma vez por registro e responderia sobre uma linha isolada \
             em vez do stream. Estreite a própria consulta com --limit, ou \
             canalize a saída para uma ferramenta que atravesse linhas"
        ),
    }
}

/// GAP-SG-215: `--filter` on a stream would desynchronise the trailer's tally.
///
/// Separate from [`knob_needs_a_whole_set`] because the reason is different, and
/// a refusal that gives the wrong reason teaches the caller the wrong fix. A
/// predicate CAN be evaluated per record; what it cannot do is stay consistent
/// with a count the command computed before the surface saw a single line. The
/// corrective action is therefore the query's own narrowing flags, not a wider
/// output ceiling.
pub fn filter_would_desync_a_tally() -> String {
    match current() {
        Language::English => "--filter cannot narrow a stream: this subcommand emits one record \
             per line and its summary line counts the records the QUERY \
             returned, so a predicate applied here would leave that count \
             describing rows you never received. Narrow the query instead — \
             --type, --namespace or --limit"
            .to_string(),
        Language::Portuguese => "--filter não pode estreitar um stream: este subcomando emite um \
             registro por linha e a linha de sumário conta os registros que a \
             CONSULTA devolveu, então um predicado aplicado aqui deixaria essa \
             contagem descrevendo linhas que você nunca recebeu. Estreite a \
             consulta em vez disso — --type, --namespace ou --limit"
            .to_string(),
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
