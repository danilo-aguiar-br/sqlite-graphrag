# Briefing obrigatório — documentação v1.2.1 (sqlite-graphrag)

## Contexto
- Crate version: **1.2.1** (já em Cargo.toml). Schema **v16** (sem migrate main-DB).
- gaps.md (raiz) = SSOT dos bugs resolvidos nesta release; status RESOLVED.
- CHANGELOG.md e CHANGELOG.pt-BR.md já têm seção `[1.2.1] - 2026-07-31` no topo. NÃO reescrever o corpo inteiro do CHANGELOG.
- Mutações de arquivo: OBRIGATÓRIO usar `atomwrite --workspace /home/comandoaguiar/Dropbox/ai/dev/rust/linux/cli_sqlite-graphrag` (edit/replace/write). PROIBIDO sed -i, python write, tee redirecionado no projeto.
- Inglês canônico; PT-BR espelhado com acentos corretos.
- NÃO inventar features. Só documentar o que o binário faz.

## Temas CAPA v1.2.1 (obrigatórios em docs de operador / agent)
1. **Namespace isolation no claim** — `dequeue_next_pending` filtra `operation` + `namespace`. Drain em um ns não processa outro.
2. **`--until-empty` conta só op+ns** — `count_eligible_pending` (não all-ops).
3. **`--force-redescribe` reabre skipped/done** — `reopen_force_redescribe_candidates` uma vez por processo; nunca reabre dead.
4. **ReEmbed zombie reconcile** — `reconcile_satisfied_reembed_pending` marca done se `LENGTH(embedding)=dim*4` já ok.
5. **Elegibilidade re-embed por BLOB** — predicados usam LENGTH, não só coluna dim (CORRUPT/META_AHEAD).
6. **Enqueue entity: prefix** — lookup bare name; key na fila permanece `entity:…`; bare ok; missing rejeita.
7. **Enqueue chunk valida ns** — chunk_id deve existir em memória do namespace alvo não-deletada.
8. **CAPA-D** — markers "configuration file" compostos; sem bare `%configuration file%` FP.
9. Testes de regressão: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`; suite queue 38 OK.

## Inventário CLI completo (incluir em monografias operator-facing e skills)

Top-level (50 + help):
init, remember, remember-batch, ingest, recall, read, list, forget, purge, rename, split-body, edit, history, restore, hybrid-search, health, migrate, namespace-detect, optimize, stats, sync-safe-copy, backup, vacuum, link, unlink, deep-research, related, graph, export, fts, vec, codex-models, prune-relations, prune-ner, slots, pending, embedding, pending-embeddings, cleanup-orphans, memory-entities, cache, delete-entity, reclassify, rename-entity, merge-entities, enrich, reclassify-relation, normalize-entities, completions, config, help

Nested:
- graph: traverse, stats, entities, recompute-degree
- config: add-key, list-keys, remove-key, doctor, path, set, get, list, unset
- slots: status, release, cleanup
- pending: list, show, cleanup
- embedding: status, list, abandon
- pending-embeddings: list, status, abandon
- cache: clear-models, list, stats
- fts: rebuild, check, stats
- vec: orphan-list, purge-orphan, stats
- enrich inspectors: --status, --list-dead, --requeue-dead, --list-skipped, --requeue-skipped, --prune-dead-orphans, --prune-dead-entity-orphans
- enrich write flags relevantes 1.2.1: --until-empty, --force-redescribe, --operation re-embed --target memories|entities|chunks|all, --namespace, --mode openrouter, --rest-concurrency

## Contrato operacional vigente (manter, não regredir)
- flag > XDG config set > default; sem product env no hot path
- DEFAULT_EMBEDDING_DIM=1024
- --db DEPOIS do verbo
- GAP-SG-139: --db no-op em host leaves (config/slots/cache/codex-models/completions)
- e2e: scripts/e2e_offline_v120.sh 20/20
- list-skipped / requeue-skipped

## Fórmulas CLI prontas (exemplos a incluir)
sqlite-graphrag enrich --db "$DB" --operation re-embed --target entities --mode openrouter --openrouter-model MODEL --until-empty --namespace global -q --wait-lock 60
sqlite-graphrag enrich --db "$DB" --operation entity-descriptions --mode openrouter --openrouter-model MODEL --force-redescribe --until-empty --namespace global -q
sqlite-graphrag enrich --db "$DB" --status --operation re-embed --namespace global -q
sqlite-graphrag enrich --db "$DB" --list-skipped --operation entity-descriptions --namespace global -q
sqlite-graphrag enrich --db "$DB" --requeue-skipped --operation entity-descriptions --namespace global -q

## O que NÃO fazer
- Não criar ADR novo (notas no CHANGELOG + monografias)
- Não reescrever monografias inteiras se patch cirúrgico bastar
- Não apagar seções históricas; adicione seções v1.2.1 no topo ou atualize stamps Current 1.2.0 → 1.2.1
- Skills: PROIBIDO histórico de versões; PROIBIDO blocos de código longos; ≤5000 palavras; description ≤1054 chars e um único colon no YAML key; linguagem imperativa; TODOS os comandos da CLI
