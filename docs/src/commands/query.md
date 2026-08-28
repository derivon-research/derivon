# Query Commands

Query commands inspect a graph without mutating it. Set arguments use repeatable singular
flags; duplicate IDs are errors:

```bash
derivon query closure --start A --start B
derivon query route --start A --target X --target Y
derivon query diagnose --start A --target X --target Y
```

Omitting `--start` selects the empty start set. A route target set must contain at least
one point. Set-valued output uses deterministic ID ordering.

Closure returns a self-describing object:

```json
{"startPointIds":["A"],"pointIds":["A","B","X"]}
```

Route and diagnosis results use `reachable` as a discriminator. A reachable route
contains point and hyperedge IDs, executable order, cost bounds, `provenOptimal`, and
search metrics. An unreachable route contains `targetDiagnoses` and omits inapplicable
solution fields. Diagnosis returns one item for every requested target; reachable targets
have empty `blockingPointIds` and `cycles`.

Unreachability is a successful query result rather than a malformed-command error.
The complete route grammar is:

```text
derivon query route [--start <POINT_ID>]... --target <POINT_ID>
  [--target <POINT_ID>]... [--max-nodes <N>] [--max-millis <N>]
```

Route search defaults to 200,000 branch nodes and 10,000 milliseconds. Both budgets may
be overridden, including with zero to request bounds and an initial witness without
branch expansion. There is no unlimited mode or product-level hard maximum.
