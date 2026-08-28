# Point Commands

```text
derivon point list
derivon point get <ID>
derivon point add <ID> [--data <JSON> | --data-file <FILE>]
derivon point remove <ID> [--cascade] [--ignore-missing]
derivon point rename <ID> <NEW_ID>
derivon point data get <ID> [POINTER]
derivon point data set <ID> [POINTER] (--value <JSON> | --value-file <FILE>)
derivon point data remove <ID> [POINTER] [--ignore-missing]
```

Mutations emit the complete transformed graph. New points are appended. If no payload is
supplied for a new point, its data defaults to `{}`; explicit `--data null` preserves
null.

Point rename updates all hyperedge tail and head references atomically while retaining
array position. Renaming to an existing point or hyperedge ID, including itself, fails.

Removing a referenced point fails by default. `point remove A --cascade` removes the
point and every hyperedge whose tails or head reference it. All CRUD operations are
strict. Unknown IDs and collisions fail. `--ignore-missing` is available only for remove
operations. Removing an unknown point with both `--cascade` and `--ignore-missing` is a
successful no-op. Cascading an existing unreferenced point succeeds normally.

## Data

Data commands use RFC 6901 JSON Pointer. Omitting the pointer addresses the complete data
value. Missing intermediate parents fail; set may create its final object member and may
append to an array with `-`. JSON null is a value, not a removal request. Large values may
come from `--value-file`.

Removing the root omits the data field. Removing an array element shifts later elements
toward zero; `/-` is invalid for remove. Missing paths fail unless `--ignore-missing` is
present.

Absent and null data are valid. A complete data set may replace either with any JSON
value. A subpath get, set, or remove under absent or null data returns
`pointer_type_mismatch` and never promotes the value or panics. `--ignore-missing` does
not suppress type mismatches. Setting an existing value to an equal JSON value succeeds;
removing a missing path with `--ignore-missing` is a successful no-op.
