//! Weighted directed B-hypergraph: the only data structure in the core layer.
//!
//! The mathematical specification lives in the Derivon paper. This module records the
//! engineering reasons behind the representation so they are not optimized away later.
//!
//! # What the object is
//!
//! `G = (P, H)` consists of points and hyperedges. A hyperedge
//! `h = (T, y, w)` states that once every point in tail `T` is available, head `y` can
//! be obtained at cost `w`. The head is singular; this is the B-hypergraph restriction.
//!
//! # Constraints that are easy to lose accidentally
//!
//! 1. An empty tail is legal. It is executable in every state and represents an
//!    unconditional entry step. A graph without empty-tail edges cannot derive anything
//!    for a query whose start set is empty.
//! 2. `H` is an indexed family, not a mathematical set. Two edges with identical tail,
//!    head, and weight may carry different application payloads and must remain
//!    distinguishable. Storage never deduplicates parallel alternatives.
//! 3. Acyclicity is not assumed. Grounded cycles can express real mutual derivations;
//!    ungrounded cycles cannot start themselves under least-fixed-point semantics.
//! 4. AND and OR are not object types. A tail with several points is conjunction;
//!    several hyperedges with one head are alternative routes.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign};

/// Process-local dense index of a point.
///
/// In the mathematical model `P` is unordered. The `usize` exists only because a finite
/// set can be placed in bijection with `0..n`; dense indices make point sets bit vectors
/// and adjacency an array lookup instead of a string hash in every hot-loop step.
///
/// IDs are assigned in insertion order. They are opaque handles for this graph instance,
/// not durable identities: do not serialize them or assume the same external input in a
/// different order receives the same IDs. External string names provide the stable
/// boundary identity.
///
/// Three consequences are part of the representation contract:
///
/// - Durable APIs and files use caller-provided names, never the numeric index.
/// - Reordering input must not change an exact cost. The regression suite shuffles both
///   point and edge insertion order fifty times and checks this property.
/// - An equally optimal derivation may change with insertion order because internal IDs
///   break ties. Cross-implementation checks compare costs strictly, then validate the
///   returned derivation for reachability and weight consistency rather than demanding
///   the same edge set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PointId(usize);

impl PointId {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

/// Process-local dense index of a hyperedge.
///
/// Like [`PointId`], this is an insertion-order implementation artifact. The caller's
/// stable edge name is stored separately, while this handle indexes edge storage and
/// solver bit vectors.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct HyperedgeId(usize);

impl HyperedgeId {
    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

/// Non-negative fixed-point cost expressed in caller-defined integer units.
///
/// # The core consumes cost; it never produces it
///
/// Deciding what a step is worth belongs to the application layer: measured duration,
/// a calibrated model score, or another common additive scale. The core receives that
/// number and never contains weight-producing logic. Positive transfer cannot be encoded
/// as a negative edge; represent it as an additional lower-cost edge with the relevant
/// premise.
///
/// # Why this is fixed point rather than a float
///
/// Generalized Dijkstra needs a total order for `BinaryHeap`, while `f32` and `f64` have
/// `NaN` and only a partial order. Floating-point addition is also non-associative, so
/// changing tail or load order can change low bits of a result. Fixed-point integers give
/// total ordering and deterministic addition together.
///
/// The application chooses the unit: milliseconds, thousandths of a score, or another
/// calibrated quantum. `u64::MAX` is reserved for infinity. Arithmetic saturates to
/// infinity on overflow, and the solver reports reachable overflow separately from
/// ordinary unreachability.
///
/// # Modeling assumptions
///
/// Non-negativity is required by both generalized Dijkstra and the bracketing theorem.
/// Additivity is also an axiom, not an empirical fact: fatigue, interference, and
/// forgetting can violate it in real applications. The type enforces non-negativity,
/// while the application remains responsible for choosing commensurable units whose
/// sums can be meaningfully compared across routes.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Cost(u64);

impl Cost {
    pub const ZERO: Self = Self(0);
    pub const INFINITY: Self = Self(u64::MAX);

    /// Constructs a finite cost.
    pub const fn from_units(units: u64) -> Self {
        assert!(units != u64::MAX, "u64::MAX is reserved for Cost::INFINITY");
        Self(units)
    }

    pub const fn units(self) -> Option<u64> {
        if self.0 == u64::MAX {
            None
        } else {
            Some(self.0)
        }
    }

    pub const fn is_finite(self) -> bool {
        self.0 != u64::MAX
    }

    pub const fn saturating_add(self, rhs: Self) -> Self {
        if !self.is_finite() || !rhs.is_finite() {
            return Self::INFINITY;
        }
        match self.0.checked_add(rhs.0) {
            Some(value) if value != u64::MAX => Self(value),
            _ => Self::INFINITY,
        }
    }
}

impl Add for Cost {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign for Cost {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sum for Cost {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a Cost> for Cost {
    fn sum<I: Iterator<Item = &'a Cost>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

impl fmt::Display for Cost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.units() {
            Some(units) => units.fmt(formatter),
            None => formatter.write_str("inf"),
        }
    }
}

/// A point and its opaque application payload.
///
/// The core assigns no semantics to `P`: it may be a definition, a design-state record,
/// or `()` when payloads remain in another store. Reachability and solving inspect only
/// identity. Keeping payload generic reconciles the core admission rule with lossless
/// application migration without introducing `Any` or untyped maps.
///
/// In the mathematical layer a point has identity but no required internal structure.
/// The `Point` struct exists only because the Rust storage boundary may retain a stable
/// external name and opaque payload. Neither field changes firing or cost semantics.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Point<P = ()> {
    name: String,
    data: P,
}

impl<P> Point<P> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn data(&self) -> &P {
        &self.data
    }
}

/// One derivation step and its opaque application payload.
///
/// A hyperedge is one step, while a derivation is the whole selected set of steps that
/// witnesses a query. Weight and payload belong to the complete premise set, never to
/// projected ordinary edges. The algorithms read `tail`, `head`, and `weight`, but never
/// inspect `E`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hyperedge<E = ()> {
    /// Equal to this edge's position in `Graph::edges`.
    ///
    /// This is redundant storage, but it lets an edge carry its own handle when passed
    /// through iterators and solver internals.
    id: HyperedgeId,

    /// Stable caller-provided identity; parallel edges must use distinct names.
    ///
    /// Tail and head are insufficient identity because two alternative arguments can
    /// have exactly the same structural endpoints.
    name: String,

    /// Premise set. Empty is legal; order has no semantic meaning.
    ///
    /// Closure's hot path does not repeatedly test subsets. Instead, this vector is used
    /// once to initialize an unsatisfied-premise counter, while `Graph::by_tail` tells
    /// the algorithm which counters to decrement when a point first becomes available.
    tail: Vec<PointId>,

    /// Single conclusion required by the B-hypergraph model.
    ///
    /// A multi-head step can be encoded with an intermediate point: `(T, p_star, w)`
    /// followed by zero-cost edges from `p_star` to each conclusion. That encoding is
    /// faithful for reachability and set cost, but tree cost can charge `p_star` once per
    /// conclusion and therefore produce a looser upper bound.
    head: PointId,

    /// Non-negative cost of this complete derivation step.
    ///
    /// Weight cannot be distributed over projected ordinary edges in general: the same
    /// premise/head pair may participate in several hyperedges with incompatible total
    /// weights, and conjunction can have a genuine combination effect.
    weight: Cost,

    /// Application payload that core algorithms deliberately ignore.
    ///
    /// Definitions, questions, arguments, materials, and similar domain fields can live
    /// here without entering any algorithmic contract. Weight is computed from them by
    /// the application before the edge reaches core.
    data: E,
}

impl<E> Hyperedge<E> {
    pub const fn id(&self) -> HyperedgeId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tail(&self) -> &[PointId] {
        &self.tail
    }

    pub const fn head(&self) -> PointId {
        self.head
    }

    pub const fn weight(&self) -> Cost {
        self.weight
    }

    pub fn data(&self) -> &E {
        &self.data
    }
}

/// Errors that would violate graph identity or adjacency invariants.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GraphError {
    DuplicatePoint(String),
    DuplicateHyperedge(String),
    UnknownPoint(PointId),
    NonFiniteWeight,
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePoint(name) => write!(formatter, "point id `{name}` already exists"),
            Self::DuplicateHyperedge(name) => {
                write!(formatter, "hyperedge id `{name}` already exists")
            }
            Self::UnknownPoint(id) => write!(formatter, "point id {id:?} does not belong to graph"),
            Self::NonFiniteWeight => formatter.write_str("hyperedge weight must be finite"),
        }
    }
}

impl Error for GraphError {}

/// A weighted directed B-hypergraph.
///
/// # Core storage, not a database
///
/// `P` and `E` are opaque payloads and algorithms never inspect them. External string
/// identities remain stable for the graph lifetime; dense handles make hot-path
/// adjacency array based. The graph owns rebuildable in-memory working data and provides
/// no persistence contract.
///
/// Points and edges are append-only in this version. That is deliberate: removing from
/// the middle of a `Vec` would shift later entries and invalidate every outstanding ID.
/// A future deletion API needs tombstones or generational IDs rather than `Vec::remove`.
///
/// Bulk construction remains a first-class path. All indices are rebuildable from the
/// caller's names and edge records, so terminating the process loses no application
/// truth. This crate must not become the authoritative database merely because it owns
/// an optimized in-memory representation.
#[derive(Clone, Debug)]
pub struct Graph<P = (), E = ()> {
    /// `PointId -> Point`. Its length defines the point universe, including isolated
    /// points that cannot be reconstructed from edge endpoints.
    ///
    /// Keeping isolated points is required because they may appear in a query start set
    /// even when both adjacency lists are empty.
    points: Vec<Point<P>>,

    /// Stable external point name to dense internal handle.
    ///
    /// String hashing happens when data crosses the API boundary, not in closure,
    /// Dijkstra, or branch-and-bound hot loops.
    point_ids: HashMap<String, PointId>,

    /// Sole edge owner. The position of an edge is its `HyperedgeId`.
    ///
    /// This is a vector rather than a set because structurally identical alternatives
    /// must remain distinct.
    edges: Vec<Hyperedge<E>>,

    /// Stable external edge name to dense internal handle.
    ///
    /// Solutions return dense IDs for efficient in-process use; `Hyperedge::name`
    /// translates them back to durable application identities.
    edge_ids: HashMap<String, HyperedgeId>,

    /// `PointId -> edges whose head is the point`.
    ///
    /// Multiple entries are OR alternatives. A minimal derivation normally chooses one
    /// route to produce this point; this is distinct from conjunction inside one tail.
    by_head: Vec<Vec<HyperedgeId>>,

    /// `PointId -> edges whose tail contains the point`.
    ///
    /// Multiple entries express reuse. This is the sharing structure that makes set cost
    /// semantically useful and computationally hard. Closure walks this list exactly
    /// once when the point first becomes available.
    by_tail: Vec<Vec<HyperedgeId>>,
}

impl<P, E> Default for Graph<P, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P, E> Graph<P, E> {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            point_ids: HashMap::new(),
            edges: Vec::new(),
            edge_ids: HashMap::new(),
            by_head: Vec::new(),
            by_tail: Vec::new(),
        }
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// `sum_h (|tail(h)| + 1)`, the linear-time reachability size measure.
    pub fn size(&self) -> usize {
        self.edges.iter().map(|edge| edge.tail.len() + 1).sum()
    }

    pub fn add_point(&mut self, name: impl Into<String>, data: P) -> Result<PointId, GraphError> {
        let name = name.into();
        if self.point_ids.contains_key(&name) {
            return Err(GraphError::DuplicatePoint(name));
        }

        let id = PointId(self.points.len());
        self.point_ids.insert(name.clone(), id);
        self.points.push(Point { name, data });
        self.by_head.push(Vec::new());
        self.by_tail.push(Vec::new());
        Ok(id)
    }

    pub fn point_id(&self, name: &str) -> Option<PointId> {
        self.point_ids.get(name).copied()
    }

    pub fn point(&self, id: PointId) -> Option<&Point<P>> {
        self.points.get(id.index())
    }

    pub fn point_data_mut(&mut self, id: PointId) -> Option<&mut P> {
        self.points.get_mut(id.index()).map(|point| &mut point.data)
    }

    pub fn points(&self) -> impl ExactSizeIterator<Item = (PointId, &Point<P>)> {
        self.points
            .iter()
            .enumerate()
            .map(|(index, point)| (PointId(index), point))
    }

    /// Adds an edge without deduplicating parallel alternatives.
    ///
    /// Tail order has no semantic meaning. It is canonicalised by sorting and removing
    /// duplicates so closure counters agree with mathematical set semantics.
    pub fn add_hyperedge<I>(
        &mut self,
        name: impl Into<String>,
        tail: I,
        head: PointId,
        weight: Cost,
        data: E,
    ) -> Result<HyperedgeId, GraphError>
    where
        I: IntoIterator<Item = PointId>,
    {
        let name = name.into();
        if self.edge_ids.contains_key(&name) {
            return Err(GraphError::DuplicateHyperedge(name));
        }
        if !weight.is_finite() {
            return Err(GraphError::NonFiniteWeight);
        }
        self.ensure_point(head)?;

        let mut tail: Vec<_> = tail.into_iter().collect();
        for point in &tail {
            self.ensure_point(*point)?;
        }
        tail.sort_unstable();
        tail.dedup();

        let id = HyperedgeId(self.edges.len());
        let edge = Hyperedge {
            id,
            name: name.clone(),
            tail,
            head,
            weight,
            data,
        };

        self.edge_ids.insert(name, id);
        self.by_head[head.index()].push(id);
        for point in &edge.tail {
            self.by_tail[point.index()].push(id);
        }
        self.edges.push(edge);
        Ok(id)
    }

    pub fn hyperedge_id(&self, name: &str) -> Option<HyperedgeId> {
        self.edge_ids.get(name).copied()
    }

    pub fn hyperedge(&self, id: HyperedgeId) -> Option<&Hyperedge<E>> {
        self.edges.get(id.index())
    }

    pub fn hyperedge_data_mut(&mut self, id: HyperedgeId) -> Option<&mut E> {
        self.edges.get_mut(id.index()).map(|edge| &mut edge.data)
    }

    pub fn hyperedges(&self) -> impl ExactSizeIterator<Item = &Hyperedge<E>> {
        self.edges.iter()
    }

    pub fn incoming(&self, point: PointId) -> Option<&[HyperedgeId]> {
        self.by_head.get(point.index()).map(Vec::as_slice)
    }

    pub fn outgoing(&self, point: PointId) -> Option<&[HyperedgeId]> {
        self.by_tail.get(point.index()).map(Vec::as_slice)
    }

    pub(crate) fn edge_unchecked(&self, id: HyperedgeId) -> &Hyperedge<E> {
        &self.edges[id.index()]
    }

    pub(crate) fn incoming_unchecked(&self, point: PointId) -> &[HyperedgeId] {
        &self.by_head[point.index()]
    }

    pub(crate) fn outgoing_unchecked(&self, point: PointId) -> &[HyperedgeId] {
        &self.by_tail[point.index()]
    }

    pub(crate) fn point_id_at(&self, index: usize) -> PointId {
        PointId(index)
    }

    pub(crate) fn edge_id_at(&self, index: usize) -> HyperedgeId {
        HyperedgeId(index)
    }

    fn ensure_point(&self, id: PointId) -> Result<(), GraphError> {
        if id.index() < self.points.len() {
            Ok(())
        } else {
            Err(GraphError::UnknownPoint(id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_is_a_set_but_parallel_edges_are_retained() {
        let mut graph = Graph::<(), &str>::new();
        let a = graph.add_point("a", ()).unwrap();
        let b = graph.add_point("b", ()).unwrap();

        let first = graph
            .add_hyperedge("first", [b, a, a], b, Cost::from_units(3), "proof one")
            .unwrap();
        let second = graph
            .add_hyperedge("second", [a, b], b, Cost::from_units(3), "proof two")
            .unwrap();

        assert_eq!(graph.hyperedge(first).unwrap().tail(), &[a, b]);
        assert_eq!(graph.hyperedge(second).unwrap().tail(), &[a, b]);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.incoming(b).unwrap(), &[first, second]);
    }

    #[test]
    fn fixed_point_cost_saturates_instead_of_wrapping() {
        let almost_infinite = Cost::from_units(u64::MAX - 1);
        assert_eq!(almost_infinite + Cost::from_units(1), Cost::INFINITY);
        assert_eq!(Cost::INFINITY + Cost::ZERO, Cost::INFINITY);
    }

    #[test]
    fn infinity_cannot_be_stored_as_a_real_edge_weight() {
        let mut graph = Graph::new();
        let point = graph.add_point("point", ()).unwrap();
        assert_eq!(
            graph.add_hyperedge("edge", [], point, Cost::INFINITY, ()),
            Err(GraphError::NonFiniteWeight)
        );
    }
}
