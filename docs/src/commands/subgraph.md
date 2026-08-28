# Subgraph Commands

Every subgraph command returns a result envelope:

```json
{
  "graph": { "points": [], "hyperedges": [] },
  "selection": {}
}
```

Extract `.graph` explicitly before passing it to another graph command:

```bash
derivon subgraph reachable --start A < graph.json \
  | jq '.graph' \
  | derivon validate
```

## Induced

```text
derivon subgraph induced [--point <POINT_ID>]...
```

No point flags produce the empty graph.

The graph contains exactly the selected points and every hyperedge whose head and all
tails belong to that point set. Isolated selected points and eligible empty-tail edges
are retained.

## Reachable

```text
derivon subgraph reachable [--start <POINT_ID>]...
```

The command computes closure under the complete input graph, then returns the induced
subgraph of that closure. It retains all executable alternatives, not only one route.

## Route

```text
derivon subgraph route [--start <POINT_ID>]... --target <POINT_ID>
  [--target <POINT_ID>]... [--max-nodes <N>] [--max-millis <N>]
```

The graph contains the best-known route hyperedges, every incident point, and requested
start or target points not incident to a selected edge. `selection` is the complete route
result, including cost bounds and `provenOptimal`, so an approximate result is never
hidden. If the target set is unreachable, `graph` is null and `selection` is the
unreachable route result.

All projections preserve schema presence, original array order, IDs, weights, and opaque
data. Point-set flags are repeatable and duplicates are errors.
