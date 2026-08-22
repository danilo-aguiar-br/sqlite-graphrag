# ADR-0068: v1.2.8 — The entity_type vocabulary is open, not merely wider

- Status: Accepted
- Date: 2026-08-18
- Release: v1.2.8 (crate `1.2.8`)
- Supersedes: the implicit policy introduced by `V001__init.sql` and extended by `V008__expand_entity_types.sql`
- Superseded by: none
- Related: ADR-0069 (foreign keys around migrations), GAP-SG-277, GAP-SG-278, GAP-SG-216, `V010__open_relation_vocabulary.sql`


## Context

`entity_type` accepted thirteen values. Any other label was folded onto the nearest one, terminating at `concept`, and the string the caller wrote was destroyed inside `impl Deserialize` before any layer above could see it.

Measured on this workspace's database on 2026-08-18:

| type | entities | share |
| --- | --- | --- |
| `concept` | 10 902 | 69,3 % |
| every other kind | 4 842 | 30,7 % |
| **total** | **15 744** | |

Filtering by `--entity-type concept` returns two thirds of the graph, which is indistinguishable from not filtering. The more eloquent number is the small one: `person` held 17 nodes in a corpus that discusses people constantly.

The vocabulary also failed to describe its own domain. This corpus is about building a Rust CLI, and the labels that would describe it — `crate`, `gap`, `flag`, `migration`, `schema` — were none of the thirteen, so all five collapsed into `concept`.

Two constraints framed the decision:

- The project's own rules cap a public enum at 12 variants. `EntityType` had 13, so it already violated that rule, which ruled out "add more kinds" as a remedy.
- `gaps.md` had already ruled out refusing unknown labels by default, because agents emit free-form labels through `--graph-stdin` and refusing would break them.


## Decision

Open the vocabulary rather than widen it, reusing the pattern this repository already runs in production.

`V010__open_relation_vocabulary.sql` did exactly this for relations in v1.0.49: it removed the `CHECK` on `relationships.relation`, moved the canonical list into `parsers::CANONICAL_RELATIONS` as advice, and gave `link` a `--strict-relations` flag for callers who want the closed set. That pattern was never applied back to `entity_type`, and its own comment says it follows `V008__expand_entity_types.sql`.

Accordingly:

- `V017__open_entity_type_vocabulary.sql` drops the `CHECK` on `entities.type`.
- `EntityType` ceases to be an enum. `CANONICAL_ENTITY_TYPES`, `is_canonical_entity_type` and `normalize_entity_type` mirror the relation trio.
- `normalize_entity_type` enforces **shape only** — trim, lowercase, hyphen to underscore — and refuses only labels that could not be a word in any vocabulary: empty, digits only, containing a line break, or over `MAX_ENTITY_TYPE_LEN` characters.
- Membership is never a reason to refuse there. It is enforced one layer up, by `--strict-entity-types`, where the caller asked for it.
- A non-canonical label is reported in the response `warnings` array and stored as written.
- `graph entity-types` reports the vocabulary a database actually uses, with a `canonical` flag per row.


## Consequences

The correction is by **subtraction**. GAP-SG-277 proposed a `raw_type` column to preserve the caller's label, and GAP-SG-278 proposed an `entity_types` table with a foreign key plus an XDG key. Neither was built, and neither is needed: with nothing folding the label, it survives by construction. A vocabulary table would exist only to refuse someone, and the decision is never to refuse.

Declared residue, which no later change undoes:

- Entities written before v1.2.8 remain `concept`, and their original label is gone. Reclassifying them without it would be guessing, and this project does not record guessing as a correction.
- `--entity-type concept` returns **less** from this release onward, because `framework` is now stored as `framework`.
- An open vocabulary can fragment the filter over time. There is no cap, by the owner's decision. `graph entity-types` is the instrument that makes such fragmentation visible if it happens.

The recovery path exists but was not run: `enrich --operation entity-type-validate` can now propose a specific label instead of choosing among ten, and its scan gained the filter it needed to target `concept`. Running it costs one LLM call per entity, so it is the operator's call.
