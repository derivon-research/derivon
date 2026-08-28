# Route Semantics

Given a start set `S` and target set `T`, a route is a selected hyperedge set `R` whose
closure from `S` contains every target in `T`.

The route cost is:

```text
cost(R) = sum(weight(h)) for h in R
```

A selected hyperedge is charged once even when several targets or branches reuse it.
This is minimum set cost over a B-hypergraph, not an ordinary shortest path.

## Budgeted Exact Search

Minimum set cost is NP-hard. Route search therefore defaults to a 200,000-node and
10,000-millisecond budget. Both values may be overridden, including with zero, but there
is no unlimited mode. A completed proof returns `provenOptimal: true`. If a budget is exhausted, the query still
returns the best executable derivation found, a certified lower bound, an upper bound,
and `provenOptimal: false`.

When optimality has not been proven, `cost` is the best known cost and must not be
interpreted as the exact minimum.

A target already present in the start set needs no hyperedge and contributes zero cost.
If any requested target is unreachable, the result reports `reachable: false` rather
than treating the graph as malformed.

## Determinism

Reachability, exact optimal cost, and certified bounds do not depend on point or
hyperedge array order. Equal-cost witnesses are not canonical across equivalent reordered
graphs. For identical input and node budget, a witness is deterministic when the wall
clock budget does not stop search first.

Set-valued IDs are ASCII-sorted. Executable order is not sorted. Blocking IDs and IDs
inside cycles are sorted, then cycles are sorted by their ID sequences. `millis` is not
reproducible. When wall time stops search first, best-known witness and search counters
may vary by machine, but every returned bound and witness remains valid.
