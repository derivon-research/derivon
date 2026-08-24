//! `derivond` — the protocol adapter.
//!
//! This binary is deliberately thin: it owns transport and message framing, and nothing
//! else. All data structures and algorithms live in `derivon-core`, which has no IO
//! dependencies at all. The dependency runs one way and Cargo enforces it.
//!
//! Not implemented yet. The protocol is specified in the handover notes: JSONL over
//! stdio for the offline case, the same messages over a socket for the hosted one, with
//! `graph.load` / `graph.apply` / `query.route` / `query.diagnose` as the surface.
//!
//! Note that the verification harness is *not* here — see
//! `derivon-core/examples/selftest.rs`. Correctness tooling belongs with the code it
//! certifies, not with the transport.

fn main() {
    eprintln!("derivond: protocol adapter not implemented yet");
    std::process::exit(2);
}
