# ADR-0070: v1.2.8 — An enrich operation that writes to the graph must see evidence, or abstain

- Status: Accepted
- Date: 2026-08-18
- Release: v1.2.8 (crate `1.2.8`)
- Supersedes: the unwritten convention that an operation's `input_text` was a local decision of its call site
- Superseded by: none
- Related: ADR-0068 (open entity_type vocabulary), GAP-SG-277, GAP-SG-278, GAP-SG-279


## Context

`enrich --operation entity-type-validate` decided an entity's type and wrote the answer to `entities.type`. What it showed the model, in full:

```rust
let input_text = format!("Entity: {ent_name}\nCurrent type: {ent_type}");
```

Two lines: the entity's name, and the label being disputed. No description, no linked corpus, no typed neighbours. The `SELECT` immediately above it read `id, name, type` and stopped, although `entities.description` has existed since V017 and the row was already in hand.

This mattered more than it would in another operation. GAP-SG-277 measured 10 898 of 15 744 entities collapsed into `concept` by the closed vocabulary, and GAP-SG-278 named `entity-type-validate` as the path back. The operation asked to repair two thirds of the graph was the one deciding from the weakest input in the pipeline — one paid call per entity, judging labels like `rd_gs` and `v017` by how they are spelled.

Nothing was hidden. It was invisible, which is different. The `format!` sat among five siblings in the same module whose calls looked alike at a glance and carried three to five fields each. Reading the file told you it compiled.

Two guards already watched the other halves of that decision. `entity_type_vocabulary_contract` fails the build when the prompt and `CANONICAL_ENTITY_TYPES` disagree. `normalize_entity_type` constrains what reaches the column. Between a watched prompt and a watched output sat an unwatched input, and the input was the defect.

The asymmetry inside the crate made it plain. `entity-descriptions` had four XDG keys for gathering evidence — `corpus_top_k`, `snippet_chars`, `neighbour_top_k`, `min_corpus_chars` — and a pre-request gate that abstains when there is nothing to describe from. `entity-type-validate` had none of it. One operation had been designed to look before writing a sentence; the other wrote a label with its eyes closed.


## Decision

**An enrich operation that writes to `entities` or `relationships` must be shown evidence about its subject, and must abstain when there is none.**

Three parts, in the order they take effect.

### 1. Evidence is gathered before the request, from the shared source of truth

`call_entity_type_validate` selects `description` on the query it already ran, then calls `load_entity_evidence_tuned` — the same assembly `entity-descriptions` and the `--status` sampler read from. Reuse here is not code economy. It is what makes the three agree on what "what we know about this entity" means; when the sampler measured only bodies while the writer also saw edges, the reported quality described a corpus that never existed.

### 2. Tuning is per operation, because the operations buy different amounts

Four keys, mirroring the description path exactly:

- `enrich.entity_type_validate.corpus_top_k` (8)
- `enrich.entity_type_validate.snippet_chars` (2000)
- `enrich.entity_type_validate.neighbour_top_k` (12)
- `enrich.entity_type_validate.min_corpus_chars` (40)

They start at the description path's values because the evidence needed is the same evidence. They are separate keys because one operation writes a sentence and the other rewrites a label across ten thousand rows, and an operator has every reason to pay for more context before the second.

### 3. Absence of evidence is a reason to abstain, not a licence to guess

`should_abstain_from_type_judgement` refuses an entity with neither a description nor sufficient linked corpus, **before** the request, at zero cost. A description alone passes: it is thin, but it is a statement about the subject rather than a reading of its name.

The response schema gained `sufficient_evidence` and a nullable `validated_type`, so the model has somewhere to put "I cannot tell" other than a plausible-looking label. That shape is imposed by the transport, not by taste: OpenRouter sends every schema under `strict: true`, and strict mode requires every key in `properties` to appear in `required` — an "optional" field is a refused request, not a lenient one.


## Consequences

### The defect was in three operations, not one

`tests/enrich_input_evidence_gate.rs` was written to keep this from returning. On its first run it failed two operations nobody had looked at:

| operation | what it saw | what it wrote |
| --- | --- | --- |
| `weight-calibrate` | two entity names, relation, current weight | `UPDATE relationships SET weight` |
| `relation-reclassify` | two entity names, current relation | `UPDATE relationships SET relation, weight` |

Same defect, same class, same silence. Both were corrected by selecting `description` for each endpoint — two extra columns on the join that was already running, and no extra query. That is characteristic of an input defect rather than an access one: the data is usually one column away from whoever decided without it.

### The envelope now says what a paid drain changed

Reclassification, confirmation and discarded suggestion used to emit an identical `Done { entities: 1 }`. An operator who paid for ten thousand calls could not answer "how many labels moved" from the output, only by diffing against a backup. `EnrichItemResult::Retyped` carries the previous label, the new one and the evidence size; `retyped` counts them in the summary; and a skip now carries its reason to the caller instead of only into the sidecar.

`Retyped` is a new variant rather than fields on `Done` for an arithmetic reason: `Done` is constructed in thirty places, and the enum is matched exhaustively in three.

### Costs accepted

Prompt input grows by roughly the evidence budget per item, multiplied by however many entities a drain touches. This is the price of the decision being grounded, and it is bounded by the four keys above.

This ADR does **not** record which input has the best accuracy-to-cost ratio. That is only measurable by comparing paid samples with different inputs against the same corpus, and it remains open.

An entity with no description, no neighbour and no corpus stays undecidable by any input. For that entity abstention is the only honest answer, and it now costs nothing.


## Alternatives considered

**Keep the two-line input and accept the noise.** Rejected: the operation writes to a column, and a guess that reaches storage is indistinguishable from a measurement once it is there.

**Add fields to `EnrichItemResult::Done`.** Rejected on cost: thirty construction sites against three match arms.

**Reuse `load_entity_evidence` unchanged, with the description path's keys.** Rejected: it would have tied the budget of a ten-thousand-row rewrite to the budget of writing one sentence, and any operator tuning one would have silently moved the other.

**Exempt the two edge operations, since an edge has no body.** Rejected: an edge has no body, but its endpoints have descriptions, and the graph already stored them.
