//! Reachability: which points can be obtained from a given starting set.
//!
//! # This is Horn satisfiability, not a graph traversal
//!
//! Read a hyperedge `(T, y, w)` as the Horn clause "all points in T imply y", and the
//! start set as a set of facts. The closure is exactly the unique minimal model of that
//! Horn program. This is not an analogy: it is the same object, and it gives us a strong
//! result for free. Reachability is decidable in linear time,
//! `O(sum_h (|T(h)| + 1))`, by counter-based forward chaining (Dowling and Gallier,
//! 1984).
//!
//! The algorithm gives every hyperedge a counter initialized to `|T(h)|`. When a point
//! is first added to the closure, it walks that point's `by_tail` list and decrements
//! each affected edge counter. An edge fires when its counter reaches zero. Every edge
//! is touched once per premise, hence the linear bound.
//!
//! # Why this makes the acyclicity axiom unnecessary
//!
//! Closure is a least fixed point, which exists and is unique on a finite lattice even
//! when the graph contains cycles. Ungrounded cycles simply never fire: if `A` needs
//! `B`, `B` needs `A`, and neither has another source, neither edge is initially
//! executable and neither point enters the closure.
//!
//! An unrooted cycle cannot start itself. Removing the earlier acyclicity axiom also
//! makes the model more permissive: genuinely mutual derivations, such as Euler's
//! formula relating `e^{ix}` and `sin/cos`, remain expressible.
//!
//! # The price of allowing cycles
//!
//! A cycle used to be a syntax error. It now appears as an unreachable target, and the
//! reason is not obvious from reachability alone. The core therefore owes callers a
//! diagnostic. [`blocking_frontier`] reports blocked dependencies and their strongly
//! connected components, while [`dangling_points`] performs a whole-graph grounding
//! check. These are compensation for removing the acyclicity restriction, not optional
//! presentation helpers.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use crate::graph::{Graph, HyperedgeId, PointId};

/// A dense set of points belonging to a graph with a fixed point count.
///
/// The dominant operations are membership tests, insertion, and iteration. `PointId`
/// values are dense, so a `Vec<bool>` is both simpler and cheaper on the hot path than a
/// `HashSet<PointId>`: there is no hashing, and membership is one indexed access.
///
/// The set records its universe size. Passing it to a graph with a different point count
/// is rejected by public solver APIs and asserted in infallible reachability APIs.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PointSet {
    members: Vec<bool>,
    len: usize,
}

impl PointSet {
    pub fn empty<P, E>(graph: &Graph<P, E>) -> Self {
        Self {
            members: vec![false; graph.point_count()],
            len: 0,
        }
    }

    pub fn from_ids<P, E, I>(graph: &Graph<P, E>, ids: I) -> Result<Self, PointSetError>
    where
        I: IntoIterator<Item = PointId>,
    {
        let mut set = Self::empty(graph);
        for id in ids {
            set.insert(id)?;
        }
        Ok(set)
    }

    pub fn insert(&mut self, id: PointId) -> Result<bool, PointSetError> {
        let Some(member) = self.members.get_mut(id.index()) else {
            return Err(PointSetError::UnknownPoint(id));
        };
        if *member {
            return Ok(false);
        }
        *member = true;
        self.len += 1;
        Ok(true)
    }

    pub fn contains(&self, id: PointId) -> bool {
        self.members.get(id.index()).copied().unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = PointId> + '_ {
        self.members
            .iter()
            .enumerate()
            .filter_map(|(index, present)| present.then_some(PointId::from_index(index)))
    }

    pub(crate) fn universe_len(&self) -> usize {
        self.members.len()
    }

    pub(crate) fn insert_unchecked(&mut self, id: PointId) -> bool {
        let member = &mut self.members[id.index()];
        if *member {
            false
        } else {
            *member = true;
            self.len += 1;
            true
        }
    }
}

/// Invalid dense point-set construction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PointSetError {
    UnknownPoint(PointId),
}

impl fmt::Display for PointSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPoint(id) => write!(formatter, "point id {id:?} is outside this set"),
        }
    }
}

impl Error for PointSetError {}

/// Least fixed point of all hyperedges starting from `start`.
///
/// # Empty tails must be seeded before the main loop
///
/// This is a real bug class found in the Python reference. If the work queue is primed
/// only from `start`, then an empty start produces an empty queue and unconditional
/// edges never fire, even though an empty tail is a subset of every state. This
/// implementation explicitly seeds empty-tail edges before processing queued points.
///
/// Reachability is cross-checked against the finite/infinite result of the Knuth bounds
/// in solver tests. That disagreement check is what exposes empty-tail seeding bugs.
pub fn closure<P, E>(graph: &Graph<P, E>, start: &PointSet) -> PointSet {
    closure_with_mask(graph, start, None)
}

/// Least fixed point using only the listed hyperedges.
///
/// Branch-and-bound uses this operation to ask what the current selected edge set can
/// already derive. Both unrestricted and restricted closure share
/// the internal `closure_with_mask`, so firing semantics cannot drift between copies.
pub fn closure_restricted<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    allowed: &[HyperedgeId],
) -> PointSet {
    let mut mask = vec![false; graph.edge_count()];
    for edge in allowed {
        assert!(
            graph.hyperedge(*edge).is_some(),
            "hyperedge id {edge:?} does not belong to graph"
        );
        mask[edge.index()] = true;
    }
    closure_with_mask(graph, start, Some(&mask))
}

pub(crate) fn closure_with_mask<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    allowed: Option<&[bool]>,
) -> PointSet {
    assert_eq!(
        graph.point_count(),
        start.universe_len(),
        "point set belongs to a graph with a different point count"
    );
    if let Some(mask) = allowed {
        assert_eq!(graph.edge_count(), mask.len());
    }

    let mut known = start.clone();
    let mut queue: VecDeque<_> = known.iter().collect();
    let mut need: Vec<_> = graph.hyperedges().map(|edge| edge.tail().len()).collect();

    // Empty tails are executable even when both the start set and work queue are empty.
    for edge in graph.hyperedges() {
        if allowed.is_none_or(|mask| mask[edge.id().index()])
            && edge.tail().is_empty()
            && known.insert_unchecked(edge.head())
        {
            queue.push_back(edge.head());
        }
    }

    while let Some(point) = queue.pop_front() {
        for &edge_id in graph.outgoing_unchecked(point) {
            if allowed.is_some_and(|mask| !mask[edge_id.index()]) {
                continue;
            }
            let remaining = &mut need[edge_id.index()];
            debug_assert!(*remaining > 0);
            *remaining -= 1;
            if *remaining == 0 {
                let head = graph.edge_unchecked(edge_id).head();
                if known.insert_unchecked(head) {
                    queue.push_back(head);
                }
            }
        }
    }

    known
}

/// Why a selected edge family cannot be arranged as a fully executable sequence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExecutableOrderError {
    pub blocked_edges: Vec<HyperedgeId>,
}

impl fmt::Display for ExecutableOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} selected hyperedge(s) have no executable order",
            self.blocked_edges.len()
        )
    }
}

impl Error for ExecutableOrderError {}

/// Produces a permutation in which every selected edge is executable when reached.
///
/// The earlier model used a topological order, which exists only for acyclic graphs.
/// This function instead returns an executable order `h_1, ..., h_k`: the tail of each
/// `h_i` is available from `start` plus the heads of earlier edges.
///
/// Independent branches can admit several valid orders. If the supplied edge family
/// contains an ungrounded cycle or another dead edge, no permutation of the entire
/// family is executable and [`ExecutableOrderError`] reports the blocked edges.
pub fn executable_order<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    derivation: &[HyperedgeId],
) -> Result<Vec<HyperedgeId>, ExecutableOrderError> {
    assert_eq!(graph.point_count(), start.universe_len());

    let mut selected = vec![false; graph.edge_count()];
    let mut selected_count = 0;
    for &edge in derivation {
        assert!(graph.hyperedge(edge).is_some(), "invalid hyperedge id");
        if !selected[edge.index()] {
            selected[edge.index()] = true;
            selected_count += 1;
        }
    }

    let mut known = start.clone();
    let mut need: Vec<_> = graph.hyperedges().map(|edge| edge.tail().len()).collect();
    let mut ready = VecDeque::new();

    for edge in graph.hyperedges() {
        if selected[edge.id().index()] && edge.tail().is_empty() {
            ready.push_back(edge.id());
        }
    }
    for point in known.iter() {
        satisfy_point(graph, point, &selected, &mut need, &mut ready);
    }

    let mut order = Vec::with_capacity(selected_count);
    while let Some(edge_id) = ready.pop_front() {
        order.push(edge_id);
        let head = graph.edge_unchecked(edge_id).head();
        if known.insert_unchecked(head) {
            satisfy_point(graph, head, &selected, &mut need, &mut ready);
        }
    }

    if order.len() == selected_count {
        Ok(order)
    } else {
        let ordered: std::collections::HashSet<_> = order.iter().copied().collect();
        let blocked_edges = derivation
            .iter()
            .copied()
            .filter(|edge| !ordered.contains(edge))
            .collect();
        Err(ExecutableOrderError { blocked_edges })
    }
}

fn satisfy_point<P, E>(
    graph: &Graph<P, E>,
    point: PointId,
    selected: &[bool],
    need: &mut [usize],
    ready: &mut VecDeque<HyperedgeId>,
) {
    for &edge in graph.outgoing_unchecked(point) {
        if !selected[edge.index()] {
            continue;
        }
        debug_assert!(need[edge.index()] > 0);
        need[edge.index()] -= 1;
        if need[edge.index()] == 0 {
            ready.push_back(edge);
        }
    }
}

/// Reachability obstruction relevant to one target.
///
/// `blocking` contains unreached points found while tracing all blocked alternatives
/// backward from the target. `cycles` distinguishes a circular definition from a plain
/// missing prerequisite, which require different fixes from the graph author.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnosis {
    /// Unreached points encountered while tracing blocked alternatives backward.
    pub blocking: Vec<PointId>,
    /// Non-trivial strongly connected components among the blocking dependencies.
    pub cycles: Vec<Vec<PointId>>,
}

/// Returns the target's blocking frontier and circular components within it.
///
/// Every incoming edge of an unreachable point has at least one unavailable premise.
/// The backward walk follows those unavailable premises, then Tarjan's algorithm marks
/// strongly connected components in the resulting dependency subgraph. A reachable
/// target has an empty diagnosis.
pub fn blocking_frontier<P, E>(
    graph: &Graph<P, E>,
    start: &PointSet,
    target: PointId,
) -> Diagnosis {
    assert!(graph.point(target).is_some(), "invalid target point id");
    let reached = closure(graph, start);
    if reached.contains(target) {
        return Diagnosis {
            blocking: Vec::new(),
            cycles: Vec::new(),
        };
    }

    let mut blocking = vec![false; graph.point_count()];
    let mut stack = vec![target];
    while let Some(point) = stack.pop() {
        if reached.contains(point) || blocking[point.index()] {
            continue;
        }
        blocking[point.index()] = true;

        for &edge_id in graph.incoming_unchecked(point) {
            let edge = graph.edge_unchecked(edge_id);
            for &premise in edge.tail() {
                if !reached.contains(premise) && !blocking[premise.index()] {
                    stack.push(premise);
                }
            }
        }
    }

    let blocking_points: Vec<_> = blocking
        .iter()
        .enumerate()
        .filter_map(|(index, is_blocking)| is_blocking.then_some(graph.point_id_at(index)))
        .collect();
    let cycles = blocking_cycles(graph, &blocking);

    Diagnosis {
        blocking: blocking_points,
        cycles,
    }
}

/// Points not generatable from the graph's natural entries.
///
/// The correct criterion starts closure from points with in-degree zero. Empty-tail
/// edges are seeded by closure itself. Anything outside the resulting grounded set can
/// never be generated by the graph without being supplied directly in a query start.
///
/// A tempting alternative is to assume every point except the one under inspection is
/// known. That test is wrong in both directions: members of a circular pair prop each
/// other up, while legitimate entry points are falsely reported. The cycle regression
/// uses the English concepts `Taylor series`, `geometric trigonometry`, `linearity`, and
/// `superposition principle` to preserve the reference scenario without application
/// language leaking into the API.
pub fn dangling_points<P, E>(graph: &Graph<P, E>) -> Vec<PointId> {
    let entries = PointSet::from_ids(
        graph,
        graph
            .points()
            .map(|(id, _)| id)
            .filter(|&id| graph.incoming_unchecked(id).is_empty()),
    )
    .expect("all entry ids came from this graph");
    let grounded = closure(graph, &entries);

    graph
        .points()
        .map(|(id, _)| id)
        .filter(|&id| !grounded.contains(id))
        .collect()
}

fn blocking_cycles<P, E>(graph: &Graph<P, E>, blocking: &[bool]) -> Vec<Vec<PointId>> {
    struct Tarjan {
        next_index: usize,
        indices: Vec<Option<usize>>,
        lowlink: Vec<usize>,
        stack: Vec<PointId>,
        on_stack: Vec<bool>,
        components: Vec<Vec<PointId>>,
    }

    fn visit<P, E>(graph: &Graph<P, E>, blocking: &[bool], point: PointId, state: &mut Tarjan) {
        let point_index = point.index();
        let index = state.next_index;
        state.next_index += 1;
        state.indices[point_index] = Some(index);
        state.lowlink[point_index] = index;
        state.stack.push(point);
        state.on_stack[point_index] = true;

        for &edge_id in graph.incoming_unchecked(point) {
            for &premise in graph.edge_unchecked(edge_id).tail() {
                let premise_index = premise.index();
                if !blocking[premise_index] {
                    continue;
                }
                if state.indices[premise_index].is_none() {
                    visit(graph, blocking, premise, state);
                    state.lowlink[point_index] =
                        state.lowlink[point_index].min(state.lowlink[premise_index]);
                } else if state.on_stack[premise_index] {
                    state.lowlink[point_index] = state.lowlink[point_index]
                        .min(state.indices[premise_index].expect("visited point has index"));
                }
            }
        }

        if state.lowlink[point_index] == index {
            let mut component = Vec::new();
            loop {
                let member = state.stack.pop().expect("SCC root is on stack");
                state.on_stack[member.index()] = false;
                component.push(member);
                if member == point {
                    break;
                }
            }
            state.components.push(component);
        }
    }

    let mut state = Tarjan {
        next_index: 0,
        indices: vec![None; graph.point_count()],
        lowlink: vec![0; graph.point_count()],
        stack: Vec::new(),
        on_stack: vec![false; graph.point_count()],
        components: Vec::new(),
    };

    for index in 0..graph.point_count() {
        if blocking[index] && state.indices[index].is_none() {
            visit(graph, blocking, graph.point_id_at(index), &mut state);
        }
    }

    state.components.retain(|component| {
        component.len() > 1
            || component.first().is_some_and(|&point| {
                graph
                    .incoming_unchecked(point)
                    .iter()
                    .any(|&edge| graph.edge_unchecked(edge).tail().contains(&point))
            })
    });
    for component in &mut state.components {
        component.sort_unstable();
    }
    state
        .components
        .sort_unstable_by_key(|component| component[0]);
    state.components
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Cost;

    #[test]
    fn empty_tail_seeds_an_empty_start() {
        let mut graph = Graph::new();
        let a = graph.add_point("a", ()).unwrap();
        let b = graph.add_point("b", ()).unwrap();
        graph
            .add_hyperedge("entry", [], a, Cost::from_units(2), ())
            .unwrap();
        graph
            .add_hyperedge("next", [a], b, Cost::from_units(1), ())
            .unwrap();

        let reached = closure(&graph, &PointSet::empty(&graph));
        assert!(reached.contains(a));
        assert!(reached.contains(b));
    }

    #[test]
    fn restricted_closure_and_order_use_only_selected_edges() {
        let mut graph = Graph::new();
        let a = graph.add_point("a", ()).unwrap();
        let b = graph.add_point("b", ()).unwrap();
        let c = graph.add_point("c", ()).unwrap();
        let ab = graph
            .add_hyperedge("ab", [a], b, Cost::from_units(1), ())
            .unwrap();
        let bc = graph
            .add_hyperedge("bc", [b], c, Cost::from_units(1), ())
            .unwrap();
        let start = PointSet::from_ids(&graph, [a]).unwrap();

        assert!(!closure_restricted(&graph, &start, &[bc]).contains(c));
        assert_eq!(
            executable_order(&graph, &start, &[bc, ab]).unwrap(),
            vec![ab, bc]
        );
    }

    #[test]
    fn benign_and_ungrounded_cycles_are_distinguished() {
        let mut graph = Graph::new();
        let taylor_series = graph.add_point("Taylor series", ()).unwrap();
        let geometric_trigonometry = graph.add_point("geometric trigonometry", ()).unwrap();
        let complex_exponential = graph.add_point("e^ix", ()).unwrap();
        let trigonometric_functions = graph.add_point("sin/cos", ()).unwrap();
        let linearity = graph.add_point("linearity", ()).unwrap();
        let superposition_principle = graph.add_point("superposition principle", ()).unwrap();

        for (name, tail, head) in [
            ("Taylor series to e^ix", taylor_series, complex_exponential),
            (
                "e^ix to sin/cos",
                complex_exponential,
                trigonometric_functions,
            ),
            (
                "sin/cos to e^ix",
                trigonometric_functions,
                complex_exponential,
            ),
            (
                "geometric trigonometry to sin/cos",
                geometric_trigonometry,
                trigonometric_functions,
            ),
            (
                "linearity to superposition principle",
                linearity,
                superposition_principle,
            ),
            (
                "superposition principle to linearity",
                superposition_principle,
                linearity,
            ),
        ] {
            graph
                .add_hyperedge(name, [tail], head, Cost::from_units(1), ())
                .unwrap();
        }

        let from_taylor_series = PointSet::from_ids(&graph, [taylor_series]).unwrap();
        assert!(closure(&graph, &from_taylor_series).contains(trigonometric_functions));
        assert!(!closure(&graph, &PointSet::empty(&graph)).contains(trigonometric_functions));
        assert_eq!(
            dangling_points(&graph),
            vec![linearity, superposition_principle]
        );

        let diagnosis = blocking_frontier(&graph, &from_taylor_series, linearity);
        assert_eq!(
            diagnosis.cycles,
            vec![vec![linearity, superposition_principle]]
        );
    }
}
