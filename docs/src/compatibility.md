# Compatibility

The first CLI release is package version `0.1.0` with Git tag
`derivon-cli-v0.1.0`. Package-prefixed tags avoid collisions with existing workspace
tags.

## CLI

Before 1.0, patch releases preserve the contract within one minor line. A new `0.x`
minor may make documented incompatible changes. After 1.0, incompatible command,
successful-output, error-code, or apply-operation changes require a new CLI major.

Minor-compatible releases may add commands, optional flags, and error codes, so consumers
must handle unknown codes. Patch releases fix defects without changing the mathematical
result or JSON shape for previously valid input. Automation should pin a compatible CLI
line, such as `0.1.x` before 1.0 or a major version after 1.0. Apply arrays follow that
CLI compatibility line.

## Graph Protocol

`derivon.graph/v1` is frozen once released. Any incompatible structure change requires a
new graph schema, even while the CLI is below 1.0. Unversioned graph input always means
graph/v1; it never means the newest schema.

New CLI lines continue to read older graph schemas unless an incompatible release
explicitly deprecates support in advance. The CLI never performs an implicit graph
upgrade or downgrade.

## Documentation

GitHub Pages publishes main under `https://docs.derivon.net/cli/dev/`. A tag such as
`derivon-cli-v0.1.0` publishes the latest `0.1.x` manual under `/cli/v0.1/`; after 1.0,
releases publish major paths such as `/cli/v1/`. `/cli/` points to the latest released
manual, while the site root currently points to `/cli/`. English is normative and each
version's Chinese translation is under its version path at `/zh/`.
