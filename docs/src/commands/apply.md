# Atomic Apply

```text
derivon apply --operations <FILE> [--max-operations-bytes <N>]
```

The operations file cannot be stdin or `-`. It is an unversioned JSON array whose version
follows the CLI compatibility line (`0.1.x` before 1.0, major version after 1.0). Every
item is a typed mutation object.

| `op` | Required fields | Optional fields and defaults |
| --- | --- | --- |
| `point.add` | `id` | `data` = `{}` |
| `point.remove` | `id` | `cascade` = false, `ignoreMissing` = false |
| `point.rename` | `id`, `newId` | none |
| `point.data.set` | `id`, `value` | `pointer` = `""` |
| `point.data.remove` | `id` | `pointer` = `""`, `ignoreMissing` = false |
| `hyperedge.add` | `id`, `head`, `weight` | `tails` = `[]`, `data` = `{}` |
| `hyperedge.remove` | `id` | `ignoreMissing` = false |
| `hyperedge.rename` | `id`, `newId` | none |
| `hyperedge.set.tails` | `id`, `tails` | none |
| `hyperedge.set.head` | `id`, `head` | none |
| `hyperedge.set.weight` | `id`, `weight` | none |
| `hyperedge.data.set` | `id`, `value` | `pointer` = `""` |
| `hyperedge.data.remove` | `id` | `pointer` = `""`, `ignoreMissing` = false |

Explicit data null is preserved. Tails are JSON arrays. Boolean fields require JSON
booleans. File-based data/value forms do not exist inside operations. Missing, unknown,
or incorrectly typed fields fail the complete apply. Issue paths include the operation
array index, for example `/operations/3/head`.

Operations execute in array order and each intermediate graph must remain valid. Later
operations may refer to entities created or renamed earlier. Reads, queries, and subgraph
operations are rejected. An empty operation array succeeds and returns the input graph.
The same no-op and strictness rules as individual commands apply.

The process mutates one owned graph and needs no rollback copy because no intermediate
state is externally visible. The complete result is validated and serialized into memory
before stdout is touched. Argument, parse, validation, operation, and serialization
failures produce zero stdout bytes.

A final transport failure exits 74 but cannot retract bytes already accepted by the OS.
This unavoidable Unix IO case is outside the atomic operation guarantee. Input and
operations files remain read-only.
