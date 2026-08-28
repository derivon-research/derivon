# Unix Pipeline

The default graph input is stdin:

```bash
derivon point add B < graph.json > graph-with-b.json
```

`--input` selects a file instead:

```bash
derivon --input graph.json point get A
```

When `--input` is present, the CLI reads that file and does not inspect stdin. Without
`--input`, it reads exactly one graph from stdin. The CLI has no in-place edit mode and
does not own an output-file option. Use shell redirection when a result should be stored.

Commands compose through standard pipes because every mutation emits a complete graph.
Use `--pretty` only at the end of a pipeline when formatted output is useful:

```bash
derivon point add B < graph.json \
  | derivon hyperedge add h-ab --tail A --head B --weight 1.5 \
  > updated-graph.json
```

An authoring document contains graph-independent fields which the CLI must not receive.
Use an external JSON processor to extract or replace its graph:

```bash
jq '.graph' workspace.json | derivon query closure --start A
```

For several changes that must succeed or fail together, use atomic `apply` rather than a
multi-process pipeline:

```bash
derivon apply --operations changes.json < graph.json > updated-graph.json
```
