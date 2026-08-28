# Graph Format

The CLI directly accepts the `graph` fragment used by the Derivon mind-map authoring
format:

```json
{
  "points": [
    {
      "id": "A",
      "data": { "label": "A" }
    },
    {
      "id": "B",
      "data": { "label": "B" }
    }
  ],
  "hyperedges": [
    {
      "id": "h-ab",
      "weight": 1.5,
      "tails": ["A"],
      "head": "B",
      "data": { "source": "example" }
    }
  ]
}
```

The optional top-level schema is `"schema": "derivon.graph/v1"`. Input without a
`schema` means exactly `derivon.graph/v1`, not whichever version is newest. Unknown
versions are rejected.

Mutation and subgraph commands preserve schema presence. Unversioned input remains
unversioned; versioned input retains the same schema. The CLI never performs an implicit
protocol upgrade or downgrade.

## Points

A point has a globally unique string `id` and optional opaque `data`.

## Hyperedges

A hyperedge has:

- a globally unique string `id`;
- a `tails` array containing unique point IDs;
- one `head` point ID;
- an exact finite non-negative numeric `weight` in tenths units no greater than
  `900719925474099.1`; and
- optional opaque `data`.

Point and hyperedge IDs share one namespace and are case-sensitive machine identifiers.
Agents, scripts, and the mind-map backend own their allocation. Human-facing names and
labels belong in `data`, never in the structural ID.

An ID is 1 to 128 ASCII bytes and matches:

```text
^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$
```

Tail order has no mathematical meaning. The CLI rejects duplicate tails but preserves
the supplied order when transforming a graph.

Weights are parsed as exact decimal values without `f64` conversion or rounding. The
maximum is `900719925474099.1`, whose tenths units equal JavaScript's safe-integer maximum
`2^53 - 1`. Scientific notation is accepted when its mathematical value is valid;
negative zero is accepted and emitted as zero.

## Opaque Data

`data` may contain any JSON value. It is never inspected by validation or derivation
logic. A command which does not address `data` preserves its JSON value. JSON numbers in
data retain arbitrary precision and are never converted through binary floating point.
Duplicate object keys are rejected everywhere, including inside data.

Preservation is semantic rather than textual. Whitespace, object-key order, numeric
spelling such as `1.0`, and Unicode escape spelling may change after serialization.

Unknown structural fields outside `data` are rejected. Every command validates the
complete input graph before operating. Mutations validate the complete result again.
Validation reports deterministic issues but does not repair, sort, or normalize input.

## Valid Edge Cases

Empty graphs, isolated points, empty-tail hyperedges, self-dependencies, cycles,
parallel hyperedges, zero-weight edges, and zero-weight cycles are valid. A hyperedge may
refer to a point appearing later in the points array because array position is not
declaration order.

Identity conflicts, duplicate IDs or tails, unknown point references, missing required
fields, unknown structural fields or schema, invalid weights, and duplicate JSON keys are
invalid.

## Ordering

Point and hyperedge arrays retain input order. Add operations append; rename operations
modify an item in place. Ordinary query result sets use deterministic ID ordering.
Executable derivation order is preserved as computed. Output object keys are ordered
deterministically; compact output has no unnecessary whitespace and pretty output uses
two spaces.

## Resource Limits

Graph input is limited to 256 MiB by default. Operation and value files are limited to
64 MiB. Explicit byte-limit flags may raise these limits. JSON nesting depth is fixed at
128 and cannot be disabled. There is no separate point or hyperedge count limit.
