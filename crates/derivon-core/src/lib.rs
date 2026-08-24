//! Cost-optimal derivation over weighted directed B-hypergraphs.
//!
//! The crate implements least-fixed-point reachability, executable derivation ordering,
//! Knuth depth/tree bounds, and budgeted branch-and-bound for minimum set cost.

pub mod closure;
pub mod cost;
pub mod graph;
pub mod solve;

pub use closure::{
    Diagnosis, ExecutableOrderError, PointSet, PointSetError, blocking_frontier, closure,
    closure_restricted, dangling_points, executable_order,
};
pub use cost::{BoundRole, CostMeasure, DepthCost, TreeCost};
pub use graph::{Cost, Graph, GraphError, Hyperedge, HyperedgeId, Point, PointId};
pub use solve::{Budget, Solution, SolveError, bounds, min_set_cost, solve, tree_derivation};
