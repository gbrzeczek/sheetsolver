//! Generic iterative beam search.
//!
//! A port of the part of upstream `treesearchsolver` that the guillotine
//! solver uses: a branching-scheme trait ([`BranchingScheme`]) and
//! `iterative_beam_search_2` ([`iterative_beam_search`]), and the variant
//! that generates children one at a time, `iterative_beam_search`
//! ([`child_by_child_beam_search`], with [`ChildByChildScheme`]).

mod beam_search;
mod child_by_child;
mod scheme;

use std::cmp::Ordering;

pub use beam_search::{Output, Parameters, Stop, iterative_beam_search};
pub use child_by_child::{ChildByChildScheme, child_by_child_beam_search};
pub use scheme::BranchingScheme;

/// A floating-point guide value usable in a [`BranchingScheme::Priority`].
///
/// Orders like C++'s `<` on doubles, which upstream's comparators use: `-0.0`
/// and `0.0` are equal. NaN is not allowed.
#[derive(Debug, Clone, Copy)]
pub struct Guide(f64);

impl Guide {
    /// Wraps `value`. Panics in debug builds if it is NaN.
    pub fn new(value: f64) -> Self {
        debug_assert!(!value.is_nan(), "guide value is NaN");
        // Adding 0.0 turns -0.0 into 0.0, so total_cmp matches `<`.
        Guide(value + 0.0)
    }

    /// The wrapped value.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl PartialEq for Guide {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Guide {}

impl PartialOrd for Guide {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Guide {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Guide;

    #[test]
    fn guides_order_like_cpp_doubles() {
        assert_eq!(Guide::new(-0.0), Guide::new(0.0));
        assert!(Guide::new(-1.5) < Guide::new(0.0));
        assert!(Guide::new(0.25) < Guide::new(1.0));
    }
}
