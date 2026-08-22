# ADR-0015 — Preservation Gate Jaccard (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Alice Martins (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G29 Passo 4 (validação de preservação), G29 Passo 5 (idempotência blake3).

## Contexto

`enrich --operation body-enrich` chama um LLM para expandir um body curto de memória. O LLM pode inventar fatos, descartar tokens críticos ou devolver um body que se afasta do original. Sem um gate, todo `body-enrich` é uma aposta: uma alucinação é persistida em silêncio e `restore --version N` é a única válvula de escape. Reprocessar a mesma memória também é inseguro porque `persist_enriched_body` sempre insere uma nova versão mesmo quando o LLM produziu um body idêntico byte a byte (raro, mas possível).

## Decisão

1. Criar `src/preservation.rs` com `jaccard_similarity(a: &str, b: &str) -> f64` que opera sobre trigramas de caractere (seguro em UTF-8) e um enum `PreservationVerdict` com as variantes `Preserved { score, threshold }`, `Rejected { score, threshold }` e `Unchanged { byte_len }`. 10 testes unitários cobrem condições de borda (0.0, 0.5, 0.7, 1.0), trigramas, strings vazias e Unicode.
2. Adicionar `--preserve-threshold <FLOAT>` a `EnrichArgs` com default 0.7. O threshold é a similaridade Jaccard mínima entre o body original e o enriquecido exigida para persistir.
3. Em `call_body_enrich`, DEPOIS da chamada ao LLM, calcular a similaridade Jaccard. Se `score < threshold`, retornar `EnrichItemResult::PreservationFailed { score, threshold, chars_before, chars_after }` e NÃO chamar `memories::update`.
4. Adicionar idempotência via `blake3::hash`. Calcular `old_hash = blake3(body)` e `new_hash = blake3(enriched_body)`. Se os hashes forem iguais, retornar `EnrichItemResult::Skipped { reason: "enriched body hash matches original (blake3:{hash}); idempotency skip" }` ANTES da checagem Jaccard.
5. A ordem de verificação é: (a) idempotência blake3, (b) preservação Jaccard, (c) sanidade de comprimento `chars_after <= chars_before`, (d) `memories::update`. Uma falha em qualquer passo emite uma variante de `EnrichItemResult` e pula a persistência.

## Consequências

- Bodies alucinados com baixa sobreposição de tokens são rejeitados no gate, e não em `history --name <X>` depois do fato.
- Reprocessar a mesma memória é seguro: hashes idênticos retornam `Skipped`, hashes divergentes que falham no teste Jaccard retornam `PreservationFailed`, e hashes divergentes que passam no teste Jaccard persistem normalmente.
- O stream NDJSON inclui eventos `preservation_failed` com o score Jaccard, para que operadores possam auditar rejeições.
- O threshold é configurável por invocação, então o CI pode baixá-lo para testes rápidos e operadores podem elevá-lo para corpora de alta precisão.
- 10 + 0 = 10 novos testes (a lógica do gate é exercitada nos 745 testes existentes).

## Alternativas Consideradas

- Usar BLEU ou ROUGE em vez de Jaccard. REJEITADO. Jaccard sobre trigramas é livre de dependências, rápido e adequado para bodies curtos.
- Usar um segundo LLM como juiz. REJEITADO. O custo e a latência dobrados não se justificam para a release v1.0.69; um ADR futuro pode adicionar uma flag `--judge-model`.
- Pular a checagem de sanidade de comprimento (passo c). REJEITADO. Um body mais curto que o original é quase sempre uma regressão.

## Referências

- `src/preservation.rs` (10 testes).
- `src/commands/enrich.rs:2127-2158` (variante `EnrichItemResult::PreservationFailed`).
- `src/commands/enrich.rs:2488-2500` (idempotência blake3).
- `src/commands/enrich.rs:2404-2448` (gate Jaccard).
- gaps.md G29 Passos 4-5 linhas 823-851.
