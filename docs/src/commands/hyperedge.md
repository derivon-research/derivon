# Hyperedge Commands

```text
derivon hyperedge list
derivon hyperedge get <ID>
derivon hyperedge add <ID> [--tail <POINT_ID>]... --head <POINT_ID> --weight <WEIGHT>
  [--data <JSON> | --data-file <FILE>]
derivon hyperedge remove <ID> [--ignore-missing]
derivon hyperedge rename <ID> <NEW_ID>
derivon hyperedge set tails <ID> [--tail <POINT_ID>]...
derivon hyperedge set head <ID> <POINT_ID>
derivon hyperedge set weight <ID> <WEIGHT>
derivon hyperedge data get <ID> [POINTER]
derivon hyperedge data set <ID> [POINTER] (--value <JSON> | --value-file <FILE>)
derivon hyperedge data remove <ID> [POINTER] [--ignore-missing]
```

Mutations emit the complete transformed graph. New hyperedges are appended; absent data
defaults to `{}`, while explicit null is preserved. CRUD is strict: unknown IDs and
global ID collisions fail, including rename to the same ID. `--ignore-missing` is
available only for remove; removing an unknown edge with it is a successful no-op.

Every referenced point must exist. Duplicate tails are rejected. Empty tails are valid;
`set tails` without `--tail` sets an empty tail. Parallel hyperedges are valid when their
IDs differ.

Weights are exact non-negative decimals no greater than `900719925474099.1` and must be
multiples of one tenth. Invalid values are rejected rather than rounded. Scientific
notation is accepted by mathematical value and negative zero is emitted as zero.

## Data

Data commands follow the same JSON Pointer rules as point data commands. Omitting the
pointer addresses the entire payload. Missing intermediate parents fail, a final object
member may be created, and `-` appends to an array.

Root removal omits data. Array removal shifts later elements; `/-` is invalid for remove.
Missing paths fail unless `--ignore-missing` is present. A subpath operation under absent
or null data returns `pointer_type_mismatch`; the CLI neither promotes the value nor
panics, and `--ignore-missing` does not suppress type mismatches. Same-value set
operations succeed. Removing a missing data path with `--ignore-missing` is a successful
no-op.
