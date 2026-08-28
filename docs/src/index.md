# Derivon CLI

The Derivon CLI is a stateless JSON processor for non-negative weighted directed
B-hypergraphs. It is designed for Agents, scripts, and Unix pipelines.

This English manual is normative. The Chinese manual is a maintained translation; when
the two disagree, the English contract takes precedence.

The CLI accepts graph data, not an entire Derivon authoring document. Extract the graph
before invoking it:

```bash
jq '.graph' workspace.json | derivon validate
```

Every invocation processes exactly one complete graph. The CLI stores no graph between
commands and does not edit input files in place.

## Mathematical Scope

A Derivon hyperedge contains zero or more tail points, exactly one head point, and a
non-negative weight:

```text
h = (tails, head, weight)
```

All tails are required before the head can be derived. Multiple hyperedges with the same
head are alternative derivations. Empty tails, parallel hyperedges, and cycles are valid.

Derivon is not a general-purpose graph CLI. It does not assign ordinary directed-graph
semantics to a multi-tail hyperedge.

## Output Contract

Successful commands write JSON to stdout. stdout never contains prompts, progress
messages, or logs. Errors are written as structured JSON to stderr.

Mutation commands write the complete transformed graph. Read and query commands write
only their result. A failed mutation writes no partial graph. Output is compact by
default; `--pretty` uses two-space indentation. Successful JSON ends with one newline.

`--help`, command-level `--help`, and `--version` are the only plain-text stdout
exceptions. They do not read graph input.
