//! Iterative beam search that generates each node's children one at a
//! time, with one dominance history for all depths.
//!
//! upstream: treesearchsolver, include/treesearchsolver/iterative_beam_search.hpp,
//! `iterative_beam_search` (not `_2`), with `global_history` on, the only way
//! upstream packingsolver uses it (the maximal-spaces search). The control
//! flow, counters and every call to the branching scheme follow upstream one
//! for one, quirks included (see the comments in the loop). Left out as
//! unused: per-depth histories, `goal`, `cutoff`, solution pools larger than
//! one.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use crate::beam_search::{Output, Parameters};

/// A problem the search can explore one child at a time.
///
/// Like [`crate::BranchingScheme`], but the search asks for children one
/// by one, and at most `queue size` of them per node: a node's remaining
/// children are dropped once the budget is spent.
pub trait ChildByChildScheme {
    /// A node of the search tree: a cheap handle, e.g. an `Rc`. The scheme
    /// keeps track of which child comes next inside the node.
    type Node: Clone;

    /// The order in which nodes are expanded: smaller first. Must be a
    /// strict order.
    type Priority: Ord;

    /// Nodes with equal keys may dominate each other.
    type DominanceKey: Hash + Eq;

    /// A new root node. Called once for the initial best solution and once
    /// at the start of every iteration.
    fn root(&self) -> Self::Node;

    /// The node's next child, or `None` if this attempt produced none
    /// (upstream `nullptr`). Each call uses up one of the node's attempts.
    fn next_child(&self, node: &Self::Node) -> Option<Self::Node>;

    /// Whether the node has no more children to give (upstream `infertile`).
    fn infertile(&self, node: &Self::Node) -> bool;

    /// The node's depth, which picks its queue. `None` means one more than
    /// the depth being processed.
    fn depth(&self, node: &Self::Node) -> Option<usize>;

    /// Whether the node is complete: it is not queued.
    fn is_leaf(&self, node: &Self::Node) -> bool;

    /// The node's position in the expansion order.
    fn priority(&self, node: &Self::Node) -> Self::Priority;

    /// Whether `node` is a strictly better solution than `other`.
    fn is_better(&self, node: &Self::Node, other: &Self::Node) -> bool;

    /// Whether nothing below `node` can be better than `incumbent`.
    fn cannot_beat(&self, node: &Self::Node, incumbent: &Self::Node) -> bool;

    /// The node's dominance key, or `None` if it is never compared.
    fn dominance_key(&self, node: &Self::Node) -> Option<Self::DominanceKey>;

    /// Whether `node` dominates `other`. Only called for nodes with equal
    /// dominance keys.
    fn dominates(&self, node: &Self::Node, other: &Self::Node) -> bool;
}

/// The queues of one iteration, one per depth, and their shared history.
struct Queues<S: ChildByChildScheme> {
    levels: Vec<BTreeMap<S::Priority, S::Node>>,
    /// Every node added during the iteration, by dominance key, whichever
    /// its depth, until the iteration ends.
    history: HashMap<S::DominanceKey, Vec<S::Node>>,
}

impl<S: ChildByChildScheme> Queues<S> {
    fn level(&mut self, depth: usize) -> &mut BTreeMap<S::Priority, S::Node> {
        while self.levels.len() <= depth {
            self.levels.push(BTreeMap::new());
        }
        &mut self.levels[depth]
    }

    /// Adds `node` to the queue at `depth` unless a node in the history
    /// dominates it. The nodes it dominates leave the history, and the
    /// queue at `depth` if they are in it (upstream erases them from that
    /// queue only, whichever queue holds them).
    ///
    /// upstream: treesearchsolver common.hpp, `add_to_history_and_queue`.
    fn add(&mut self, scheme: &S, depth: usize, node: S::Node) {
        self.level(depth);
        if let Some(key) = scheme.dominance_key(&node) {
            let list = self.history.entry(key).or_default();
            if list.iter().any(|other| scheme.dominates(other, &node)) {
                return;
            }
            let levels = &mut self.levels;
            list.retain(|other| {
                let dominated = scheme.dominates(&node, other);
                if dominated {
                    levels[depth].remove(&scheme.priority(other));
                }
                !dominated
            });
            list.push(node.clone());
        }
        self.level(depth).insert(scheme.priority(&node), node);
    }

    /// Removes the worst node of the queue at `depth`, and from the
    /// history.
    ///
    /// upstream: treesearchsolver common.hpp, `remove_from_history_and_queue`.
    fn remove_worst(&mut self, scheme: &S, depth: usize) {
        let Some((priority, node)) = self.levels[depth].pop_last() else {
            return;
        };
        let Some(key) = scheme.dominance_key(&node) else {
            return;
        };
        if let Some(list) = self.history.get_mut(&key) {
            if let Some(index) = list
                .iter()
                .position(|other| scheme.priority(other) == priority)
            {
                list.swap_remove(index);
            }
            if list.is_empty() {
                self.history.remove(&key);
            }
        }
    }
}

/// Runs beam searches with growing queue sizes, generating at most `queue
/// size` children per node, until one prunes nothing, the queue size would
/// exceed `maximum_queue_size`, or the search is stopped.
///
/// `on_solution` is called whenever a strictly better node is found, and
/// once more at the end of an iteration that proves optimality. Here
/// `parameters.maximum_nodes_expanded` limits the children generated
/// (upstream `number_of_nodes`), which `nodes_generated` counts.
pub fn child_by_child_beam_search<S: ChildByChildScheme>(
    scheme: &S,
    parameters: &Parameters,
    mut on_solution: impl FnMut(&Output<S::Node>),
) -> Output<S::Node> {
    // upstream: the solution pool starts with a root.
    let mut output = Output {
        best: scheme.root(),
        optimal: false,
        queue_size: parameters.minimum_queue_size,
        nodes_generated: 0,
        nodes_added: 0,
        nodes_processed: 0,
        nodes_expanded: 0,
    };
    let mut queues = Queues::<S> {
        levels: Vec::new(),
        history: HashMap::new(),
    };
    let mut end = false;

    loop {
        let mut nothing_pruned = true;
        let queue_size = output.queue_size as usize;

        let root = scheme.root();
        queues.level(0).insert(scheme.priority(&root), root);

        let mut current_depth = 0;
        loop {
            while let Some((_, node)) = queues.levels[current_depth].pop_first() {
                output.nodes_processed += 1;
                if scheme.cannot_beat(&node, &output.best) {
                    continue;
                }
                if parameters.stop.requested() {
                    end = true;
                    break;
                }

                output.nodes_expanded += 1;
                let mut all_children_generated = false;
                for _ in 0..queue_size {
                    if scheme.infertile(&node) {
                        all_children_generated = true;
                        break;
                    }
                    let Some(child) = scheme.next_child(&node) else {
                        continue;
                    };
                    output.nodes_generated += 1;

                    // upstream: these stops only leave the loop over this
                    // node's children. The stop request is seen again
                    // before the next node; the node limit is not, so each
                    // remaining node of this depth still gets one child.
                    if parameters.stop.requested() {
                        end = true;
                        break;
                    }
                    if parameters
                        .maximum_nodes_expanded
                        .is_some_and(|maximum| output.nodes_generated > maximum)
                    {
                        end = true;
                        break;
                    }

                    let child_depth = scheme.depth(&child).unwrap_or(current_depth + 1);
                    if scheme.is_better(&child, &output.best) {
                        output.best = child.clone();
                        on_solution(&output);
                    }

                    if scheme.is_leaf(&child) || scheme.cannot_beat(&child, &output.best) {
                        continue;
                    }
                    let level = queues.level(child_depth);
                    if level.len() >= queue_size {
                        nothing_pruned = false;
                    }
                    let fits = level.len() < queue_size
                        || level
                            .last_key_value()
                            .is_some_and(|(worst, _)| scheme.priority(&child) < *worst);
                    if fits {
                        queues.add(scheme, child_depth, child);
                        output.nodes_added += 1;
                        if queues.levels[child_depth].len() > queue_size {
                            queues.remove_worst(scheme, child_depth);
                        }
                    }
                }
                if !all_children_generated {
                    nothing_pruned = false;
                }
            }
            if end {
                break;
            }

            queues.levels[current_depth].clear();
            current_depth += 1;
            let deeper_nodes_left = queues.levels[current_depth.min(queues.levels.len())..]
                .iter()
                .any(|level| !level.is_empty());
            if !deeper_nodes_left {
                break;
            }
        }
        if end {
            break;
        }

        queues.levels.iter_mut().for_each(BTreeMap::clear);
        queues.history.clear();
        if nothing_pruned {
            output.optimal = true;
            on_solution(&output);
        }

        // upstream: (NodeId)(size * growth_factor) truncates.
        let next_queue_size = (output.queue_size + 1)
            .max((output.queue_size as f64 * parameters.growth_factor) as u64);
        if next_queue_size > parameters.maximum_queue_size {
            break;
        }
        output.queue_size = next_queue_size;
        if nothing_pruned {
            break;
        }
    }

    output
}
