//! Iterative beam search: beam searches with growing queue sizes.
//!
//! upstream: treesearchsolver, include/treesearchsolver/iterative_beam_search_2.hpp,
//! `iterative_beam_search_2`. The control flow, counters and every call to
//! the branching scheme follow upstream one for one, so the same scheme
//! explores the same nodes in the same order. Upstream features nobody in
//! this project uses are left out: `goal`, `cutoff`, `global_history`,
//! solution pools larger than one, and JSON search-tree export.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::scheme::BranchingScheme;

/// Search settings.
#[derive(Debug, Clone)]
pub struct Parameters {
    /// Queue size of the first iteration.
    pub minimum_queue_size: u64,
    /// Each iteration's queue size is the previous one times this factor
    /// (at least one more).
    pub growth_factor: f64,
    /// The search stops before an iteration whose queue size would exceed
    /// this.
    pub maximum_queue_size: u64,
    /// The search stops once more nodes than this have been expanded.
    pub maximum_nodes_expanded: Option<u64>,
    /// When to stop early: a deadline or a cancellation request.
    pub stop: Stop,
}

impl Default for Parameters {
    /// Upstream's defaults.
    fn default() -> Self {
        Parameters {
            minimum_queue_size: 1,
            growth_factor: 2.0,
            maximum_queue_size: 100_000_000,
            maximum_nodes_expanded: None,
            stop: Stop::default(),
        }
    }
}

impl Parameters {
    /// Upstream packingsolver's deterministic, "not anytime" settings: one
    /// iteration with queue size 1, then one with `queue_size`.
    pub fn fixed_queue_size(queue_size: u64) -> Self {
        Parameters {
            minimum_queue_size: 1,
            growth_factor: queue_size as f64,
            maximum_queue_size: queue_size + 1,
            ..Parameters::default()
        }
    }
}

/// A request to stop the search early, checked before each expansion.
#[derive(Debug, Clone, Default)]
pub struct Stop {
    /// Stop at this time.
    pub deadline: Option<Instant>,
    /// Stop once this flag is set, from any thread or from a new-solution
    /// callback.
    pub cancel: Option<Arc<AtomicBool>>,
    /// More flags that stop the search once set: those of the searches
    /// this one runs inside (upstream adds each enclosing algorithm's end
    /// flag to the timer it passes down).
    pub flags: Vec<Arc<AtomicBool>>,
}

impl Stop {
    /// Whether the search should stop now.
    pub fn requested(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
            || self
                .cancel
                .iter()
                .chain(&self.flags)
                .any(|flag| flag.load(Ordering::Relaxed))
    }

    /// This stop, plus `flag`.
    pub fn with_flag(&self, flag: &Arc<AtomicBool>) -> Stop {
        let mut stop = self.clone();
        stop.flags.push(Arc::clone(flag));
        stop
    }
}

/// What the search found, and how much work it did.
#[derive(Debug, Clone)]
pub struct Output<N> {
    /// The best node found. The root until a better node is found.
    pub best: N,
    /// Whether the last iteration pruned nothing, which proves `best`
    /// optimal for the scheme.
    pub optimal: bool,
    /// The queue size of the current (or last) iteration.
    pub queue_size: u64,
    /// Nodes created: roots and children.
    pub nodes_generated: u64,
    /// Nodes inserted into a queue.
    pub nodes_added: u64,
    /// Nodes taken out of a queue.
    pub nodes_processed: u64,
    /// Nodes whose children were generated.
    pub nodes_expanded: u64,
}

/// One depth's queue and dominance history.
struct Level<S: BranchingScheme> {
    /// Nodes waiting to be expanded, best priority first.
    queue: BTreeMap<S::Priority, S::Node>,
    /// Nodes added at this depth, by dominance key, to discard dominated
    /// nodes. Kept after nodes leave the queue, until the depth is done.
    history: HashMap<S::DominanceKey, Vec<S::Node>>,
}

impl<S: BranchingScheme> Level<S> {
    fn new() -> Self {
        Level {
            queue: BTreeMap::new(),
            history: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.queue.clear();
        self.history.clear();
    }

    /// Adds `node` unless a node in the history dominates it, and removes
    /// the nodes it dominates. Returns whether it was added.
    ///
    /// upstream: treesearchsolver common.hpp, `add_to_history_and_queue`.
    fn add(&mut self, scheme: &S, node: S::Node) -> bool {
        if let Some(key) = scheme.dominance_key(&node) {
            let list = self.history.entry(key).or_default();
            if list.iter().any(|other| scheme.dominates(other, &node)) {
                return false;
            }
            let queue = &mut self.queue;
            list.retain(|other| {
                let dominated = scheme.dominates(&node, other);
                if dominated {
                    // A no-op if it already left the queue.
                    queue.remove(&scheme.priority(other));
                }
                !dominated
            });
            list.push(node.clone());
        }
        self.queue.insert(scheme.priority(&node), node);
        true
    }

    /// Removes the worst node of the queue, and from the history.
    ///
    /// upstream: treesearchsolver common.hpp, `remove_from_history_and_queue`.
    fn remove_worst(&mut self, scheme: &S) {
        let Some((priority, node)) = self.queue.pop_last() else {
            return;
        };
        let Some(key) = scheme.dominance_key(&node) else {
            return;
        };
        if let Some(list) = self.history.get_mut(&key) {
            // Priorities are unique, so they identify nodes.
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

    fn worst_priority(&self) -> Option<&S::Priority> {
        self.queue.last_key_value().map(|(priority, _)| priority)
    }
}

/// Runs beam searches with growing queue sizes until one prunes nothing
/// (the best node is then optimal), the queue size would exceed
/// `maximum_queue_size`, or the search is stopped.
///
/// `on_solution` is called with the current output whenever a strictly
/// better node is found, and once more at the end of an iteration that
/// proves optimality.
pub fn iterative_beam_search<S: BranchingScheme>(
    scheme: &S,
    parameters: &Parameters,
    mut on_solution: impl FnMut(&Output<S::Node>),
) -> Output<S::Node> {
    // upstream: the solution pool is created with the root as its first
    // solution, before the first iteration's root.
    let mut output = Output {
        best: scheme.root(),
        optimal: false,
        queue_size: parameters.minimum_queue_size,
        nodes_generated: 0,
        nodes_added: 0,
        nodes_processed: 0,
        nodes_expanded: 0,
    };
    let mut levels: Vec<Level<S>> = Vec::new();
    let mut end = false;

    loop {
        // Whether this iteration has pruned nothing because of the queue
        // size.
        let mut nothing_pruned = true;
        let queue_size = output.queue_size as usize;

        let root = scheme.root();
        output.nodes_generated += 1;
        level(&mut levels, 0)
            .queue
            .insert(scheme.priority(&root), root);
        output.nodes_added += 1;

        let mut current_depth = 0;
        loop {
            while let Some((_, node)) = levels[current_depth].queue.pop_first() {
                output.nodes_processed += 1;

                if scheme.cannot_beat(&node, &output.best) {
                    continue;
                }
                if parameters.stop.requested() {
                    end = true;
                    break;
                }

                let children = scheme.children(&node);
                output.nodes_expanded += 1;

                for child in children {
                    output.nodes_generated += 1;
                    let child_depth = scheme.depth(&child).unwrap_or(current_depth + 1);
                    debug_assert!(
                        child_depth >= current_depth,
                        "child above the current depth"
                    );

                    if scheme.is_better(&child, &output.best) {
                        output.best = child.clone();
                        on_solution(&output);
                    }

                    if scheme.is_leaf(&child) || scheme.cannot_beat(&child, &output.best) {
                        continue;
                    }
                    let level = level(&mut levels, child_depth);
                    if level.queue.len() >= queue_size {
                        nothing_pruned = false;
                    }
                    let fits = level.queue.len() < queue_size
                        || level
                            .worst_priority()
                            .is_some_and(|worst| scheme.priority(&child) < *worst);
                    if fits {
                        if level.add(scheme, child) {
                            output.nodes_added += 1;
                        }
                        if level.queue.len() > queue_size {
                            level.remove_worst(scheme);
                        }
                    }
                }

                if parameters
                    .maximum_nodes_expanded
                    .is_some_and(|maximum| output.nodes_expanded > maximum)
                {
                    end = true;
                    break;
                }
            }
            if end {
                break;
            }

            levels[current_depth].clear();
            current_depth += 1;
            let deeper_nodes_left = levels[current_depth.min(levels.len())..]
                .iter()
                .any(|level| !level.queue.is_empty());
            if !deeper_nodes_left {
                break;
            }
        }
        if end {
            break;
        }

        levels.iter_mut().for_each(Level::clear);
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

/// The level at `depth`, creating it (and any shallower ones) if needed.
fn level<S: BranchingScheme>(levels: &mut Vec<Level<S>>, depth: usize) -> &mut Level<S> {
    while levels.len() <= depth {
        levels.push(Level::new());
    }
    &mut levels[depth]
}
