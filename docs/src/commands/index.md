# Commands

Commands are grouped by graph resource and operation:

```text
validate
point list|get|add|remove|rename|data get|data set|data remove
hyperedge list|get|add|remove|rename|set tails|set head|set weight|data get|data set|data remove
query closure|route|diagnose
subgraph route|reachable|induced
apply
```

Global options are:

```text
--input <FILE>
--pretty
--max-input-bytes <N>
--max-value-bytes <N>
-h, --help
-v, --version
```

Graph input defaults to stdin. When `--input` is present, stdin is not inspected. Byte
limits are positive decimal values; zero is not an unlimited sentinel. Business options
have stable long names only. The v0.1.0 spelling `-V` remains accepted as a hidden
compatibility alias; new scripts use `-v` or `--version`.

Point and hyperedge mutations emit a complete graph. Reads emit bare JSON entities or
values without an envelope. `validate` passes a valid graph through unchanged. Query
commands do not mutate input. Successful JSON is compact unless `--pretty` is present.

`--data` and `--value` accept JSON text and are mutually exclusive with `--data-file` and
`--value-file`. Plain strings therefore require JSON quotes. Pointers are empty or begin
with `/`.
