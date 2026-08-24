//! Cost measures: how per-step costs aggregate into a whole derivation.
//!
//! # The distinction that governs this module
//!
//! A [`CostMeasure`] is a local recurrence. It answers "what does `y` cost?" using only
//! the costs of `y`'s immediate premises:
//!
//! ```text
//! d(y) = min over h with head y of combine(w(h), [d(p) for p in T(h)])
//! ```
//!
//! Locality is exactly what makes a Dijkstra-style greedy algorithm correct. Knuth
//! (1977) characterized the required condition: aggregation must be a superior function,
//! monotone in every premise and never smaller than any premise.
//!
//! Set cost is not a [`CostMeasure`] and cannot be turned into one. Whether branch `D`
//! must pay for prerequisite `B` depends on whether another branch has already selected
//! the edge that derives `B`. The answer depends on the globally selected edge set, not
//! only on the numerical costs of immediate premises. This is why set cost is NP-hard
//! while [`TreeCost`] and [`DepthCost`] are polynomial bounds.
//!
//! # Bracketing theorem
//!
//! For non-negative weights:
//!
//! ```text
//! depth(S, t) <= minimum_set_cost(S, t) <= tree(S, t)
//! ```
//!
//! All three values coincide when every hyperedge has at most one premise. Both bounds
//! are polynomial, so equality pins the exact answer without search. The paper's random
//! graph measurements close this bracket in roughly 97 to 99 percent of reachable
//! queries, though deliberately shared hub structures produce wider intervals.
//!
//! # Custom measures
//!
//! Custom local recurrences are supported, but their declared [`BoundRole`] is a
//! soundness contract. The solver refuses to use a non-superior recurrence in Knuth's
//! algorithm and refuses to use a measure with the wrong role for pruning or an
//! incumbent.

use crate::graph::Cost;

/// Proven relationship between a local measure and minimum set cost.
///
/// This is a soundness declaration, not a performance hint. A false declaration can
/// prune away the optimum or install an invalid incumbent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoundRole {
    /// Never exceeds minimum set cost; admissible as a search lower bound.
    Lower,
    /// Never falls below minimum set cost and provides an executable witness.
    Upper,
    /// No proven relationship; must not be used for pruning or an incumbent.
    Neither,
}

/// A local recurrence suitable for Knuth's generalized Dijkstra when superior.
///
/// `premises` is empty for an unconditional hyperedge, so every implementation must
/// define that case. Implementors are responsible for reporting the superior-function
/// property and bound role honestly; [`mod@crate::solve`] validates those declarations
/// before starting set-cost search.
pub trait CostMeasure {
    /// Aggregates one step's own cost with the costs of its premises.
    ///
    /// `premises` is empty for an unconditional hyperedge. Forgetting this case can make
    /// Dijkstra disagree with closure when the query start set is empty.
    fn combine(&self, step: Cost, premises: &[Cost]) -> Cost;

    /// Whether the recurrence is monotone and never below any premise.
    ///
    /// Returning `true` permits the greedy generalized-Dijkstra implementation. A false
    /// declaration can make its settled-point invariant invalid; returning `false`
    /// causes the solver to reject the measure rather than continue unsafely.
    fn is_superior(&self) -> bool;

    /// Declares the proven relationship to minimum set cost.
    ///
    /// The branch-and-bound solver validates this before using the measure. A custom
    /// measure with [`BoundRole::Neither`] remains usable for standalone experiments but
    /// cannot prune or seed an exact set-cost solve.
    fn bound_role(&self) -> BoundRole;
}

/// Unfolded-tree cost: `step + sum(premises)`.
///
/// A shared sub-derivation is charged once per use, so this overestimates set cost. Its
/// chosen edges still form a real executable derivation, which supplies the anytime
/// solver's initial incumbent.
///
/// For example, if point `B` is needed independently by branches `D` and `E`, the tree
/// recurrence includes the cost of deriving `B` in both premise subtrees. Converting the
/// resulting edge multiset to its unique support can only lower the total, which proves
/// the upper-bound side of the bracket.
#[derive(Clone, Copy, Default, Debug)]
pub struct TreeCost;

/// Critical-path cost: `step + max(premises)`.
///
/// This is the shortest completion time under unlimited parallelism. It ignores the
/// aggregate cost of independent branches and therefore lower-bounds set cost.
///
/// With an empty premise list, `max` has no element, so the recurrence uses zero and the
/// result is exactly the unconditional edge's own cost.
#[derive(Clone, Copy, Default, Debug)]
pub struct DepthCost;

impl CostMeasure for TreeCost {
    fn combine(&self, step: Cost, premises: &[Cost]) -> Cost {
        step + premises.iter().copied().sum()
    }

    fn is_superior(&self) -> bool {
        true
    }

    fn bound_role(&self) -> BoundRole {
        BoundRole::Upper
    }
}

impl CostMeasure for DepthCost {
    fn combine(&self, step: Cost, premises: &[Cost]) -> Cost {
        step + premises.iter().copied().max().unwrap_or(Cost::ZERO)
    }

    fn is_superior(&self) -> bool {
        true
    }

    fn bound_role(&self) -> BoundRole {
        BoundRole::Lower
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_measures_handle_empty_and_multiple_tails() {
        let one = Cost::from_units(1);
        let three = Cost::from_units(3);
        let four = Cost::from_units(4);

        assert_eq!(TreeCost.combine(one, &[]), one);
        assert_eq!(DepthCost.combine(one, &[]), one);
        assert_eq!(TreeCost.combine(one, &[three, four]), Cost::from_units(8));
        assert_eq!(DepthCost.combine(one, &[three, four]), Cost::from_units(5));
    }
}
