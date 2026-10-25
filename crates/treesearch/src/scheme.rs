//! The interface between a problem and the search.

use std::hash::Hash;

/// A problem the search can explore: how to start, how to branch, and how
/// to compare nodes.
///
/// Mirrors the implicit interface upstream's `treesearchsolver` expects of a
/// branching scheme. The search calls these methods in exactly the order
/// upstream does, so a scheme that counts its calls (e.g. to number its
/// nodes, as upstream's schemes do for tie-breaking) sees the same sequence.
pub trait BranchingScheme {
    /// A node of the search tree. Cloned often (queues, history, best
    /// solution), so it should be a cheap handle, e.g. an `Rc`.
    type Node: Clone;

    /// The order in which nodes are expanded: smaller first. Must be a
    /// strict order: two different nodes never compare equal (upstream
    /// breaks ties by node id).
    type Priority: Ord;

    /// Nodes with equal keys may dominate each other (upstream's
    /// `NodeHasher`).
    type DominanceKey: Hash + Eq;

    /// A new root node. Called once for the initial best solution and once
    /// at the start of every iteration.
    fn root(&self) -> Self::Node;

    /// The children of `node`, in the order they should be considered.
    fn children(&self, node: &Self::Node) -> Vec<Self::Node>;

    /// The node's depth, which picks its queue. `None` means one more than
    /// the depth being processed. A child's depth must never be smaller
    /// than the depth being processed.
    fn depth(&self, node: &Self::Node) -> Option<usize>;

    /// Whether the node has no children worth exploring: it is not queued.
    fn is_leaf(&self, node: &Self::Node) -> bool;

    /// The node's position in the expansion order.
    fn priority(&self, node: &Self::Node) -> Self::Priority;

    /// Whether `node` is a strictly better solution than `other`
    /// (upstream `better`).
    fn is_better(&self, node: &Self::Node, other: &Self::Node) -> bool;

    /// Whether nothing below `node` can be better than `incumbent`, so
    /// `node` can be pruned (upstream `bound`).
    fn cannot_beat(&self, node: &Self::Node, incumbent: &Self::Node) -> bool;

    /// The node's dominance key, or `None` if it is not compared with other
    /// nodes for dominance (upstream `comparable` false).
    fn dominance_key(&self, node: &Self::Node) -> Option<Self::DominanceKey>;

    /// Whether `node` dominates `other`: `other` can be discarded. Only
    /// called for nodes with equal dominance keys.
    fn dominates(&self, node: &Self::Node, other: &Self::Node) -> bool;
}
