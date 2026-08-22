//! Messages about CLI ARGUMENTS and flag combinations (GAP-SG-146).
//!
//! Missing, conflicting or malformed flags, and the explicit confirmations a
//! destructive subcommand refuses to run without.

use crate::i18n::{current, Language};

/// A concurrency ceiling of zero was handed to the LLM slot acquisition.
///
/// GAP-SG-262: this message lived as a Portuguese string literal inside
/// `llm_slots::acquire_llm_slot`, so it was the one refusal on that path that
/// an English-speaking caller received in Portuguese. It never reached
/// `refusal_message_call_site_gate` either, because that gate walks THIS module
/// and the message was not in it.
///
/// Zero is rejected rather than treated as unlimited: a zero ceiling would make
/// every slot comparison vacuous and authorise unbounded concurrency, which is
/// the opposite of what the caller asked for by naming a limit at all.
pub fn llm_slot_ceiling_must_be_positive() -> String {
    match current() {
        Language::English => {
            "max_concurrent must be >= 1 for acquire_llm_slot: a ceiling of 0 would \
             authorise unbounded concurrency rather than none"
                .to_string()
        }
        Language::Portuguese => "max_concurrent deve ser >= 1 para acquire_llm_slot: um teto de 0 \
             autorizaria concorrência ilimitada em vez de nenhuma"
            .to_string(),
    }
}

/// Invalid JSON for a CLI flag that takes a file path (e.g. `--entities-file`).
pub fn invalid_json_in_flag(flag: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("invalid JSON in {flag}: {err}"),
        Language::Portuguese => format!("JSON inválido em {flag}: {err}"),
    }
}

/// Invalid JSON payload on a stdin flag (e.g. `--graph-stdin`).
pub fn invalid_json_payload_on_flag(flag: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("invalid JSON payload on {flag}: {err}"),
        Language::Portuguese => format!("payload JSON inválido em {flag}: {err}"),
    }
}

/// Invalid pending-embeddings status filter.
pub fn invalid_status_filter(other: &str) -> String {
    match current() {
        Language::English => {
            format!("invalid status filter: {other} (expected pending|in_progress|done|abandoned)")
        }
        Language::Portuguese => format!(
            "filtro de status inválido: {other} (esperado pending|in_progress|done|abandoned)"
        ),
    }
}

/// Localized message for `invalid_tz`.
pub fn invalid_tz(v: &str) -> String {
    match current() {
        Language::English => {
            format!("display.tz invalid: '{v}'; use an IANA name like 'America/Sao_Paulo'")
        }
        Language::Portuguese => {
            format!("display.tz inválido: '{v}'; use um nome IANA como 'America/Sao_Paulo'")
        }
    }
}

/// `--enable-ner` and `--skip-extraction` conflict.
pub fn enable_ner_skip_extraction_exclusive() -> String {
    match current() {
        Language::English => {
            "--enable-ner and --skip-extraction are mutually exclusive; remove one".to_string()
        }
        Language::Portuguese => {
            "--enable-ner e --skip-extraction são mutuamente exclusivos; remova um".to_string()
        }
    }
}

/// --entity required when --memory is used.
pub fn entity_required_when_memory() -> String {
    match current() {
        Language::English => "--entity is required when --memory is used".to_string(),
        Language::Portuguese => "--entity é obrigatório quando --memory é usado".to_string(),
    }
}

/// --from required when not using --entity/--all.
pub fn from_required_without_entity_all() -> String {
    match current() {
        Language::English => "--from is required when --entity/--all is not used".to_string(),
        Language::Portuguese => {
            "--from é obrigatório quando --entity/--all não é usado".to_string()
        }
    }
}

/// Batch reclassify missing --from-type.
pub fn from_type_required_batch() -> String {
    match current() {
        Language::English => "--from-type is required in batch mode".to_string(),
        Language::Portuguese => "--from-type é obrigatório no modo batch".to_string(),
    }
}

/// --to required when not using --entity/--all.
pub fn to_required_without_entity_all() -> String {
    match current() {
        Language::English => "--to is required when --entity/--all is not used".to_string(),
        Language::Portuguese => "--to é obrigatório quando --entity/--all não é usado".to_string(),
    }
}

/// Batch reclassify missing --to-type.
pub fn to_type_required_batch() -> String {
    match current() {
        Language::English => "--to-type is required in batch mode".to_string(),
        Language::Portuguese => "--to-type é obrigatório no modo batch".to_string(),
    }
}

/// Memory/entity name required as positional or --name.
pub fn name_required_positional_or_flag() -> String {
    match current() {
        Language::English => "name required: pass as positional argument or via --name".to_string(),
        Language::Portuguese => {
            "nome obrigatório: passe como argumento posicional ou via --name".to_string()
        }
    }
}

/// `split-body`: neither a name nor `--batch` was supplied.
///
/// Distinct from [`name_required_positional_or_flag`] because this verb has a
/// second legitimate way to be complete — operating on every oversized memory —
/// and a message naming only the name would read as though batch mode did not
/// exist. GAP-SG-272 added the positional spelling, so the text names all three
/// ways in rather than the one flag it used to hardcode in English only.
pub fn split_body_needs_name_or_batch() -> String {
    match current() {
        Language::English => "name required: pass it as a positional argument or via --name, \
             or use --batch to split every oversized memory"
            .to_string(),
        Language::Portuguese => {
            "nome obrigatório: passe como argumento posicional ou via --name, ou use \
             --batch para dividir toda memória acima do teto"
                .to_string()
        }
    }
}

/// Neither a name nor `--id` was supplied to a verb that accepts both.
///
/// One function for two call sites — `read` and `rename-entity` — which each
/// carried their own English literal, and had already drifted: one listed the
/// positional spelling and the other did not, for the same choice.
pub fn name_or_id_required() -> String {
    match current() {
        Language::English => "name or --id required: pass the name as a positional \
             argument, via --name, or identify the row with --id"
            .to_string(),
        Language::Portuguese => {
            "nome ou --id obrigatório: passe o nome como argumento posicional, via \
             --name, ou identifique a linha com --id"
                .to_string()
        }
    }
}

/// `reclassify` single mode: neither `--new-type` nor `--description` was given.
///
/// Previously an English literal inline in the handler, which made the refusal
/// untranslatable in a product that ships every other message in two languages.
pub fn reclassify_needs_type_or_description() -> String {
    match current() {
        Language::English => {
            "at least one of --new-type or --description is required in single mode".to_string()
        }
        Language::Portuguese => {
            "pelo menos um entre --new-type e --description é obrigatório no modo single"
                .to_string()
        }
    }
}

/// Single-mode missing name.
///
/// GAP-SG-272: the text names the positional spelling too, because the verbs that
/// read this message now accept both. A refusal that names only the flag sends the
/// operator to `--help` to discover the form they could have used.
pub fn name_required_single_mode() -> String {
    match current() {
        Language::English => {
            "name required in single mode: pass it as a positional argument or via --name"
                .to_string()
        }
        Language::Portuguese => {
            "nome obrigatório no modo single: passe como argumento posicional ou via --name"
                .to_string()
        }
    }
}

/// Single-mode missing --target.
pub fn target_required_single_mode() -> String {
    match current() {
        Language::English => "--target is required in single mode".to_string(),
        Language::Portuguese => "--target é obrigatório no modo single".to_string(),
    }
}

/// CREATE path requires `--type` and `--description`.
pub fn type_and_description_required() -> String {
    match current() {
        Language::English => {
            "--type and --description are required when creating a new memory".to_string()
        }
        Language::Portuguese => {
            "--type e --description são obrigatórios ao criar uma nova memória".to_string()
        }
    }
}

/// `--target` only applies to re-embed operation.
pub fn reembed_target_only(target: &str) -> String {
    match current() {
        Language::English => {
            format!("--target {target} only applies to --operation re-embed")
        }
        Language::Portuguese => {
            format!("--target {target} só se aplica a --operation re-embed")
        }
    }
}

/// Refusing orphan entity delete without --yes.
/// Counts rows, not entities: the caller sums orphan entities, dangling
/// relationships and every other foreign key violation in the file, and a
/// message that named only one of the three would understate what is about to
/// be deleted.
pub fn refuse_delete_orphans_without_yes(orphan_count: usize) -> String {
    match current() {
        Language::English => format!(
            "refusing to delete {orphan_count} orphaned rows without --yes (use --dry-run to preview)"
        ),
        Language::Portuguese => format!(
            "recusando excluir {orphan_count} linhas órfãs sem --yes (use --dry-run para pré-visualizar)"
        ),
    }
}

/// Refusing vec orphan delete without --yes.
pub fn refuse_delete_vec_orphans_without_yes(
    orphan_count: i64,
    orphan_entities_count: i64,
    orphan_chunks_count: i64,
) -> String {
    match current() {
        Language::English => format!(
            "refusing to delete {orphan_count} memory embedding + {orphan_entities_count} vec_entities + {orphan_chunks_count} vec_chunks orphan rows without --yes (use --dry-run to preview)"
        ),
        Language::Portuguese => format!(
            "recusando excluir {orphan_count} embeddings de memória + {orphan_entities_count} vec_entities + {orphan_chunks_count} vec_chunks órfãos sem --yes (use --dry-run para pré-visualizar)"
        ),
    }
}

/// Refusing to release slot without --yes.
pub fn refuse_release_slot_without_yes(slot_id: &str, path: &str) -> String {
    match current() {
        Language::English => {
            format!("refusing to release slot {slot_id} without --yes (file: {path})")
        }
        Language::Portuguese => {
            format!("recusando liberar slot {slot_id} sem --yes (arquivo: {path})")
        }
    }
}

/// Self-referential merge by id (generic).
pub fn self_merge_id(id: i64, target_id: i64) -> String {
    match current() {
        Language::English => format!(
            "source entity id={id} equals target id={target_id} — \
             self-referential merge is not allowed"
        ),
        Language::Portuguese => format!(
            "entidade fonte id={id} é igual ao alvo id={target_id} — \
             merge auto-referencial não é permitido"
        ),
    }
}

/// Self-referential merge by id (pre-check with --ids hint).
pub fn self_merge_id_in_ids(id: i64) -> String {
    match current() {
        Language::English => format!(
            "source entity id={id} equals target id={id} — \
             self-referential merge is not allowed (remove target from --ids)"
        ),
        Language::Portuguese => format!(
            "entidade fonte id={id} é igual ao alvo id={id} — \
             merge auto-referencial não é permitido (remova o alvo de --ids)"
        ),
    }
}

/// Self-referential merge by name (generic).
pub fn self_merge_name(name: &str, target_name: &str) -> String {
    match current() {
        Language::English => format!(
            "source entity '{name}' equals target '{target_name}' — \
             self-referential merge is not allowed"
        ),
        Language::Portuguese => format!(
            "entidade fonte '{name}' é igual ao alvo '{target_name}' — \
             merge auto-referencial não é permitido"
        ),
    }
}

/// Self-referential merge by name (pre-check with --names hint).
pub fn self_merge_name_in_names(name: &str) -> String {
    match current() {
        Language::English => format!(
            "source entity '{name}' equals target '{name}' — \
             self-referential merge is not allowed (remove target from --names)"
        ),
        Language::Portuguese => format!(
            "entidade fonte '{name}' é igual ao alvo '{name}' — \
             merge auto-referencial não é permitido (remova o alvo de --names)"
        ),
    }
}

/// Source name resolves to target id (self-merge).
pub fn self_merge_name_resolves_to_target(name: &str, target_id: i64) -> String {
    match current() {
        Language::English => format!(
            "source entity '{name}' resolves to the target (id={target_id}) — \
             self-referential merge is not allowed"
        ),
        Language::Portuguese => format!(
            "entidade fonte '{name}' resolve para o alvo (id={target_id}) — \
             merge auto-referencial não é permitido"
        ),
    }
}

/// Source and target entity names identical.
pub fn source_target_entity_names_identical() -> String {
    match current() {
        Language::English => "source and target entity names are identical".to_string(),
        Language::Portuguese => "nomes de entidade de origem e destino são idênticos".to_string(),
    }
}

/// Source and target names identical.
pub fn source_target_names_identical() -> String {
    match current() {
        Language::English => "source and target names are identical".to_string(),
        Language::Portuguese => "nomes de origem e destino são idênticos".to_string(),
    }
}

/// GAP-SG-142: `--filter` expression has an empty key on the left-hand side.
pub fn agent_surface_filter_empty_key(expr: &str) -> String {
    match current() {
        Language::English => {
            format!("invalid --filter expression '{expr}': the key must not be empty")
        }
        Language::Portuguese => {
            format!("expressão --filter inválida '{expr}': a chave não pode ser vazia")
        }
    }
}

/// GAP-SG-142: `--filter` expression carries no recognised operator.
pub fn agent_surface_filter_invalid(expr: &str) -> String {
    match current() {
        Language::English => format!(
            "invalid --filter expression '{expr}': expected key=value, key!=value or key~substring"
        ),
        Language::Portuguese => format!(
            "expressão --filter inválida '{expr}': esperado chave=valor, chave!=valor ou chave~substring"
        ),
    }
}

/// Unknown pending_embeddings status string.
pub fn unknown_pending_embeddings_status(other: &str) -> String {
    match current() {
        Language::English => format!("unknown pending_embeddings status: {other}"),
        Language::Portuguese => {
            format!("status de pending_embeddings desconhecido: {other}")
        }
    }
}

/// Generic "missing / required field" helper.
pub fn missing_field(field: &str) -> String {
    match current() {
        Language::English => format!("missing required field: {field}"),
        Language::Portuguese => format!("campo obrigatório ausente: {field}"),
    }
}

/// Sub-queries file has no usable lines.
pub fn sub_queries_file_empty(path: &str) -> String {
    match current() {
        Language::English => format!("sub-queries file '{path}' has no usable lines"),
        Language::Portuguese => {
            format!("arquivo de sub-consultas '{path}' não tem linhas utilizáveis")
        }
    }
}

/// Failed to read names file.
pub fn failed_to_read_names_file(path: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to read names file {path}: {err}"),
        Language::Portuguese => format!("falha ao ler arquivo de nomes {path}: {err}"),
    }
}

/// Localized message for `sync_destination_equals_source`.
pub fn sync_destination_equals_source() -> String {
    match current() {
        Language::English => {
            "destination path must differ from the source database path".to_string()
        }
        Language::Portuguese => {
            "caminho de destino deve ser diferente do caminho do banco de dados fonte".to_string()
        }
    }
}

/// `schema --name <ID>` was given an id that is not shipped in `docs/schemas/`.
///
/// `suggestion` carries the nearest known id when one is close enough; the
/// message stays terse when nothing is, because an unrelated pointer is worse
/// than none.
pub fn unknown_schema_id(id: &str, suggestion: Option<&str>) -> String {
    match (current(), suggestion) {
        (Language::English, Some(near)) => {
            format!("unknown schema id '{id}'; did you mean '{near}'? Run `sqlite-graphrag schema` to list every id")
        }
        (Language::English, None) => {
            format!("unknown schema id '{id}'; run `sqlite-graphrag schema` to list every id")
        }
        (Language::Portuguese, Some(near)) => {
            format!("id de schema desconhecido '{id}'; você quis dizer '{near}'? Execute `sqlite-graphrag schema` para listar todos os ids")
        }
        (Language::Portuguese, None) => {
            format!("id de schema desconhecido '{id}'; execute `sqlite-graphrag schema` para listar todos os ids")
        }
    }
}

/// A stdin-reading path was requested while `--no-input` was in force.
///
/// Reported before the read is attempted, so the guarantee holds even when a
/// pipe is present and would have supplied data.
pub fn no_input_blocks_stdin() -> String {
    match current() {
        Language::English => "--no-input is in force: this invocation refuses to read stdin; \
             supply the content through a file or inline flag \
             (for example --body / --body-file), or drop --no-input"
            .to_string(),
        Language::Portuguese => "--no-input está em vigor: esta invocação recusa ler stdin; \
             forneça o conteúdo por arquivo ou flag inline \
             (por exemplo --body / --body-file), ou remova --no-input"
            .to_string(),
    }
}
