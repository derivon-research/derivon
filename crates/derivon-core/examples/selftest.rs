//! Emit instances and this implementation's answers as JSONL, for comparison against the
//! independently written Python oracle.
//!
//! This lives beside the crate it verifies rather than in `derivond`: it is a
//! *verification* artefact, not a protocol one, and a correctness harness belongs with
//! the code it certifies.
//!
//! # Why this exists
//!
//! `solve.rs` already checks branch and bound against an exhaustive search, but that
//! exhaustive search lives in the same crate, was written by the same author in the same
//! session, and — decisively — calls the very same `closure_restricted` the solver does.
//! Any misunderstanding baked into that shared function is invisible to it: both sides
//! agree, and every test stays green.
//!
//! The Python oracle in `derivon-research/paper` is independent in exactly the way that
//! matters: separate author pass, separate language, separate reachability code, and
//! already cross-validated over 1325 queries. Comparing across that boundary is the only
//! check that can catch a shared misreading of the model itself.
//!
//! # Contract
//!
//! This binary emits the *instance* alongside its own answer. The Python side recomputes
//! the answer from the instance using its own code — it never trusts a number from here.
//! In particular it re-verifies the returned derivation with its own closure, which is
//! what puts `closure_restricted` under independent scrutiny.
//!
//! ```text
//! cargo run -p derivon-core --example selftest -- --count 400 --seed 7 > /tmp/rust.jsonl
//! python3 verification/cross_check.py /tmp/rust.jsonl
//! ```

use derivon_core::closure::PointSet;
use derivon_core::graph::{Cost, Graph, PointId};
use derivon_core::solve::{Budget, solve};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count = flag(&args, "--count").unwrap_or(400);
    let seed = flag(&args, "--seed").unwrap_or(7);
    selftest(count, seed);
}

fn flag(args: &[String], name: &str) -> Option<u64> {
    let index = args.iter().position(|a| a == name)?;
    args.get(index + 1)?.parse().ok()
}

/// Deterministic so both sides can reproduce a failing case from `--seed` alone.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0 >> 32
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

struct Instance {
    graph: Graph,
    point_names: Vec<String>,
    start_names: Vec<String>,
    start: PointSet,
    target: PointId,
    target_name: String,
}

/// Instances are kept small because the Python side settles them by `2^|H|` enumeration.
///
/// The generator deliberately produces empty tails, empty start sets, and cycles: those
/// are the three cases where the reference implementation has actually been wrong before,
/// so a batch that omits them tests very little.
fn generate(rng: &mut Lcg) -> Instance {
    let point_count = 4 + rng.below(4) as usize; // 4..=7
    let edge_count = 3 + rng.below(7) as usize; // 3..=9

    let mut graph = Graph::new();
    let point_names: Vec<String> = (0..point_count).map(|i| format!("p{i}")).collect();
    let ids: Vec<PointId> = point_names
        .iter()
        .map(|n| graph.add_point(n.clone(), ()).expect("fresh name"))
        .collect();

    for i in 0..edge_count {
        // Tail size 0 exercises unconditional entry points.
        let tail_len = rng.below(3) as usize;
        let mut tail: Vec<PointId> = Vec::new();
        for _ in 0..tail_len {
            let candidate = ids[rng.below(point_count as u64) as usize];
            if !tail.contains(&candidate) {
                tail.push(candidate);
            }
        }
        // Head may point back into earlier premises, so cycles arise naturally.
        let head = loop {
            let candidate = ids[rng.below(point_count as u64) as usize];
            if !tail.contains(&candidate) {
                break candidate;
            }
        };
        graph
            .add_hyperedge(
                format!("e{i}"),
                tail,
                head,
                Cost::from_units(rng.below(6)), // zero weights are legal and worth covering
                (),
            )
            .expect("fresh name, known points, finite weight");
    }

    // An empty start set is generated roughly a third of the time; without it the
    // empty-tail seeding path is never reached.
    let start_len = rng.below(3) as usize;
    let mut start_ids: Vec<PointId> = Vec::new();
    let mut start_names: Vec<String> = Vec::new();
    for _ in 0..start_len {
        let index = rng.below(point_count as u64) as usize;
        if !start_ids.contains(&ids[index]) {
            start_ids.push(ids[index]);
            start_names.push(point_names[index].clone());
        }
    }
    let start = PointSet::from_ids(&graph, start_ids).expect("ids came from this graph");

    let target_index = rng.below(point_count as u64) as usize;

    Instance {
        target: ids[target_index],
        target_name: point_names[target_index].clone(),
        point_names,
        start_names,
        start,
        graph,
    }
}

fn selftest(count: u64, seed: u64) {
    let mut rng = Lcg(seed);
    for _ in 0..count {
        let instance = generate(&mut rng);
        let solution = solve(
            &instance.graph,
            &instance.start,
            instance.target,
            &Budget::default(),
        )
        .expect("well-formed query");

        println!("{}", render(&instance, &solution));
    }

    // Trailer. A panic anywhere above truncates the dump, and a truncated dump would
    // otherwise read as "few instances, all passing" on the Python side. The comparator
    // requires this line and the count it carries, so an aborted run fails loudly
    // instead of quietly shrinking the batch.
    println!("{{\"_summary\":{{\"count\":{count},\"seed\":{seed}}}}}");
}

/// Hand-rolled because the schema is fixed and all names are `p0`/`e0` shaped, so no
/// escaping is required. When the real protocol lands it will bring `serde`, and this
/// should switch over rather than grow.
fn render(instance: &Instance, solution: &derivon_core::solve::Solution) -> String {
    let quoted = |items: &[String]| {
        items
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(",")
    };

    let edges = instance
        .graph
        .hyperedges()
        .map(|edge| {
            let tail: Vec<String> = edge
                .tail()
                .iter()
                .map(|id| {
                    instance
                        .graph
                        .point(*id)
                        .expect("in graph")
                        .name()
                        .to_owned()
                })
                .collect();
            format!(
                "{{\"name\":\"{}\",\"tail\":[{}],\"head\":\"{}\",\"weight\":{}}}",
                edge.name(),
                quoted(&tail),
                instance.graph.point(edge.head()).expect("in graph").name(),
                edge.weight().units().expect("edge weights are finite"),
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let derivation: Vec<String> = solution
        .derivation
        .iter()
        .map(|id| {
            instance
                .graph
                .hyperedge(*id)
                .expect("in graph")
                .name()
                .to_owned()
        })
        .collect();

    // `null` rather than a sentinel: the target being unreachable is a normal outcome,
    // and it must survive the round trip as such.
    let number = |cost: Cost| match cost.units() {
        Some(units) => units.to_string(),
        None => "null".to_string(),
    };

    format!(
        "{{\"points\":[{}],\"edges\":[{}],\"start\":[{}],\"target\":\"{}\",\
         \"cost\":{},\"lower\":{},\"upper\":{},\"proven_optimal\":{},\"derivation\":[{}]}}",
        quoted(&instance.point_names),
        edges,
        quoted(&instance.start_names),
        instance.target_name,
        number(solution.cost),
        number(solution.lower),
        number(solution.upper),
        solution.proven_optimal,
        quoted(&derivation),
    )
}
