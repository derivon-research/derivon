# Errors

stdout is reserved for successful JSON output. Errors are structured JSON on stderr. A
failed mutation emits no graph. Messages are diagnostic text and are not a stable machine
contract; scripts use `code`, issue codes, paths, and details.

```json
{
  "error": {
    "code": "invalid_graph",
    "message": "graph validation failed",
    "issues": [
      {
        "code": "unknown_point",
        "path": "/hyperedges/2/tails/0",
        "message": "point `A` does not exist"
      }
    ]
  }
}
```

## Exit And Top-Level Codes

| Exit | Stable top-level codes |
| ---: | --- |
| 0 | Success, including unreachable queries and exhausted route budgets |
| 64 | `invalid_arguments` |
| 65 | `invalid_json`, `duplicate_key`, `input_limit_exceeded`, `nesting_limit_exceeded`, `unsupported_schema`, `invalid_graph`, `invalid_operations`, `invalid_id`, `invalid_weight`, `unknown_point`, `unknown_hyperedge`, `id_conflict`, `point_referenced`, `invalid_pointer`, `pointer_not_found`, `pointer_type_mismatch` |
| 66 | `file_not_found`, `file_unreadable` |
| 70 | `internal` |
| 74 | `io` |

Full graph validation uses `invalid_graph` with issues. Apply structure or operation
failure uses `invalid_operations`; issue paths include operation indexes. A single CLI
mutation uses its specific business code. Optional `details` may carry fields such as
`id`, `source`, or `limit`.

## Issue Codes

Validation issues use this stable set:

```text
missing_field
unknown_field
invalid_type
invalid_id
duplicate_id
duplicate_tail
unknown_point
invalid_weight
duplicate_key
invalid_pointer
pointer_not_found
pointer_type_mismatch
point_referenced
```

Parser failures which prevent structural validation return without synthetic follow-up
issues. Otherwise issues are ordered by schema/root structure, points in array order,
hyperedges in array order, then cross-references. Point fields use `id`, `data` order;
hyperedge fields use `id`, `weight`, `tails`, `head`, `data`. Apply follows operation
array and documented field order. Multiple issues at one path sort by issue code.

An invalid parent value suppresses cascading child issues. Unreachability and route budget
exhaustion are not errors. A final stdout transport failure exits 74; pipe bytes already
accepted by the OS cannot be retracted.
