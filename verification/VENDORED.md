# Vendored oracle

These files are **copies**, taken from the reference implementation:

    source:  git@github.com:derivon-research/paper.git
    path:    verification/
    commit:  9a2096e0e9132ec1ff595001350004c2661c5d35

## Why they are copied rather than referenced

CI has to run the Rust implementation and the Python oracle against each other, and a
single checkout is far simpler than coordinating two repositories for a check that must
run on every commit.

Copying does not weaken the check. The oracle's value is that it is a **separately
written implementation** — different author pass, different language, its own
reachability code — and that property travels with the file. What copying risks instead
is silent divergence, which is why the source commit is pinned above.

## Rules

- **Do not edit these files here.** Fix upstream, then re-vendor and update the commit
  above in the same change.
- If upstream ever changes, re-vendoring is a deliberate act with its own commit, not a
  merge conflict to resolve.
- Everything else under `paper/verification/` verifies the *Python* implementation and
  the propositions in the paper. It is deliberately not copied here; only the oracle
  needed to certify this crate is.
