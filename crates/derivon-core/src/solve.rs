//! Minimum-cost derivation: two polynomial bounds and the search between them.
//!
//! # What is being solved
//!
//! Given start set `S` and target set `T`, find the cheapest edge set `R` such that every
//! target in `T` is in the closure of `S` under `R`. Set cost is `sum_{h in R} w(h)`:
//! every edge is charged once no matter how many targets or branches reuse it.
//!
//! That "once" is the point of the model. A useful intermediate point compresses a
//! route precisely because one derivation can serve several later branches. It is also
//! what makes the problem hard: Set Cover reduces to minimum set cost, so no local
//! Dijkstra recurrence can solve it in general.
//!
//! # Strategy
//!
//! 1. Compute depth and tree bounds with generalized Dijkstra. If the lower bound equals
//!    the cost of the tree witness's unique edge support, the exact answer is pinned.
//! 2. Otherwise start deterministic branch-and-bound with the tree witness as incumbent
//!    and depth cost as an admissible lower bound.
//! 3. If the budget expires, return the best executable route and a certified interval.
//!    Never present tree cost as an exact set cost.
//!
//! # Why hardness is not disabling
//!
//! The upper bound is not merely a number. Its chosen edges form a concrete executable
//! derivation, so the algorithm is anytime: at every moment it has a route to recommend
//! and a proof of how low the unknown optimum may still be.
//!
//! # The interval is also a diagnostic
//!
//! A wide tree/depth interval signals substantial sharing, which is desirable graph
//! structure but also tends to make exact search harder. The same quantity is therefore
//! both a model-quality signal and an instance-hardness signal; instrumentation in
//! [`Solution`] exists so real hub-shaped workloads can be measured.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

use crate::closure::{PointSet, closure, closure_with_mask};
use crate::cost::{BoundRole, CostMeasure, DepthCost, TreeCost};
use crate::graph::{Cost, Graph, HyperedgeId, PointId};

struct KnuthResult {
    distances: Vec<Cost>,
    choices: Vec<Option<HyperedgeId>>,
}

/// Knuth's generalization of Dijkstra for a superior local measure.
///
/// The shape is Dijkstra-like: settle points in increasing cost order, but relax a
/// hyperedge only after all of its premises have settled. Correctness requires
/// [`CostMeasure::is_superior`], which this function asserts rather than assuming.
///
/// Rust's [`BinaryHeap`] is a max heap, so entries are wrapped in [`Reverse`]. The fixed
/// point [`Cost`] type supplies the total order that raw floating-point types cannot.
/// Empty-tail edges are seeded before the loop; otherwise an empty start set would leave
/// the queue empty and unconditional entry points would be missed.
pub fn bounds<P, E, M: CostMeasure>(
    graph: &Graph<P, E>,
    start: &PointSet,
    measure: &M,
) -> Vec<Cost> {
    assert!(
        measure.is_superior(),
        "Knuth's algorithm requires a superior measure"
    );
    knuth(graph, start, measure, None).distances
}

fn knuth<P, E, M: CostMeasure>(
    graph: &Graph<P, E>,
    start: &PointSet,
    measure: &M,
    prepaid: Option<&[bool]>,
) -> KnuthResult {
    assert_eq!(graph.point_count(), start.universe_len());
    if let Some(prepaid) = prepaid {
        assert_eq!(graph.edge_count(), prepaid.len());
    }

    let mut distances = vec![Cost::INFINITY; graph.point_count()];
    let mut choices = vec![None; graph.point_count()];
    let mut queue = BinaryHeap::new();
    for point in start.iter() {
        distances[point.index()] = Cost::ZERO;
        queue.push(Reverse((Cost::ZERO, point)));
    }

    // Empty-tail edges must seed the queue independently of the start set.
    for edge in graph.hyperedges().filter(|edge| edge.tail().is_empty()) {
        let step = if prepaid.is_some_and(|mask| mask[edge.id().index()]) {
            Cost::ZERO
        } else {
            edge.weight()
        };
        let candidate = measure.combine(step, &[]);
        if candidate < distances[edge.head().index()] {
            distances[edge.head().index()] = candidate;
            choices[edge.head().index()] = Some(edge.id());
            queue.push(Reverse((candidate, edge.head())));
        }
    }

    let mut settled = vec![false; graph.point_count()];
    let mut remaining: Vec<_> = graph.hyperedges().map(|edge| edge.tail().len()).collect();

    while let Some(Reverse((distance, point))) = queue.pop() {
        if settled[point.index()] || distance != distances[point.index()] {
            continue;
        }
        settled[point.index()] = true;

        for &edge_id in graph.outgoing_unchecked(point) {
            debug_assert!(remaining[edge_id.index()] > 0);
            remaining[edge_id.index()] -= 1;
            if remaining[edge_id.index()] != 0 {
                continue;
            }

            let edge = graph.edge_unchecked(edge_id);
            if settled[edge.head().index()] {
                continue;
            }
            let premise_costs: Vec<_> = edge
                .tail()
                .iter()
                .map(|point| distances[point.index()])
                .collect();
            let step = if prepaid.is_some_and(|mask| mask[edge_id.index()]) {
                Cost::ZERO
            } else {
                edge.weight()
            };
            let candidate = measure.combine(step, &premise_costs);
            if candidate < distances[edge.head().index()] {
                distances[edge.head().index()] = candidate;
                choices[edge.head().index()] = Some(edge_id);
                queue.push(Reverse((candidate, edge.head())));
            }
        }
    }

    KnuthResult { distances, choices }
}

/// The unique-edge support witnessing the tree-cost upper bound.
///
/// Starting at `target`, this walks the chosen edge for each point backward until it
/// reaches `start`. Repeated tree occurrences collapse to one edge ID in the returned
/// support. The result is a real derivation, not just a numerical upper bound.
pub fn tree_derivation<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    target: PointId,
) -> Option<Vec<HyperedgeId>> {
    graph.point(target)?;
    let targets = PointSet::from_ids(graph, [target]).ok()?;
    tree_derivation_many(graph, start, &targets)
}

/// The union of unique-edge supports witnessing tree-cost upper bounds for all targets.
///
/// The returned edge set derives every target. Shared prerequisites are collapsed to one
/// edge ID, so its set cost can be lower than the sum of the individual tree costs.
pub fn tree_derivation_many<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    targets: &PointSet,
) -> Option<Vec<HyperedgeId>> {
    if graph.point_count() != targets.universe_len() {
        return None;
    }
    let result = knuth(graph, start, &TreeCost, None);
    derivation_from_choices_many(graph, start, targets, &result)
}

fn derivation_from_choices_many<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    targets: &PointSet,
    result: &KnuthResult,
) -> Option<Vec<HyperedgeId>> {
    if targets
        .iter()
        .any(|target| !result.distances[target.index()].is_finite())
    {
        return None;
    }

    let mut have = vec![false; graph.point_count()];
    for point in start.iter() {
        have[point.index()] = true;
    }
    let mut selected = vec![false; graph.edge_count()];
    let mut stack: Vec<_> = targets.iter().collect();

    while let Some(point) = stack.pop() {
        if have[point.index()] {
            continue;
        }
        have[point.index()] = true;
        let edge_id = result.choices[point.index()]?;
        selected[edge_id.index()] = true;
        stack.extend_from_slice(graph.edge_unchecked(edge_id).tail());
    }

    Some(selected_to_ids(graph, &selected))
}

/// Limits for the exponential phase.
///
/// NP-hard search needs a mandatory exit. Polynomial bound computation is not charged
/// against `max_nodes`; elapsed wall time covers the complete solve call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Budget {
    pub max_nodes: u64,
    pub max_millis: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_nodes: 200_000,
            max_millis: 10_000,
        }
    }
}

/// Best known route and its certified interval.
///
/// When `proven_optimal` is false, `cost` and `derivation` describe the best executable
/// route found so far, and the true optimum remains in `[lower, upper]`. This state is a
/// successful anytime result, not an error.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Solution {
    /// Set cost of the best known derivation.
    pub cost: Cost,
    /// Unique selected edges; use `executable_order` when a firing order is needed.
    pub derivation: Vec<HyperedgeId>,
    /// Certified lower endpoint. Equals `cost` once optimality is proven.
    pub lower: Cost,
    /// Certified upper endpoint, tightened to the best known witness cost.
    pub upper: Cost,
    /// Whether the search exhausted the proof space before its budget.
    pub proven_optimal: bool,
    /// Branch-and-bound states expanded.
    pub nodes: u64,
    /// Branches rejected by cost or lower-bound pruning.
    pub pruned: u64,
    /// Total elapsed wall time for bounds and search.
    pub millis: u64,
}

/// Invalid solver configuration or query.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SolveError {
    UnknownTarget(PointId),
    PointSetSizeMismatch,
    TargetSetSizeMismatch,
    LowerMeasureNotSuperior,
    UpperMeasureNotSuperior,
    InvalidLowerMeasureRole(BoundRole),
    InvalidUpperMeasureRole(BoundRole),
    CostOverflow,
}

impl fmt::Display for SolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget(id) => write!(formatter, "target {id:?} does not belong to graph"),
            Self::PointSetSizeMismatch => {
                formatter.write_str("start set belongs to another graph size")
            }
            Self::TargetSetSizeMismatch => {
                formatter.write_str("target set belongs to another graph size")
            }
            Self::LowerMeasureNotSuperior => formatter.write_str("lower measure is not superior"),
            Self::UpperMeasureNotSuperior => formatter.write_str("upper measure is not superior"),
            Self::InvalidLowerMeasureRole(role) => {
                write!(
                    formatter,
                    "lower measure declares role {role:?}, expected Lower"
                )
            }
            Self::InvalidUpperMeasureRole(role) => {
                write!(
                    formatter,
                    "upper measure declares role {role:?}, expected Upper"
                )
            }
            Self::CostOverflow => formatter.write_str(
                "target set is reachable but its cost exceeds the fixed-point representation",
            ),
        }
    }
}

impl Error for SolveError {}

/// Convenience entry point using the proven [`DepthCost`] and [`TreeCost`] bounds.
pub fn solve<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    target: PointId,
    budget: &Budget,
) -> Result<Solution, SolveError> {
    min_set_cost(graph, start, target, &DepthCost, &TreeCost, budget)
}

/// Convenience entry point for a target set using the proven bounds.
pub fn solve_many<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    targets: &PointSet,
    budget: &Budget,
) -> Result<Solution, SolveError> {
    min_set_cost_many(graph, start, targets, &DepthCost, &TreeCost, budget)
}

/// Minimum set cost by deterministic, budgeted branch and bound.
///
/// Search keeps a set of goals still needed and a set of edges already selected. It
/// chooses the most constrained remaining goal, branches over incoming hyperedges, and
/// replaces that goal with the selected edge's unmet premises. Closure under selected
/// edges is recomputed at each state so shared prerequisites are charged only once.
///
/// # Sound pruning
///
/// Only a measure declaring [`BoundRole::Lower`] may prune, and only a measure declaring
/// [`BoundRole::Upper`] may provide the incumbent. Already selected edges are treated as
/// zero-cost when estimating additional work; charging them again would overestimate a
/// branch's lower bound and could silently prune the optimum.
///
/// # Determinism
///
/// Candidate edges are ordered by `(weight, HyperedgeId)`. The exact cost is invariant
/// under input permutation because [`Cost`] arithmetic is integer and associative. When
/// several derivations tie, the selected witness may legitimately differ with insertion
/// order.
///
/// # Memory
///
/// The memo table is only an optimization. It is cleared at a bounded size derived from
/// the node budget; clearing can repeat work but cannot change correctness.
pub fn min_set_cost<P, E, L, U>(
    graph: &Graph<P, E>,
    start: &PointSet,
    target: PointId,
    lower: &L,
    upper: &U,
    budget: &Budget,
) -> Result<Solution, SolveError>
where
    L: CostMeasure,
    U: CostMeasure,
{
    if graph.point(target).is_none() {
        return Err(SolveError::UnknownTarget(target));
    }
    let targets = PointSet::from_ids(graph, [target]).expect("validated target belongs to graph");
    min_set_cost_many(graph, start, &targets, lower, upper, budget)
}

/// Minimum set cost for deriving every member of `targets`.
///
/// The lower bound is the maximum per-target depth bound: every feasible derivation must
/// pay at least enough to reach each individual target. The initial executable incumbent
/// is the union of the targets' tree witnesses, with shared edges charged only once.
pub fn min_set_cost_many<P, E, L, U>(
    graph: &Graph<P, E>,
    start: &PointSet,
    targets: &PointSet,
    lower: &L,
    upper: &U,
    budget: &Budget,
) -> Result<Solution, SolveError>
where
    L: CostMeasure,
    U: CostMeasure,
{
    if graph.point_count() != start.universe_len() {
        return Err(SolveError::PointSetSizeMismatch);
    }
    if graph.point_count() != targets.universe_len() {
        return Err(SolveError::TargetSetSizeMismatch);
    }
    if !lower.is_superior() {
        return Err(SolveError::LowerMeasureNotSuperior);
    }
    if !upper.is_superior() {
        return Err(SolveError::UpperMeasureNotSuperior);
    }
    if lower.bound_role() != BoundRole::Lower {
        return Err(SolveError::InvalidLowerMeasureRole(lower.bound_role()));
    }
    if upper.bound_role() != BoundRole::Upper {
        return Err(SolveError::InvalidUpperMeasureRole(upper.bound_role()));
    }

    let started = Instant::now();
    if targets.is_empty() {
        return Ok(Solution {
            cost: Cost::ZERO,
            derivation: Vec::new(),
            lower: Cost::ZERO,
            upper: Cost::ZERO,
            proven_optimal: true,
            nodes: 0,
            pruned: 0,
            millis: elapsed_millis(started),
        });
    }

    let lower_result = knuth(graph, start, lower, None);
    let upper_result = knuth(graph, start, upper, None);
    let initial_lower = targets
        .iter()
        .map(|target| lower_result.distances[target.index()])
        .max()
        .expect("non-empty target set");

    let reached = closure(graph, start);
    let reachable = targets.iter().all(|target| reached.contains(target));
    debug_assert!(
        !initial_lower.is_finite() || reachable,
        "Knuth reported a finite cost for an unreachable point"
    );

    if !initial_lower.is_finite() {
        if reachable {
            return Err(SolveError::CostOverflow);
        }
        return Ok(Solution {
            cost: Cost::INFINITY,
            derivation: Vec::new(),
            lower: Cost::INFINITY,
            upper: Cost::INFINITY,
            proven_optimal: true,
            nodes: 0,
            pruned: 0,
            millis: elapsed_millis(started),
        });
    }

    let Some(incumbent) = derivation_from_choices_many(graph, start, targets, &upper_result) else {
        return Err(SolveError::CostOverflow);
    };
    let incumbent_cost: Cost = incumbent
        .iter()
        .map(|&edge| graph.edge_unchecked(edge).weight())
        .sum();
    if initial_lower == incumbent_cost {
        return Ok(Solution {
            cost: incumbent_cost,
            derivation: incumbent,
            lower: initial_lower,
            upper: incumbent_cost,
            proven_optimal: true,
            nodes: 0,
            pruned: 0,
            millis: elapsed_millis(started),
        });
    }

    let mut selected = vec![false; graph.edge_count()];
    let best_selected = ids_to_mask(graph, &incumbent);
    let mut goals = vec![false; graph.point_count()];
    for target in targets.iter() {
        goals[target.index()] = true;
    }

    let mut search = Search {
        graph,
        start,
        targets,
        lower,
        budget,
        started,
        best_cost: incumbent_cost,
        best_selected,
        nodes: 0,
        pruned: 0,
        seen: HashSet::new(),
    };
    let completed = search.visit(&mut goals, &mut selected, Cost::ZERO);
    let derivation = selected_to_ids(graph, &search.best_selected);

    Ok(Solution {
        cost: search.best_cost,
        derivation,
        lower: if completed {
            search.best_cost
        } else {
            initial_lower
        },
        upper: search.best_cost,
        proven_optimal: completed,
        nodes: search.nodes,
        pruned: search.pruned,
        millis: elapsed_millis(started),
    })
}

struct Search<'a, P, E, L> {
    graph: &'a Graph<P, E>,
    start: &'a PointSet,
    targets: &'a PointSet,
    lower: &'a L,
    budget: &'a Budget,
    started: Instant,
    best_cost: Cost,
    best_selected: Vec<bool>,
    nodes: u64,
    pruned: u64,
    seen: HashSet<(Vec<bool>, Vec<bool>)>,
}

impl<P, E, L: CostMeasure> Search<'_, P, E, L> {
    /// Returns false only when the budget prevented exhaustive exploration.
    fn visit(&mut self, goals: &mut [bool], selected: &mut [bool], cost: Cost) -> bool {
        if self.nodes >= self.budget.max_nodes
            || self.started.elapsed() >= Duration::from_millis(self.budget.max_millis)
        {
            return false;
        }
        self.nodes += 1;

        let derived = closure_with_mask(self.graph, self.start, Some(selected));
        let mut remaining = goals.to_vec();
        for point in derived.iter() {
            remaining[point.index()] = false;
        }

        if !remaining.iter().any(|needed| *needed) {
            if self.targets.iter().all(|target| derived.contains(target)) && cost < self.best_cost {
                self.best_cost = cost;
                self.best_selected.copy_from_slice(selected);
            }
            return true;
        }

        // Already-selected edges are prepaid when estimating additional work. Without
        // this zeroing, adding the lower bound to `cost` can double-charge them and make
        // branch-and-bound pruning unsound.
        let lower_result = knuth(self.graph, &derived, self.lower, Some(selected));
        let additional_lower = remaining
            .iter()
            .enumerate()
            .filter(|(_, needed)| **needed)
            .map(|(index, _)| lower_result.distances[index])
            .max()
            .unwrap_or(Cost::ZERO);
        let branch_lower = cost + additional_lower;
        if branch_lower >= self.best_cost {
            self.pruned += 1;
            return true;
        }

        let key = (remaining.clone(), selected.to_vec());
        if !self.seen.insert(key) {
            return true;
        }
        // Memoisation is an optimisation only. Clearing it preserves correctness while
        // bounding memory on long anytime runs.
        let memo_limit = usize::try_from(self.budget.max_nodes.min(200_000)).unwrap_or(200_000);
        if memo_limit > 0 && self.seen.len() > memo_limit {
            self.seen.clear();
        }

        let goal = remaining
            .iter()
            .enumerate()
            .filter(|(_, needed)| **needed)
            .max_by_key(|(index, _)| (lower_result.distances[*index], *index))
            .map(|(index, _)| self.graph.point_id_at(index))
            .expect("remaining contains at least one goal");

        let mut candidates: Vec<_> = self.graph.incoming_unchecked(goal).to_vec();
        candidates.sort_unstable_by_key(|&edge| (self.graph.edge_unchecked(edge).weight(), edge));

        let mut completed = true;
        for edge_id in candidates {
            if selected[edge_id.index()] {
                continue;
            }
            let edge = self.graph.edge_unchecked(edge_id);
            let next_cost = cost + edge.weight();
            if next_cost >= self.best_cost {
                self.pruned += 1;
                continue;
            }

            let mut next_goals = remaining.clone();
            next_goals[goal.index()] = false;
            for &premise in edge.tail() {
                if !derived.contains(premise) {
                    next_goals[premise.index()] = true;
                }
            }
            selected[edge_id.index()] = true;
            if !self.visit(&mut next_goals, selected, next_cost) {
                completed = false;
                selected[edge_id.index()] = false;
                break;
            }
            selected[edge_id.index()] = false;
        }
        completed
    }
}

fn ids_to_mask<P, E>(graph: &Graph<P, E>, ids: &[HyperedgeId]) -> Vec<bool> {
    let mut mask = vec![false; graph.edge_count()];
    for &id in ids {
        mask[id.index()] = true;
    }
    mask
}

fn selected_to_ids<P, E>(graph: &Graph<P, E>, selected: &[bool]) -> Vec<HyperedgeId> {
    selected
        .iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(graph.edge_id_at(index)))
        .collect()
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closure::closure_restricted;

    fn shared_graph() -> (Graph, PointSet, PointId) {
        let mut graph = Graph::new();
        let a = graph.add_point("a", ()).unwrap();
        let b = graph.add_point("b", ()).unwrap();
        let d = graph.add_point("d", ()).unwrap();
        let e = graph.add_point("e", ()).unwrap();
        let z = graph.add_point("z", ()).unwrap();
        graph
            .add_hyperedge("ab", [a], b, Cost::from_units(3), ())
            .unwrap();
        graph
            .add_hyperedge("bd", [b], d, Cost::from_units(1), ())
            .unwrap();
        graph
            .add_hyperedge("be", [b], e, Cost::from_units(1), ())
            .unwrap();
        graph
            .add_hyperedge("dez", [d, e], z, Cost::from_units(1), ())
            .unwrap();
        let start = PointSet::from_ids(&graph, [a]).unwrap();
        (graph, start, z)
    }

    #[test]
    fn bounds_and_exact_cost_separate_on_shared_prerequisite() {
        let (graph, start, target) = shared_graph();
        let depth = bounds(&graph, &start, &DepthCost);
        let tree = bounds(&graph, &start, &TreeCost);
        let solution = solve(&graph, &start, target, &Budget::default()).unwrap();

        assert_eq!(depth[target.index()], Cost::from_units(5));
        assert_eq!(solution.cost, Cost::from_units(6));
        assert_eq!(tree[target.index()], Cost::from_units(9));
        assert!(solution.proven_optimal);
        assert!(closure_restricted(&graph, &start, &solution.derivation).contains(target));
    }

    #[test]
    fn multiple_targets_share_selected_edges_and_cost() {
        let (graph, start, _) = shared_graph();
        let d = graph.point_id("d").unwrap();
        let e = graph.point_id("e").unwrap();
        let targets = PointSet::from_ids(&graph, [d, e]).unwrap();

        let solution = solve_many(&graph, &start, &targets, &Budget::default()).unwrap();
        let reached = closure_restricted(&graph, &start, &solution.derivation);

        assert!(solution.proven_optimal);
        assert_eq!(solution.cost, Cost::from_units(5));
        assert_eq!(solution.derivation.len(), 3);
        assert!(targets.iter().all(|target| reached.contains(target)));
    }

    #[test]
    fn empty_target_set_has_the_unique_empty_solution() {
        let (graph, start, _) = shared_graph();
        let solution =
            solve_many(&graph, &start, &PointSet::empty(&graph), &Budget::default()).unwrap();

        assert_eq!(solution.cost, Cost::ZERO);
        assert!(solution.derivation.is_empty());
        assert!(solution.proven_optimal);
        assert_eq!(solution.lower, Cost::ZERO);
        assert_eq!(solution.upper, Cost::ZERO);
    }

    #[test]
    fn any_unreachable_target_makes_the_target_set_unreachable() {
        let mut graph = Graph::new();
        let start_point = graph.add_point("start", ()).unwrap();
        let reachable = graph.add_point("reachable", ()).unwrap();
        let unreachable = graph.add_point("unreachable", ()).unwrap();
        graph
            .add_hyperedge(
                "reachable-edge",
                [start_point],
                reachable,
                Cost::from_units(1),
                (),
            )
            .unwrap();
        let start = PointSet::from_ids(&graph, [start_point]).unwrap();
        let targets = PointSet::from_ids(&graph, [reachable, unreachable]).unwrap();

        let solution = solve_many(&graph, &start, &targets, &Budget::default()).unwrap();

        assert_eq!(solution.cost, Cost::INFINITY);
        assert!(solution.derivation.is_empty());
        assert!(solution.proven_optimal);
    }

    #[test]
    fn exhausted_budget_returns_a_witness_for_every_target() {
        let (graph, start, _) = shared_graph();
        let targets = PointSet::from_ids(
            &graph,
            [graph.point_id("d").unwrap(), graph.point_id("e").unwrap()],
        )
        .unwrap();
        let solution = solve_many(
            &graph,
            &start,
            &targets,
            &Budget {
                max_nodes: 0,
                max_millis: 1_000,
            },
        )
        .unwrap();
        let reached = closure_restricted(&graph, &start, &solution.derivation);

        assert!(!solution.proven_optimal);
        assert!(solution.lower <= solution.cost);
        assert!(targets.iter().all(|target| reached.contains(target)));
    }

    #[test]
    fn target_set_size_mismatch_is_rejected() {
        let (graph, start, _) = shared_graph();
        let mut other: Graph<(), ()> = Graph::new();
        let target = other.add_point("other", ()).unwrap();
        let targets = PointSet::from_ids(&other, [target]).unwrap();

        assert_eq!(
            solve_many(&graph, &start, &targets, &Budget::default()),
            Err(SolveError::TargetSetSizeMismatch)
        );
    }

    #[test]
    fn empty_tail_is_reachable_in_bounds_and_solver() {
        let mut graph = Graph::new();
        let point = graph.add_point("entry", ()).unwrap();
        graph
            .add_hyperedge("entry-edge", [], point, Cost::from_units(4), ())
            .unwrap();
        let start = PointSet::empty(&graph);

        assert_eq!(
            bounds(&graph, &start, &TreeCost)[point.index()],
            Cost::from_units(4)
        );
        assert_eq!(
            solve(&graph, &start, point, &Budget::default())
                .unwrap()
                .cost,
            Cost::from_units(4)
        );
    }

    #[test]
    fn unreachable_cycle_returns_infinity() {
        let mut graph = Graph::new();
        let a = graph.add_point("a", ()).unwrap();
        let b = graph.add_point("b", ()).unwrap();
        graph
            .add_hyperedge("ab", [a], b, Cost::from_units(1), ())
            .unwrap();
        graph
            .add_hyperedge("ba", [b], a, Cost::from_units(1), ())
            .unwrap();

        let solution = solve(&graph, &PointSet::empty(&graph), a, &Budget::default()).unwrap();
        assert_eq!(solution.cost, Cost::INFINITY);
        assert!(solution.proven_optimal);
    }

    #[test]
    fn reachable_cost_overflow_is_not_reported_as_unreachable() {
        let mut graph = Graph::new();
        let a = graph.add_point("a", ()).unwrap();
        let b = graph.add_point("b", ()).unwrap();
        let c = graph.add_point("c", ()).unwrap();
        graph
            .add_hyperedge("ab", [a], b, Cost::from_units(u64::MAX - 1), ())
            .unwrap();
        graph
            .add_hyperedge("bc", [b], c, Cost::from_units(1), ())
            .unwrap();
        let start = PointSet::from_ids(&graph, [a]).unwrap();

        assert_eq!(
            solve(&graph, &start, c, &Budget::default()),
            Err(SolveError::CostOverflow)
        );
    }

    #[test]
    fn exhausted_budget_returns_a_valid_incumbent_and_open_interval() {
        let (graph, start, target) = shared_graph();
        let solution = solve(
            &graph,
            &start,
            target,
            &Budget {
                max_nodes: 0,
                max_millis: 1_000,
            },
        )
        .unwrap();

        assert!(!solution.proven_optimal);
        assert!(solution.lower <= solution.cost);
        assert_eq!(solution.cost, solution.upper);
        assert!(closure_restricted(&graph, &start, &solution.derivation).contains(target));
    }

    #[test]
    fn branch_and_bound_matches_exhaustive_search_on_seeded_small_graphs() {
        let mut random = Lcg(7);
        for trial in 0..120 {
            let mut graph = Graph::new();
            let points: Vec<_> = (0..5)
                .map(|index| graph.add_point(format!("p{index}"), ()).unwrap())
                .collect();

            for edge_index in 0..7 {
                let mut tail = Vec::new();
                for &point in &points {
                    if random.next().is_multiple_of(4) {
                        tail.push(point);
                    }
                }
                let head = points[(random.next() as usize) % points.len()];
                let weight = Cost::from_units(random.next() % 6);
                graph
                    .add_hyperedge(format!("e{edge_index}"), tail, head, weight, ())
                    .unwrap();
            }

            let start = PointSet::from_ids(&graph, [points[(random.next() as usize) % 5]]).unwrap();
            for &target in &points {
                let expected = exhaustive_cost(&graph, &start, target);
                let solution = solve(
                    &graph,
                    &start,
                    target,
                    &Budget {
                        max_nodes: 1_000_000,
                        max_millis: 5_000,
                    },
                )
                .unwrap();

                assert!(solution.proven_optimal, "trial {trial}, target {target:?}");
                assert_eq!(solution.cost, expected, "trial {trial}, target {target:?}");
                assert_eq!(solution.lower, expected);
                assert_eq!(solution.upper, expected);
                if expected.is_finite() {
                    assert!(
                        closure_restricted(&graph, &start, &solution.derivation).contains(target)
                    );
                    let reported: Cost = solution
                        .derivation
                        .iter()
                        .map(|&edge| graph.edge_unchecked(edge).weight())
                        .sum();
                    assert_eq!(reported, solution.cost);
                }
            }

            for pair in points.windows(2) {
                let targets = PointSet::from_ids(&graph, pair.iter().copied()).unwrap();
                let expected = exhaustive_cost_many(&graph, &start, &targets);
                let solution = solve_many(
                    &graph,
                    &start,
                    &targets,
                    &Budget {
                        max_nodes: 1_000_000,
                        max_millis: 5_000,
                    },
                )
                .unwrap();

                assert!(solution.proven_optimal, "trial {trial}, targets {pair:?}");
                assert_eq!(solution.cost, expected, "trial {trial}, targets {pair:?}");
                assert_eq!(solution.lower, expected);
                assert_eq!(solution.upper, expected);
                if expected.is_finite() {
                    let reached = closure_restricted(&graph, &start, &solution.derivation);
                    assert!(targets.iter().all(|target| reached.contains(target)));
                }
            }
        }
    }

    fn exhaustive_cost(graph: &Graph, start: &PointSet, target: PointId) -> Cost {
        let targets = PointSet::from_ids(graph, [target]).unwrap();
        exhaustive_cost_many(graph, start, &targets)
    }

    fn exhaustive_cost_many(graph: &Graph, start: &PointSet, targets: &PointSet) -> Cost {
        let mut best = Cost::INFINITY;
        for mask in 0..(1_u64 << graph.edge_count()) {
            let mut edges = Vec::new();
            let mut cost = Cost::ZERO;
            for index in 0..graph.edge_count() {
                if mask & (1 << index) != 0 {
                    let edge = graph.edge_id_at(index);
                    edges.push(edge);
                    cost += graph.edge_unchecked(edge).weight();
                }
            }
            if cost < best {
                let reached = closure_restricted(graph, start, &edges);
                if targets.iter().all(|target| reached.contains(target)) {
                    best = cost;
                }
            }
        }
        best
    }

    /// Internal ids are handed out in load order, so a different input order yields a
    /// completely different numbering of the same graph. That numbering carries no
    /// meaning, and this test guards the seam between "the model is a set" and "the
    /// implementation is an array": if a cost ever moves when only the input order
    /// changed, some arbitrary index has been promoted into semantics.
    ///
    /// Note the deliberate asymmetry. `cost` must be bit-identical, but the particular
    /// derivation returned need not be: the graph below has two distinct optima of cost
    /// 8 (`entry -> ab -> bd,be -> dez` and `entry -> af -> fz`), and which one wins may
    /// depend on traversal order. So the derivation is checked only for
    /// self-consistency -- it must reach the target and its weights must sum to the
    /// reported cost. Cross-language conformance uses exactly this pairing.
    #[test]
    fn cost_is_invariant_under_load_order() {
        type EdgeSpec = (&'static str, &'static [&'static str], &'static str, u64);

        const POINTS: [&str; 6] = ["a", "b", "d", "e", "f", "z"];
        const EDGES: [EdgeSpec; 7] = [
            ("entry", &[], "a", 2), // empty tail: exercises the unconditional entry path
            ("ab", &["a"], "b", 3),
            ("bd", &["b"], "d", 1),
            ("be", &["b"], "e", 1),
            ("dez", &["d", "e"], "z", 1), // b is shared, so depth < set and search runs
            ("af", &["a"], "f", 5),
            ("fz", &["f"], "z", 1), // second optimum, same cost as the route through b
        ];

        fn build(points: &[&str], edges: &[EdgeSpec]) -> Graph {
            let mut graph = Graph::new();
            for name in points {
                graph.add_point(*name, ()).unwrap();
            }
            for (name, tail, head, weight) in edges {
                let tail: Vec<_> = tail
                    .iter()
                    .map(|p| graph.point_id(p).expect("tail point declared"))
                    .collect();
                let head = graph.point_id(head).expect("head point declared");
                graph
                    .add_hyperedge(*name, tail, head, Cost::from_units(*weight), ())
                    .unwrap();
            }
            graph
        }

        fn shuffle<T: Copy>(items: &mut [T], rng: &mut Lcg) {
            for i in (1..items.len()).rev() {
                let j = (rng.next() as usize) % (i + 1);
                items.swap(i, j);
            }
        }

        // Names, not ids: ids are exactly what differs between permutations.
        fn derivation_names(graph: &Graph, solution: &Solution) -> Vec<String> {
            let mut names: Vec<_> = solution
                .derivation
                .iter()
                .map(|id| {
                    graph
                        .hyperedge(*id)
                        .expect("edge in graph")
                        .name()
                        .to_owned()
                })
                .collect();
            names.sort();
            names
        }

        let baseline = build(&POINTS, &EDGES);
        let baseline_target = baseline.point_id("z").unwrap();
        let baseline_start = PointSet::empty(&baseline);
        let expected = solve(
            &baseline,
            &baseline_start,
            baseline_target,
            &Budget::default(),
        )
        .unwrap();
        assert!(
            expected.proven_optimal,
            "the baseline must close, otherwise this test compares two guesses"
        );

        let mut rng = Lcg(0x5EED);
        let mut distinct_derivations = std::collections::BTreeSet::new();
        distinct_derivations.insert(derivation_names(&baseline, &expected));

        for round in 0..50 {
            let mut points = POINTS;
            let mut edges = EDGES;
            shuffle(&mut points, &mut rng);
            shuffle(&mut edges, &mut rng);

            let graph = build(&points, &edges);
            let target = graph.point_id("z").unwrap();
            let start = PointSet::empty(&graph);
            let got = solve(&graph, &start, target, &Budget::default()).unwrap();

            assert_eq!(
                got.cost, expected.cost,
                "round {round}: cost moved when only the load order changed"
            );
            assert_eq!(
                got.lower, expected.lower,
                "round {round}: lower bound moved"
            );
            assert_eq!(
                got.upper, expected.upper,
                "round {round}: upper bound moved"
            );
            assert!(got.proven_optimal, "round {round}: optimality was lost");

            // The derivation may legitimately differ, but it must still be a real one.
            assert!(
                closure_restricted(&graph, &start, &got.derivation).contains(target),
                "round {round}: returned derivation does not reach the target"
            );
            let summed = got.derivation.iter().fold(Cost::ZERO, |acc, id| {
                acc.saturating_add(graph.hyperedge(*id).unwrap().weight())
            });
            assert_eq!(
                summed, got.cost,
                "round {round}: derivation weights do not sum to the reported cost"
            );

            distinct_derivations.insert(derivation_names(&graph, &got));
        }

        // Not an assertion: both values are correct. Seeing more than one distinct
        // derivation merely confirms the tie above is genuinely being exercised, which
        // is what makes the cost-only comparison meaningful in the first place.
        assert!(!distinct_derivations.is_empty());
    }

    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            self.0 >> 32
        }
    }
}
