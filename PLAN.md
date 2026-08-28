# Derivon CLI Plan

This file records design decisions, unresolved questions, and implementation work for
the Derivon CLI. Stable user-facing behavior belongs in `docs/`, not here.

## Status

The design tree is closed and the user confirmed the shared contract. The initial
`derivon-cli 0.1.0` implementation, contract tests, documentation, installer, and release
workflows are complete in the working tree.

External release actions remain operational prerequisites rather than code tasks:

- configure `CARGO_REGISTRY_TOKEN` and publish `derivon-core 0.2.0` before the CLI;
- create and configure `derivon-research/homebrew-tap`;
- point the intended `derivon.net` hosting at the Pages artifact or otherwise publish
  `site/cli/install.sh`; and
- enable GitHub Pages and required provenance permissions in repository settings.

## Confirmed Decisions

### Product boundary

- The CLI is a stateless Unix filter for Agents and scripts.
- Each invocation reads one complete graph, performs one operation, and writes either
  the transformed graph or a query result.
- The CLI does not own persistent state, edit files in place, or act as a daemon.
- A future `derivond` adapter may reuse the same operation layer for JSONL or socket
  transports, but it is not the CLI contract.

### Mathematical model

- The only supported model is Derivon's non-negative weighted directed B-hypergraph.
- A hyperedge has a set of zero or more tails, exactly one head, and a weight.
- Parallel hyperedges and cycles are valid.
- Point and hyperedge payloads are opaque to the CLI and never affect derivation.

### Graph protocol

- The current mind-map `graph` fragment remains directly consumable.
- The graph may carry `"schema": "derivon.graph/v1"`. A missing schema means exactly
  `derivon.graph/v1`; it never means "latest".
- Mutations preserve schema presence: unversioned input remains unversioned and
  versioned input retains its version. Unknown versions are rejected.
- `data` is optional opaque JSON. Existing values are preserved with JSON-semantic,
  rather than source-text, fidelity. Arbitrary-precision numbers are retained and
  duplicate object keys are rejected at every nesting level.
- Subpath operations on absent or null `data` return typed errors rather than promoting
  the value or panicking; replacing the complete `data` value remains valid.
- Unknown structural fields outside `data` are rejected.
- Version 1 weights are exact finite non-negative decimals whose tenths units do not
  exceed `2^53 - 1`, matching the mind-map safe-integer rule. Scientific notation and
  negative zero are accepted by mathematical value; no `f64` conversion or rounding is
  allowed.
- Every command fully validates its input graph before operating. Mutations validate the
  result again; validation collects deterministic issues without normalizing the graph.

### Command and stream contract

- Commands use resource-first nesting, including `point data get|set` and
  `hyperedge data get|set`.
- stdin is the default graph source. When `--input` is present, the CLI reads that file
  and does not inspect stdin.
- Mutation commands emit the complete transformed graph on stdout.
- Read and query commands emit only their JSON result on stdout.
- stdout contains compact JSON only by default. `--pretty` enables two-space formatted
  JSON. Successful JSON ends with one newline and object output is deterministic.
- `validate` passes a valid graph through, and entity/data reads emit bare JSON values.
- Diagnostics are structured JSON on stderr and graph paths use JSON Pointer.
- A failed mutation emits no partial graph.
- There is no in-place mode or CLI-owned output file.
- Help and version meta-operations are the only plain-text stdout exceptions and do not
  read graph input.

### Errors and no-op behavior

- Exit codes are 0, 64, 65, 66, 70, and 74 with the stable top-level codes documented
  in the user reference. Messages are not a machine contract.
- Graph and operations validation use stable issue codes and deterministic traversal
  order. Invalid parent structures suppress synthetic child errors.
- Same-value set operations and empty apply succeed. Same-ID rename and duplicate add
  fail. `--ignore-missing` applies only to absence during remove and never suppresses an
  invalid pointer or type mismatch.

### Identity and ordering

- Point and hyperedge IDs share one global namespace and compare exactly with case
  sensitivity.
- IDs are machine identifiers managed by Agents, scripts, and the mind-map backend.
  Display names belong in `data`.
- IDs match `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`. Set-valued CLI arguments use
  repeatable singular flags such as `--start`, `--target`, and `--tail`; duplicates fail.
- Graph array order is stable: add appends, rename edits in place, and tails preserve
  supplied order while rejecting duplicates.
- Ordinary query sets use deterministic ASCII ID ordering; executable derivation order
  is never sorted after computation. Equivalent graph reorderings need not choose the
  same equal-cost witness, and wall-time-limited search is machine-dependent.

### Route query

- A route minimizes selected-hyperedge set cost. A reused hyperedge is charged once.
- Start sets may be empty; target sets may not be empty.
- Exact search is budgeted and anytime. Budget exhaustion returns the best executable
  witness and certified lower/upper bounds with `provenOptimal: false`.
- Unreachability and budget exhaustion are successful query outcomes, not protocol
  errors.
- Route results are discriminated by `reachable`: reachable results contain a solution
  and search metrics; unreachable results contain diagnoses and omit inapplicable fields.
- The CLI defaults to 200,000 branch nodes and 10,000 milliseconds, permits zero and
  explicit overrides, and provides no unlimited mode or product-level maximum.

### Atomic apply

- `apply --operations FILE` consumes a JSON array of typed mutation objects. Operations
  input never shares stdin with the graph.
- Operation names and fields follow the complete user reference. They cover every point
  and hyperedge mutation, use strict JSON types/defaults, and reject unknown fields and
  query operations.
- Operations execute in order, each intermediate graph stays valid, and later operations
  may reference entities created or renamed earlier in the batch.
- Any argument, parse, validation, operation, or serialization failure produces no
  stdout bytes. The process-owned graph needs no rollback copy.
- The complete successful graph is serialized to memory before stdout is touched. A
  later transport failure exits 74 but cannot retract bytes already accepted by the OS.

### Subgraphs

- All subgraph commands return `{ "graph": ..., "selection": ... }`; callers use jq to
  extract `.graph` for another graph command.
- `induced` retains selected points and every hyperedge whose head and all tails are in
  that point set.
- `reachable` computes closure from the start set and returns its induced subgraph.
- `route` retains the best-known route edges, every incident point, and isolated requested
  start/target points. `selection` is the complete route result, preserving optimality
  status. Unreachable route results use `graph: null`.
- All projections preserve schema presence, input order, IDs, weights, and payloads.

### CLI grammar

- Global options are `--input`, `--pretty`, `--max-input-bytes`, `--max-value-bytes`,
  `--help`, and `--version`; stable business options have long names only.
- Graph input uses stdin only when `--input` is absent. Operations input is a required
  file. Data/value literals are JSON and are mutually exclusive with their file forms.
- Empty starts, tails, and induced point sets are valid; route targets are non-empty.
- Pointers are empty or begin with `/`. New payloads default to `{}` unless explicitly
  supplied, including explicit null.

### Graph edge cases

- Empty graphs, isolated points, empty tails, self-dependencies, cycles, parallel
  hyperedges, forward references by array position, zero weights, and zero-weight cycles
  are valid.
- Identity conflicts, duplicate tails/keys, unknown point references, unknown schema or
  structural fields, missing fields, and invalid weights are rejected.

### Resource limits

- Graph input defaults to 256 MiB; operations and value files default to 64 MiB.
- JSON nesting depth is fixed at 128. Byte limits may be raised through explicit flags;
  depth cannot be disabled.
- There is no separate point, hyperedge, or operation count limit.

### Distribution

- The first CLI release is `derivon-cli 0.1.0`, tagged
  `derivon-cli-v0.1.0` to avoid existing workspace tags.
- The package is `derivon-cli`, its binary is `derivon`, and it lives in
  `crates/derivon-cli`.
- `cargo install derivon-cli` and `brew install derivon-research/tap/derivon` are primary
  installation paths. The Homebrew tap uses a formula, not a cask.
- `https://docs.derivon.net/cli/install.sh` delegates to Homebrew or Cargo on macOS. It does
  not download an unsigned macOS binary or bypass Gatekeeper.
- Linux aarch64 and x86_64 receive direct release archives with SHA-256 checksums and
  provenance. Windows and standalone macOS archives are not version 0.1 commitments.
- No Apple Developer membership, Developer ID, signing, or notarization is required for
  this version 0.1 distribution model.
- The noninteractive installer supports exact version, install directory, and install
  method variables; verifies Linux checksums and smoke tests before atomic replacement;
  and never uses sudo, changes shell startup files, or installs package managers.
- CLI versions follow SemVer. `0.1.x` is patch-compatible while a new `0.x` minor may
  break with release notes. Graph/v1 is frozen, unversioned input always means v1, and
  incompatible graph structure still requires a new schema.
- The repository and all packages use the MIT license.

### Runtime purity

- Business commands are offline and stateless: no network, telemetry, update checks,
  implicit config, cache, lock, workspace discovery, or locale-dependent output.
- Only stdin and explicit files are read. Machine fields, help, and diagnostics are
  English; bilingual content exists only in the user manual.

### Documentation

- User documentation is a bilingual mdBook under `docs/` and is published with GitHub
  Pages. English is the primary and normative language; Chinese is a translation.
- Main updates `/cli/dev/`; `derivon-cli-v*` tags publish pre-1.0 minor paths such as
  `/cli/v0.1/` and post-1.0 major paths such as `/cli/v1/`. `/cli/` points to the latest
  release manual, the site root points to `/cli/`, and main never overwrites released docs.
- `docs/` contains stable behavior only. Design rationale and implementation progress
  remain in this file.

## Confirmed Command Surface

```text
validate
point list|get|add|remove|rename|data get|data set|data remove
hyperedge list|get|add|remove|rename|set tails|set head|set weight|data get|data set|data remove
query closure|route|diagnose
subgraph route|reachable|induced
apply
```

Removing a referenced point rejects by default; explicit `--cascade` removes the point
and every referring hyperedge. Point rename atomically rewrites all references. CRUD is
strict by default, with `--ignore-missing` available only for remove operations. Same-ID
rename and duplicate add fail; idempotent set and empty apply succeed. Ignored missing
removals are successful no-ops. `apply` executes a typed, ordered mutation array
atomically.

`data remove` follows JSON Patch removal behavior while using JSON Pointer for location.
Removing the root omits the `data` field; array removal shifts later elements; `/-` is
invalid for remove. Missing paths fail unless `--ignore-missing` is explicit.

## Open Design Tree

No design branches remain open. Any later contract change must update the English
normative manual, Chinese translation, tests, and this plan together.

## Implementation Outline

This outline is provisional until the design tree closes.

1. Add `crates/derivon-cli` as a library plus thin `derivon` binary without adding IO to
   `derivon-core`; keep `derivond` and the Tauri adapter unchanged, and defer a shared
   protocol crate until real reuse requires extraction.
2. Implement strict graph parsing with duplicate-key detection, arbitrary-precision data
   numbers, exact decimal weight conversion, full validation, and stable serialization.
3. Implement point and hyperedge read/write operations with transactional validation.
   For `apply`, mutate one process-owned graph, emit nothing on operation failure, run a
   final full validation, serialize the complete result into memory, then write stdout.
   No rollback copy is needed because the CLI owns no externally visible mutable state.
4. Adapt closure, route, executable-order, and diagnosis APIs from `derivon-core`.
   Sort set-valued boundary output by ASCII ID while preserving core execution order;
   document that equal-cost witnesses are not canonical across reordered input.
5. Implement induced, reachable, and route projections with common result envelopes.
6. Add structured diagnostics, exit codes, CLI parsing, and stdin/file transport.
7. Add contract fixtures, golden JSON tests, malformed-input tests, and cross-command
   pipeline tests. Use the generated 5,000-point line graph only as a parser/serializer
   and linear-operation smoke fixture; do not infer solver performance from it or pursue
   solver optimization in this scope.
8. Add crates.io, Homebrew tap, Linux archive, checksum, provenance, and noninteractive
   installer release automation.
9. Build versioned mdBook outputs under `/cli/dev/`, pre-1.0 minor paths, and post-1.0
   major paths, then publish them at `docs.derivon.net` with `/cli/` as the stable entry.
