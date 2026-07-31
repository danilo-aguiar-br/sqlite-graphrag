use crate::i18n::{current, Language};

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

/// Failed to parse an ExtractionResult from a provider.
pub fn failed_to_parse_extraction(provider: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => {
            format!("failed to deserialize {provider} output as ExtractionResult: {err}")
        }
        Language::Portuguese => {
            format!("falha ao deserializar saída de {provider} como ExtractionResult: {err}")
        }
    }
}

/// Codex turn failed with a message.
pub fn codex_turn_failed(message: &str) -> String {
    match current() {
        Language::English => format!("codex turn failed: {message}"),
        Language::Portuguese => format!("turno codex falhou: {message}"),
    }
}

/// Failed to parse codex agent_message as ExtractionResult.
pub fn failed_to_parse_codex_agent_message(err: &impl std::fmt::Display, text: &str) -> String {
    match current() {
        Language::English => {
            format!("failed to parse codex agent_message as ExtractionResult: {err}. text={text}")
        }
        Language::Portuguese => format!(
            "falha ao parsear agent_message do codex como ExtractionResult: {err}. text={text}"
        ),
    }
}

/// Claude -p failed with an error message.
pub fn claude_p_failed(err: &str) -> String {
    match current() {
        Language::English => format!("claude -p failed: {err}"),
        Language::Portuguese => format!("claude -p falhou: {err}"),
    }
}

/// Mode-conditional flag conflicts (G20).
pub fn mode_flag_conflicts(mode: &str, conflicts: &str) -> String {
    match current() {
        Language::English => format!(
            "G20: mode-conditional flag conflicts detected for --mode={mode}:\n  - {conflicts}"
        ),
        Language::Portuguese => format!(
            "G20: conflitos de flags condicionais ao modo detectados para --mode={mode}:\n  - {conflicts}"
        ),
    }
}

// ── GAP-SG-110/123: remaining Validation catalog helpers ──────────────────
// Prefer these over ad-hoc AppError::Validation construction so EN/PT stay
// in one place.

/// Config file is a symlink (potential attack).
pub fn config_file_is_symlink(path: &str) -> String {
    match current() {
        Language::English => format!("config file is a symlink (potential attack): {path}"),
        Language::Portuguese => {
            format!("arquivo de config é um symlink (potencial ataque): {path}")
        }
    }
}

/// Config parse error at path.
pub fn config_parse_error(path: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("config parse error in {path}: {err}"),
        Language::Portuguese => format!("erro de parse de config em {path}: {err}"),
    }
}

/// Config path has no parent directory component.
pub fn config_path_no_parent(path: &str) -> String {
    match current() {
        Language::English => format!("config path has no parent: {path}"),
        Language::Portuguese => format!("caminho de config sem diretório pai: {path}"),
    }
}

/// Config file owned by a different uid; refuse overwrite.
pub fn config_file_wrong_owner(path: &str, file_uid: u32, my_uid: u32) -> String {
    match current() {
        Language::English => format!(
            "config file {path} owned by uid {file_uid}, not current uid {my_uid}; refusing to overwrite"
        ),
        Language::Portuguese => format!(
            "arquivo de config {path} pertence ao uid {file_uid}, não ao uid atual {my_uid}; recusando sobrescrever"
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

/// Embedded schema JSON is invalid.
pub fn embedded_schema_invalid_json(name: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("embedded schema for {name} is not valid JSON: {err}"),
        Language::Portuguese => {
            format!("schema embutido para {name} não é JSON válido: {err}")
        }
    }
}

/// Invalid memory source string.
pub fn invalid_memory_source(other: &str, expected: &str) -> String {
    match current() {
        Language::English => {
            format!("invalid memory source: {other}; expected one of {expected}")
        }
        Language::Portuguese => {
            format!("fonte de memória inválida: {other}; esperado um de {expected}")
        }
    }
}

/// Legacy embedding extraction backend removed.
pub fn legacy_embedding_backend_removed(model: &str) -> String {
    match current() {
        Language::English => format!(
            "the legacy embedding extraction backend was removed in v1.0.79 \
             (the CLI is LLM-only); use --extraction-backend llm instead. \
             Model requested: {model}"
        ),
        Language::Portuguese => format!(
            "o backend legado de extração por embedding foi removido em v1.0.79 \
             (a CLI é apenas LLM); use --extraction-backend llm. \
             Modelo solicitado: {model}"
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

/// Unknown pending_memories status string.
pub fn unknown_pending_memories_status(other: &str) -> String {
    match current() {
        Language::English => format!("unknown pending_memories status: {other}"),
        Language::Portuguese => {
            format!("status de pending_memories desconhecido: {other}")
        }
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

/// Refusing orphan entity delete without --yes.
pub fn refuse_delete_orphans_without_yes(orphan_count: usize) -> String {
    match current() {
        Language::English => format!(
            "refusing to delete {orphan_count} orphan entities without --yes (use --dry-run to preview)"
        ),
        Language::Portuguese => format!(
            "recusando excluir {orphan_count} entidades órfãs sem --yes (use --dry-run para pré-visualizar)"
        ),
    }
}

/// Failed to run opencode --version.
pub fn failed_to_run_opencode_version(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to run opencode --version: {err}"),
        Language::Portuguese => format!("falha ao executar opencode --version: {err}"),
    }
}

/// Could not parse opencode version string.
pub fn could_not_parse_opencode_version(raw: &str) -> String {
    match current() {
        Language::English => format!("could not parse opencode version from: {raw}"),
        Language::Portuguese => {
            format!("não foi possível parsear a versão do opencode de: {raw}")
        }
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

/// Entity name too short.
pub fn entity_name_too_short(name: &str) -> String {
    match current() {
        Language::English => format!("entity name '{name}' must be at least 2 characters"),
        Language::Portuguese => {
            format!("nome de entidade '{name}' deve ter pelo menos 2 caracteres")
        }
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

/// Non-canonical relation under --strict-relations.
pub fn non_canonical_relation(relation: &str, allowed: &str) -> String {
    match current() {
        Language::English => format!(
            "non-canonical relation '{relation}': use --strict-relations=false or choose from: {allowed}"
        ),
        Language::Portuguese => format!(
            "relação não canônica '{relation}': use --strict-relations=false ou escolha entre: {allowed}"
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

/// Batch line: invalid JSON.
pub fn batch_line_invalid_json(index: usize, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("line {index}: invalid JSON: {err}"),
        Language::Portuguese => format!("linha {index}: JSON inválido: {err}"),
    }
}

/// Batch line: name normalizes empty.
pub fn batch_line_name_empty(index: usize) -> String {
    match current() {
        Language::English => format!("line {index}: name normalizes to empty string"),
        Language::Portuguese => {
            format!("linha {index}: nome normaliza para string vazia")
        }
    }
}

/// Batch line: type/description required.
pub fn batch_line_type_description_required(index: usize) -> String {
    match current() {
        Language::English => format!(
            "line {index}: --type and --description are required when creating a new memory"
        ),
        Language::Portuguese => format!(
            "linha {index}: --type e --description são obrigatórios ao criar uma nova memória"
        ),
    }
}

/// Executable not found in PATH (generic install wording).
pub fn executable_not_in_path_generic(binary: &str) -> String {
    match current() {
        Language::English => format!(
            "executable '{binary}' not found in PATH; ensure it is installed and accessible"
        ),
        Language::Portuguese => format!(
            "executável '{binary}' não encontrado no PATH; certifique-se de que está instalado e acessível"
        ),
    }
}

/// Failed to parse claude output as JSON array.
pub fn failed_to_parse_claude_json_array(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse claude output as JSON array: {err}"),
        Language::Portuguese => {
            format!("falha ao parsear saída do claude como array JSON: {err}")
        }
    }
}

/// Claude extraction failed.
pub fn claude_extraction_failed(err: &str) -> String {
    match current() {
        Language::English => format!("claude extraction failed: {err}"),
        Language::Portuguese => format!("extração claude falhou: {err}"),
    }
}

/// Failed to parse claude result field as JSON.
pub fn failed_to_parse_claude_result_field(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse claude result field as JSON: {err}"),
        Language::Portuguese => {
            format!("falha ao parsear campo result do claude como JSON: {err}")
        }
    }
}

/// Codex model not supported with ChatGPT Pro OAuth.
pub fn codex_model_not_supported_oauth(model: &str, accepted: &str) -> String {
    match current() {
        Language::English => format!(
            "--codex-model {model:?} is not supported with ChatGPT Pro OAuth. \
             Accepted: {accepted}"
        ),
        Language::Portuguese => format!(
            "--codex-model {model:?} não é suportado com ChatGPT Pro OAuth. \
             Aceitos: {accepted}"
        ),
    }
}

/// No agent_message in codex JSONL output.
pub fn no_agent_message_in_codex_jsonl(
    rate_limited: bool,
    schema_error: bool,
    turn_failed: bool,
) -> String {
    match current() {
        Language::English => format!(
            "no agent_message in codex JSONL output (rate_limited={rate_limited}, schema_error={schema_error}, turn_failed={turn_failed})"
        ),
        Language::Portuguese => format!(
            "nenhum agent_message na saída JSONL do codex (rate_limited={rate_limited}, schema_error={schema_error}, turn_failed={turn_failed})"
        ),
    }
}

/// Codex rate-limited message.
pub fn codex_rate_limited(message: &str) -> String {
    match current() {
        Language::English => format!("codex rate-limited: {message}"),
        Language::Portuguese => format!("codex com limite de taxa: {message}"),
    }
}

/// Failed to parse codex agent_message as JSON.
pub fn failed_to_parse_codex_agent_message_json(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse codex agent_message as JSON: {err}"),
        Language::Portuguese => {
            format!("falha ao parsear agent_message do codex como JSON: {err}")
        }
    }
}

/// Codex agent_message is not valid JSON (with raw snippet).
pub fn codex_agent_message_not_valid_json(err: &impl std::fmt::Display, raw: &str) -> String {
    match current() {
        Language::English => {
            format!("codex agent_message is not valid JSON: {err}; raw={raw}")
        }
        Language::Portuguese => {
            format!("agent_message do codex não é JSON válido: {err}; raw={raw}")
        }
    }
}

/// Opencode response is not valid JSON.
pub fn opencode_response_not_valid_json(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("opencode response is not valid JSON: {err}"),
        Language::Portuguese => {
            format!("resposta do opencode não é JSON válido: {err}")
        }
    }
}

/// Failed to parse entities array.
pub fn failed_to_parse_entities_array(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse entities array: {err}"),
        Language::Portuguese => format!("falha ao parsear array de entidades: {err}"),
    }
}

/// Failed to parse relationships array.
pub fn failed_to_parse_relationships_array(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse relationships array: {err}"),
        Language::Portuguese => {
            format!("falha ao parsear array de relacionamentos: {err}")
        }
    }
}

/// Queue namespace migration failed.
pub fn queue_namespace_migration_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("queue namespace migration failed: {err}"),
        Language::Portuguese => {
            format!("migração de namespace da fila falhou: {err}")
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

/// Requeue-skipped failed.
pub fn requeue_skipped_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("requeue-skipped failed: {err}"),
        Language::Portuguese => format!("requeue-skipped falhou: {err}"),
    }
}

/// Requeue-dead failed.
pub fn requeue_dead_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("requeue-dead failed: {err}"),
        Language::Portuguese => format!("requeue-dead falhou: {err}"),
    }
}

/// Invalid chunk id in re-embed key.
pub fn invalid_chunk_id_in_reembed_key(chunk_key: &str) -> String {
    match current() {
        Language::English => format!("invalid chunk id in re-embed key: {chunk_key}"),
        Language::Portuguese => {
            format!("id de chunk inválido na chave de re-embed: {chunk_key}")
        }
    }
}

/// Invalid relationship id.
pub fn invalid_relationship_id(item_key: &str) -> String {
    match current() {
        Language::English => format!("invalid relationship id: {item_key}"),
        Language::Portuguese => format!("id de relacionamento inválido: {item_key}"),
    }
}

/// Preflight probe failed.
pub fn preflight_probe_failed(stderr: &str) -> String {
    match current() {
        Language::English => format!("preflight probe failed: {stderr}"),
        Language::Portuguese => format!("sonda de preflight falhou: {stderr}"),
    }
}

/// Preflight probe timed out.
pub fn preflight_probe_timed_out(secs: u64) -> String {
    match current() {
        Language::English => format!("preflight probe timed out after {secs}s"),
        Language::Portuguese => format!("sonda de preflight expirou após {secs}s"),
    }
}

/// Deep-research output path missing after atomic write.
pub fn deep_research_output_missing(path: &str) -> String {
    match current() {
        Language::English => {
            format!("deep-research --output failed: path does not exist after atomic write: {path}")
        }
        Language::Portuguese => format!(
            "deep-research --output falhou: caminho não existe após escrita atômica: {path}"
        ),
    }
}

/// Deep-research output file is empty.
pub fn deep_research_output_empty(path: &str) -> String {
    match current() {
        Language::English => {
            format!("deep-research --output failed: written file is empty (0 bytes): {path}")
        }
        Language::Portuguese => {
            format!("deep-research --output falhou: arquivo escrito está vazio (0 bytes): {path}")
        }
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
