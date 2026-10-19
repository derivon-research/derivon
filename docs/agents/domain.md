# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`GLOSSARY.md`** at the repo root, or
- **`GLOSSARY-MAP.md`** at the repo root if it exists: it points at one `GLOSSARY.md` per context. Read each one relevant to the topic.
- **`docs/adr/`**: read ADRs that touch the area you're about to work in. In multi-context repos, also check `src/<context>/docs/adr/` for context-scoped decisions.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The `/domain-modeling` skill (reached via `/grill-with-docs` and `/improve-codebase-architecture`) creates them lazily when terms or decisions actually get resolved.

## File structure

Single-context repo (most repos):

```
/
├── GLOSSARY.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

Multi-context repo (presence of `GLOSSARY-MAP.md` at the root):

```
/
├── GLOSSARY-MAP.md
├── docs/adr/                          ← system-wide decisions
└── src/
    ├── ordering/
    │   ├── GLOSSARY.md
    │   └── docs/adr/                  ← context-specific decisions
    └── billing/
        ├── GLOSSARY.md
        └── docs/adr/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `GLOSSARY.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal: either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders), but worth reopening because…_

## Shared vocabulary across the organisation

Core terms are shared across every `derivon-research` repo. Which repo is normative for which
vocabulary is kept in one table, in `docs/agents/domain.md` of `derivon-research/derivon-mindmap`;
read it before using a shared term. This repo is normative only for the graph protocol `derivon.graph/v1`.

Do not restate a term owned elsewhere. Link to its source instead. If two sources appear to
disagree, that is a real defect: surface it rather than picking one.

## Recorded decisions

Before treating a design question as open, check whether one of these indexes already settles
it. Read the index; open a record only when its one-line title bears on the question.

| Scope | Index |
| --- | --- |
| Product semantics, protocols, application architecture | `docs/adr/README.md` in `derivon-research/derivon-mindmap` |
| Authoring methodology | `docs/adr/README.md` in `derivon-research/skills` |
| Strategy, scope and schedule | *Decisions so far* in the open `wayfinder:map` issue of `derivon-research/planning` (private) |
