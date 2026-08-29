# Derivon

Derivon computes reachability and minimum-set-cost derivations over non-negative weighted
directed B-hypergraphs.

The `derivon` CLI is a stateless JSON processor for Agents, scripts, and Unix pipelines.
It accepts one graph that follows the CLI-owned `derivon.graph/v1` protocol:

```bash
derivon validate < graph.json
```

Example mutation and route query:

```bash
derivon point add B < graph.json \
  | derivon hyperedge add h-ab \
      --tail A --head B --weight 1.5 \
  > updated-graph.json

derivon query route --start A --target B < updated-graph.json
```

## Install

```bash
cargo install derivon-cli
brew install derivon-research/tap/derivon
curl -fsSL https://docs.derivon.net/cli/install.sh | sh
```

The installer uses Homebrew or Cargo on macOS and verified release archives on Linux.

## Documentation

The versioned, bilingual mdBook source is under [`docs/`](docs/src/index.md). English is
the normative contract; Chinese is maintained under [`docs/src/zh/`](docs/src/zh/index.md).

## Workspace

- `derivon-core`: IO-free mathematical graph and solver implementation.
- `derivon-cli`: graph protocol, CRUD, query, subgraph, and Unix transport.
- `derivond`: reserved protocol adapter.

## License

MIT. See [`LICENSE`](LICENSE).
