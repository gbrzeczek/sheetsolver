//! The maximal-spaces search: one panel, unlimited stages.
//!
//! upstream: src/rectangleguillotine/block.cpp (`compute_blocks`),
//! tree_search_maximal_spaces.hpp and .cpp (`BranchingSchemeMaximalSpaces`,
//! `tree_search_maximal_spaces`) and sequential_feasibility.cpp.
//!
//! Parts are first combined into *blocks*: grids of one part type, and
//! pairs of smaller blocks side by side. A node is a partial plan: blocks
//! placed in the bottom-left corner of *empty spaces*, each placement
//! splitting its space in two with a guillotine cut whose direction
//! alternates with the depth. A child places one block in the smallest
//! empty space. Nodes are ordered by the value a greedy completion reaches.
//!
//! The search itself maximizes the value cut from the panel (knapsack, or
//! "cut everything" subproblems). Bin packing on one panel and the open
//! dimensions are solved by [`sequential_feasibility`]: the same search on
//! ever shorter panels until the parts no longer fit.

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use treesearch::{ChildByChildScheme, Guide, Parameters, Stop, child_by_child_beam_search};

use crate::cpp_sort;
use crate::plan::{Plan, PlanBuilder};
use crate::problem::{BinType, Objective, Orientation, Problem, Size};

/// upstream: block.hpp, `BlockParameters::maximum_number_of_blocks`.
const MAXIMUM_NUMBER_OF_BLOCKS: usize = 10_000;

/// A rectangle of parts cut from one piece by guillotine cuts (upstream
/// `Block`).
#[derive(Debug, Clone)]
struct Block {
    /// The block's outline.
    size: Size,
    item_area: i64,
    item_profit: f64,
    number_of_items: u32,
    /// Copies of each part type in the block, by increasing part type.
    item_copies: Vec<(usize, u32)>,
    kind: BlockKind,
}

#[derive(Debug, Clone, Copy)]
enum BlockKind {
    /// A grid of one part type, turned or not.
    Grid { item_type: usize, rotate: bool },
    /// Two earlier blocks side by side: along x (`vertical`: a cut at a
    /// fixed x between them) or along y.
    Pair {
        vertical: bool,
        first: usize,
        second: usize,
    },
}

impl Block {
    fn fill_rate(&self) -> f64 {
        if self.size.w > 0 && self.size.h > 0 {
            self.item_area as f64 / self.size.area() as f64
        } else {
            0.0
        }
    }

    fn efficiency(&self) -> f64 {
        if self.size.w > 0 && self.size.h > 0 {
            self.item_profit / self.size.area() as f64
        } else {
            0.0
        }
    }

    /// The order blocks are combined in: blocks of several parts by
    /// increasing value per area, then by decreasing number of parts;
    /// single parts last.
    ///
    /// upstream: block.cpp, `BlockFillRateLess`.
    fn rank(&self) -> (bool, Guide, Reverse<u32>) {
        (
            self.number_of_items == 1,
            Guide::new(self.efficiency()),
            Reverse(self.number_of_items),
        )
    }

    /// What makes two blocks the same (upstream `BlockKey`).
    fn key(&self) -> (Size, Vec<(usize, u32)>) {
        (self.size, self.item_copies.clone())
    }
}

/// Merges two sorted lists of part copies.
///
/// upstream: block.cpp, `merge_item_copies`.
fn merge_item_copies(first: &[(usize, u32)], second: &[(usize, u32)]) -> Vec<(usize, u32)> {
    let mut result = Vec::with_capacity(first.len() + second.len());
    let (mut i, mut j) = (0, 0);
    while i < first.len() || j < second.len() {
        if j == second.len() || (i < first.len() && first[i].0 < second[j].0) {
            result.push(first[i]);
            i += 1;
        } else if i == first.len() || first[i].0 > second[j].0 {
            result.push(second[j]);
            j += 1;
        } else {
            result.push((first[i].0, first[i].1 + second[j].1));
            i += 1;
            j += 1;
        }
    }
    result
}

/// Whether two blocks together hold no more copies of any part type than
/// it has.
///
/// upstream: block.cpp, `is_valid_item_copies`.
fn within_quantities(problem: &Problem, first: &[(usize, u32)], second: &[(usize, u32)]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < first.len() || j < second.len() {
        let (item_type, copies) =
            if j == second.len() || (i < first.len() && first[i].0 < second[j].0) {
                i += 1;
                first[i - 1]
            } else if i == first.len() || first[i].0 > second[j].0 {
                j += 1;
                second[j - 1]
            } else {
                i += 1;
                j += 1;
                (first[i - 1].0, first[i - 1].1 + second[j - 1].1)
            };
        if copies > problem.item_types[item_type].copies {
            return false;
        }
    }
    true
}

/// The blocks for a panel, in the order the search refers to them.
///
/// upstream: block.cpp, `compute_blocks_for_bin`. Blocks wait in a queue
/// ordered by [`Block::rank`] (a `std::multiset`: among equal ranks, the
/// oldest first); the best is taken out, combined with every block taken
/// out before it, and kept. The queue keeps at most
/// [`MAXIMUM_NUMBER_OF_BLOCKS`] blocks, dropping the worst.
fn compute_blocks(problem: &Problem, bin: &BinType) -> Vec<Block> {
    let cut_thickness = problem.cut_thickness;
    let (bin_w, bin_h) = (bin.size.w, bin.size.h);

    type Rank = (bool, Guide, Reverse<u32>);
    let mut queue: BTreeMap<(Rank, u64), Block> = BTreeMap::new();
    let mut insertions = 0u64;
    let mut seen: HashSet<(Size, Vec<(usize, u32)>)> = HashSet::new();
    // The rank of the queue's worst block, kept at hand: it is compared
    // with every candidate pair once the queue is full.
    let mut worst_rank: Option<Rank> = None;
    let mut enqueue = |queue: &mut BTreeMap<(Rank, u64), Block>,
                       seen: &mut HashSet<(Size, Vec<(usize, u32)>)>,
                       worst_rank: &mut Option<Rank>,
                       block: Block| {
        queue.insert((block.rank(), insertions), block);
        insertions += 1;
        if queue.len() > MAXIMUM_NUMBER_OF_BLOCKS {
            let (_, worst) = queue.pop_first().expect("the queue is not empty");
            seen.remove(&worst.key());
        }
        *worst_rank = queue.first_key_value().map(|((rank, _), _)| *rank);
    };

    for (item_type, item) in problem.item_types.iter().enumerate() {
        if item.copies < 1 {
            continue;
        }
        let rotations: &[bool] = if item.oriented {
            &[false]
        } else {
            &[false, true]
        };
        for &rotate in rotations {
            let (w, h) = (item.size.width(rotate), item.size.height(rotate));
            let max_x = i64::from(item.copies).min((bin_w + cut_thickness) / (w + cut_thickness));
            for cx in 1..=max_x {
                let max_y = (i64::from(item.copies) / cx)
                    .min((bin_h + cut_thickness) / (h + cut_thickness));
                for cy in 1..=max_y {
                    let count = (cx * cy) as u32;
                    let block = Block {
                        size: Size {
                            w: cx * w + (cx - 1) * cut_thickness,
                            h: cy * h + (cy - 1) * cut_thickness,
                        },
                        item_area: (cx * cy) * item.size.area(),
                        item_profit: f64::from(count) * item.profit,
                        number_of_items: count,
                        item_copies: vec![(item_type, count)],
                        kind: BlockKind::Grid { item_type, rotate },
                    };
                    if seen.insert(block.key()) {
                        enqueue(&mut queue, &mut seen, &mut worst_rank, block);
                    }
                }
            }
        }
    }

    let mut blocks: Vec<Block> = Vec::new();
    while let Some((_, block)) = queue.pop_last() {
        worst_rank = queue.first_key_value().map(|((rank, _), _)| *rank);
        if blocks.len() >= MAXIMUM_NUMBER_OF_BLOCKS {
            break;
        }
        if problem.item_types.len() <= 3 && block.fill_rate() != 1.0 {
            continue;
        }
        let current = blocks.len();
        for (existing_index, existing) in blocks.iter().enumerate() {
            if let (
                BlockKind::Grid {
                    item_type: a,
                    rotate: ra,
                },
                BlockKind::Grid {
                    item_type: b,
                    rotate: rb,
                },
            ) = (block.kind, existing.kind)
            {
                if a == b && ra == rb {
                    continue;
                }
            }
            for vertical in [true, false] {
                let size = if vertical {
                    Size {
                        w: block.size.w + existing.size.w + cut_thickness,
                        h: block.size.h.max(existing.size.h),
                    }
                } else {
                    Size {
                        w: block.size.w.max(existing.size.w),
                        h: block.size.h + existing.size.h + cut_thickness,
                    }
                };
                if size.w > bin_w || size.h > bin_h {
                    continue;
                }
                let mut combined = Block {
                    size,
                    item_area: block.item_area + existing.item_area,
                    item_profit: block.item_profit + existing.item_profit,
                    number_of_items: block.number_of_items + existing.number_of_items,
                    item_copies: Vec::new(),
                    kind: BlockKind::Pair {
                        vertical,
                        first: current,
                        second: existing_index,
                    },
                };
                // A full queue only takes blocks better than its worst.
                if queue.len() >= MAXIMUM_NUMBER_OF_BLOCKS
                    && worst_rank.is_some_and(|worst| worst >= combined.rank())
                {
                    continue;
                }
                if !within_quantities(problem, &block.item_copies, &existing.item_copies) {
                    continue;
                }
                combined.item_copies = merge_item_copies(&block.item_copies, &existing.item_copies);
                if seen.insert(combined.key()) {
                    enqueue(&mut queue, &mut seen, &mut worst_rank, combined);
                }
            }
        }
        blocks.push(block);
    }
    blocks
}

/// An empty rectangle of a partial plan: a leaf of the cutting tree at
/// `depth` (upstream `EmptySpace`).
#[derive(Debug, Clone, Copy)]
struct EmptySpace {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    depth: u32,
    /// The panel's first-cut direction; `Any` only before the first block.
    orientation: Orientation,
}

impl EmptySpace {
    fn area(&self) -> i64 {
        self.width * self.height
    }
}

/// Placing a block in an empty space (upstream `Insertion`).
#[derive(Debug, Clone, Copy)]
struct Insertion {
    block: usize,
    space: usize,
    orientation: Orientation,
    /// Value of the block per area of the space.
    guide: f64,
}

#[derive(Debug, Clone, Copy)]
struct PlacedBlock {
    block: usize,
    x: i64,
    y: i64,
    orientation: Orientation,
}

/// A partial plan.
#[derive(Debug, Clone)]
struct State {
    placed_blocks: Vec<PlacedBlock>,
    empty_spaces: Vec<EmptySpace>,
    /// Copies placed of each part type.
    copies: Vec<u32>,
    profit: f64,
    number_of_items: u32,
    number_of_blocks: u32,
    /// The blocks that still fit within the part quantities.
    valid_blocks: Vec<usize>,
}

/// A node of the search (upstream `BranchingSchemeMaximalSpaces::Node`).
#[derive(Debug)]
struct MsNode {
    /// upstream `id`: creation order, for tie-breaking.
    id: u64,
    state: State,
    /// Value of the greedy completion of the plan.
    greedy_value: f64,
    /// The node's children, computed when the first is asked for.
    insertions: RefCell<Vec<Insertion>>,
    /// How many children have been asked for.
    next_child: Cell<usize>,
}

type NodeRef = Rc<MsNode>;

/// Nodes with the same copies placed compare for dominance.
struct CopiesPlaced(NodeRef);

impl PartialEq for CopiesPlaced {
    fn eq(&self, other: &Self) -> bool {
        self.0.state.copies == other.0.state.copies
    }
}

impl Eq for CopiesPlaced {}

impl Hash for CopiesPlaced {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.0.state.copies.hash(hasher);
    }
}

/// The maximal-spaces branching scheme (upstream
/// `BranchingSchemeMaximalSpaces`).
struct Scheme<'a> {
    problem: &'a Problem,
    blocks: Vec<Block>,
    bin_type: usize,
    first_cut_orientation: Orientation,
    /// upstream `node_id_`: also advanced by the greedy completions.
    next_id: Cell<u64>,
}

/// Whether the cut that places a block at `depth` is vertical (at a fixed
/// x), for first cuts in `orientation`.
///
/// upstream: `BranchingSchemeMaximalSpaces::cut_is_vertical`.
fn cut_is_vertical(depth: u32, orientation: Orientation) -> bool {
    let even = depth % 2 == 0;
    if orientation == Orientation::Vertical {
        even
    } else {
        !even
    }
}

impl<'a> Scheme<'a> {
    fn new(problem: &'a Problem) -> Self {
        let bin_type = problem.bin_type_of_position(0);
        Scheme {
            problem,
            blocks: compute_blocks(problem, &problem.bin_types[bin_type]),
            bin_type,
            first_cut_orientation: problem.first_stage_orientation,
            next_id: Cell::new(0),
        }
    }

    fn take_id(&self) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }

    fn fits(block: &Block, space: &EmptySpace) -> bool {
        block.size.w <= space.width && block.size.h <= space.height
    }

    /// The empty space with the smallest area, then closest to the origin.
    ///
    /// upstream: `BranchingSchemeMaximalSpaces::find_best_space`.
    fn best_space(spaces: &[EmptySpace]) -> Option<usize> {
        let mut best: Option<(usize, i64, i64)> = None;
        for (index, space) in spaces.iter().enumerate() {
            let (area, distance) = (space.area(), space.x + space.y);
            if best.is_none_or(|(_, best_area, best_distance)| {
                area < best_area || (area == best_area && distance < best_distance)
            }) {
                best = Some((index, area, distance));
            }
        }
        best.map(|(index, _, _)| index)
    }

    fn guide(block: &Block, space: &EmptySpace) -> f64 {
        if space.area() > 0 {
            block.item_profit / space.area() as f64
        } else {
            0.0
        }
    }

    fn root_state(&self) -> State {
        let size = self.problem.bin_types[self.bin_type].size;
        State {
            placed_blocks: Vec::new(),
            empty_spaces: vec![EmptySpace {
                x: 0,
                y: 0,
                width: size.w,
                height: size.h,
                depth: 0,
                orientation: self.first_cut_orientation,
            }],
            copies: vec![0; self.problem.item_types.len()],
            profit: 0.0,
            number_of_items: 0,
            number_of_blocks: 0,
            valid_blocks: (0..self.blocks.len()).collect(),
        }
    }

    /// The children of a node, best first.
    ///
    /// upstream: `BranchingSchemeMaximalSpaces::insertions`, sorted with
    /// `std::sort`, ties included (see [`crate::cpp_sort`]).
    fn insertions(&self, state: &State) -> Vec<Insertion> {
        let Some(space_index) = Self::best_space(&state.empty_spaces) else {
            return Vec::new();
        };
        let space = state.empty_spaces[space_index];
        let both = space.orientation == Orientation::Any;
        let mut insertions = Vec::new();
        for &block_index in &state.valid_blocks {
            let block = &self.blocks[block_index];
            if !Self::fits(block, &space) {
                continue;
            }
            let guide = Self::guide(block, &space);
            insertions.push(Insertion {
                block: block_index,
                space: space_index,
                orientation: if both {
                    Orientation::Vertical
                } else {
                    space.orientation
                },
                guide,
            });
            if both {
                insertions.push(Insertion {
                    block: block_index,
                    space: space_index,
                    orientation: Orientation::Horizontal,
                    guide,
                });
            }
        }
        cpp_sort::sort(&mut insertions, |a, b| {
            if a.guide != b.guide {
                return a.guide > b.guide;
            }
            // Vertical first.
            a.orientation == Orientation::Vertical && b.orientation == Orientation::Horizontal
        });
        insertions
    }

    /// The greedy choice: the block of highest value per area in the
    /// smallest space, the first one on ties; `None` if no block fits.
    ///
    /// upstream: `BranchingSchemeMaximalSpaces::best_insertion`.
    fn best_insertion(&self, state: &State) -> Option<Insertion> {
        let space_index = Self::best_space(&state.empty_spaces)?;
        let space = state.empty_spaces[space_index];
        let orientation = if space.orientation == Orientation::Any {
            Orientation::Vertical
        } else {
            space.orientation
        };
        let mut best: Option<Insertion> = None;
        let mut best_guide = -1.0;
        for &block_index in &state.valid_blocks {
            let block = &self.blocks[block_index];
            if !Self::fits(block, &space) {
                continue;
            }
            let guide = Self::guide(block, &space);
            if guide > best_guide {
                best_guide = guide;
                best = Some(Insertion {
                    block: block_index,
                    space: space_index,
                    orientation,
                    guide,
                });
            }
        }
        best
    }

    /// Places a block: the space is replaced by the two pieces its cuts
    /// leave, and spaces no block fits in any more are dropped. Takes a
    /// node id, as upstream does, even for greedy completions.
    ///
    /// upstream: `BranchingSchemeMaximalSpaces::apply_insertion`.
    fn apply(&self, state: &mut State, insertion: &Insertion) -> u64 {
        let space = state.empty_spaces[insertion.space];
        let block = &self.blocks[insertion.block];
        let (w, h) = (block.size.w, block.size.h);
        let cut_thickness = self.problem.cut_thickness;
        let orientation = insertion.orientation;

        state.placed_blocks.push(PlacedBlock {
            block: insertion.block,
            x: space.x,
            y: space.y,
            orientation,
        });
        for &(item_type, copies) in &block.item_copies {
            state.copies[item_type] += copies;
        }
        state.profit += block.item_profit;
        state.number_of_items += block.number_of_items;
        state.number_of_blocks += 1;
        let id = self.take_id();

        state.empty_spaces.remove(insertion.space);
        let right = |height: i64, depth: u32| EmptySpace {
            x: space.x + w + cut_thickness,
            y: space.y,
            width: space.x + space.width - space.x - w - cut_thickness,
            height,
            depth,
            orientation,
        };
        let above = |width: i64, depth: u32| EmptySpace {
            x: space.x,
            y: space.y + h + cut_thickness,
            width,
            height: space.y + space.height - space.y - h - cut_thickness,
            depth,
            orientation,
        };
        let room_right = space.width > w + cut_thickness;
        let room_above = space.height > h + cut_thickness;
        if cut_is_vertical(space.depth, orientation) {
            if room_right {
                state
                    .empty_spaces
                    .push(right(space.height, space.depth + 1));
            }
            if room_above {
                state.empty_spaces.push(above(w, space.depth + 2));
            }
        } else {
            if room_above {
                state.empty_spaces.push(above(space.width, space.depth + 1));
            }
            if room_right {
                state.empty_spaces.push(right(h, space.depth + 2));
            }
        }

        self.prune(state);
        id
    }

    /// Drops the blocks that no longer fit within the part quantities, then
    /// the spaces no remaining block fits in (upstream swaps each with the
    /// last space).
    fn prune(&self, state: &mut State) {
        self.prune_blocks(state);
        let mut index = 0;
        while index < state.empty_spaces.len() {
            let space = state.empty_spaces[index];
            let fits = state
                .valid_blocks
                .iter()
                .any(|&block_index| Self::fits(&self.blocks[block_index], &space));
            if fits {
                index += 1;
            } else {
                state.empty_spaces.swap_remove(index);
            }
        }
    }

    /// The state after placing greedy choices until no block fits.
    fn greedy_completion(&self, state: &State) -> State {
        let mut greedy = state.clone();
        while let Some(insertion) = self.best_insertion(&greedy) {
            self.apply(&mut greedy, &insertion);
        }
        greedy
    }

    /// The plan of a node: its greedy completion, replayed into a cutting
    /// tree.
    ///
    /// upstream: `BranchingSchemeMaximalSpaces::to_solution`.
    fn to_plan(&self, node: &MsNode) -> Plan {
        let greedy = self.greedy_completion(&node.state);
        let orientation = greedy
            .placed_blocks
            .first()
            .map_or(Orientation::Vertical, |placed| placed.orientation);
        let mut tree = CutTree::new(self, orientation);
        tree.replay(&greedy.placed_blocks);

        let mut builder = PlanBuilder::new(self.problem);
        builder.add_bin(self.bin_type, 1, orientation);
        tree.feed(0, &mut builder);
        let plan = builder.build();
        debug_assert!(
            plan.item_copies
                .iter()
                .zip(&self.problem.item_types)
                .all(|(&cut, item)| cut <= item.copies),
            "the plan cuts too many parts"
        );
        plan
    }
}

impl ChildByChildScheme for Scheme<'_> {
    type Node = NodeRef;
    type Priority = (bool, Reverse<Guide>, Reverse<Guide>, Reverse<u64>);
    type DominanceKey = CopiesPlaced;

    fn root(&self) -> NodeRef {
        Rc::new(MsNode {
            id: self.take_id(),
            state: self.root_state(),
            greedy_value: 0.0,
            insertions: RefCell::new(Vec::new()),
            next_child: Cell::new(0),
        })
    }

    /// upstream: `BranchingSchemeMaximalSpaces::next_child`.
    fn next_child(&self, parent: &NodeRef) -> Option<NodeRef> {
        let position = parent.next_child.get();
        if position == 0 {
            *parent.insertions.borrow_mut() = self.insertions(&parent.state);
        }
        parent.next_child.set(position + 1);
        let insertion = *parent.insertions.borrow().get(position)?;
        let mut state = parent.state.clone();
        let id = self.apply(&mut state, &insertion);
        let greedy_value = self.greedy_completion(&state).profit;
        Some(Rc::new(MsNode {
            id,
            state,
            greedy_value,
            insertions: RefCell::new(Vec::new()),
            next_child: Cell::new(0),
        }))
    }

    fn infertile(&self, node: &NodeRef) -> bool {
        let position = node.next_child.get();
        position > 0 && position >= node.insertions.borrow().len()
    }

    fn depth(&self, _node: &NodeRef) -> Option<usize> {
        None
    }

    fn is_leaf(&self, node: &NodeRef) -> bool {
        node.state.number_of_items == self.problem.number_of_items
    }

    /// upstream: `BranchingSchemeMaximalSpaces::operator()`: the root
    /// first, then by decreasing greedy value, value, and id.
    fn priority(&self, node: &NodeRef) -> Self::Priority {
        (
            node.state.number_of_blocks != 0,
            Reverse(Guide::new(node.greedy_value)),
            Reverse(Guide::new(node.state.profit)),
            Reverse(node.id),
        )
    }

    /// upstream: `BranchingSchemeMaximalSpaces::better`.
    fn is_better(&self, node: &NodeRef, other: &NodeRef) -> bool {
        node.state.profit > other.state.profit
    }

    /// upstream: `BranchingSchemeMaximalSpaces::bound`: nothing beats a
    /// plan that cuts every part.
    fn cannot_beat(&self, _node: &NodeRef, incumbent: &NodeRef) -> bool {
        self.is_leaf(incumbent)
    }

    fn dominance_key(&self, node: &NodeRef) -> Option<CopiesPlaced> {
        Some(CopiesPlaced(Rc::clone(node)))
    }

    fn dominates(&self, node: &NodeRef, other: &NodeRef) -> bool {
        node.greedy_value >= other.greedy_value
    }
}

/// A node of the cutting tree being rebuilt from placed blocks (upstream
/// `CutNode` in `to_solution`).
#[derive(Debug, Clone, Default)]
struct CutNode {
    depth: u32,
    item_type: Option<usize>,
    children: Vec<usize>,
}

/// Rebuilds the cutting tree of placed blocks.
///
/// upstream: `BranchingSchemeMaximalSpaces::to_solution`. Nodes only need
/// their depth, part and children: the plan builder recomputes every
/// position from the cut positions, which are the right or top ends.
struct CutTree<'s, 'a> {
    scheme: &'s Scheme<'a>,
    orientation: Orientation,
    nodes: Vec<CutNode>,
    /// The cut position of each node: its right end if it is cut at a
    /// fixed x, its top otherwise.
    ends: Vec<(i64, i64)>,
}

impl<'s, 'a> CutTree<'s, 'a> {
    fn new(scheme: &'s Scheme<'a>, orientation: Orientation) -> Self {
        let size = scheme.problem.bin_types[scheme.bin_type].size;
        CutTree {
            scheme,
            orientation,
            nodes: vec![CutNode::default()],
            ends: vec![(size.w, size.h)],
        }
    }

    /// Adds a child of `parent` at `depth` ending at (`right`, `top`).
    fn push(&mut self, parent: usize, depth: u32, right: i64, top: i64) -> usize {
        let index = self.nodes.len();
        self.nodes.push(CutNode {
            depth,
            item_type: None,
            children: Vec::new(),
        });
        self.ends.push((right, top));
        self.nodes[parent].children.push(index);
        index
    }

    fn vertical(&self, depth: u32) -> bool {
        cut_is_vertical(depth, self.orientation)
    }

    /// A grid of one part type filling the node `parent`, whose
    /// bottom-left corner is (`x`, `y`).
    ///
    /// upstream: `place_simple_block` in `to_solution`.
    fn place_grid(&mut self, parent: usize, block: &Block, depth: u32, x: i64, y: i64) {
        let BlockKind::Grid { item_type, rotate } = block.kind else {
            unreachable!("a grid block");
        };
        let size = self.scheme.problem.item_types[item_type].size;
        let cut_thickness = self.scheme.problem.cut_thickness;
        let (w, h) = (size.width(rotate), size.height(rotate));
        let cx = if w > 0 {
            (block.size.w + cut_thickness) / (w + cut_thickness)
        } else {
            1
        };
        let cy = if h > 0 {
            (block.size.h + cut_thickness) / (h + cut_thickness)
        } else {
            1
        };
        if cx == 1 && cy == 1 {
            self.nodes[parent].item_type = Some(item_type);
            return;
        }
        let vertical = self.vertical(depth);
        if cx == 1 {
            if vertical {
                let wrap = self.push(parent, depth + 1, x + w, y + block.size.h);
                self.place_grid(wrap, block, depth + 1, x, y);
                return;
            }
            for row in 0..cy {
                let bottom = y + row * (h + cut_thickness);
                let cell = self.push(parent, depth + 1, x + w, bottom + h);
                self.nodes[cell].item_type = Some(item_type);
            }
            return;
        }
        if cy == 1 {
            if !vertical {
                let wrap = self.push(parent, depth + 1, x + block.size.w, y + h);
                self.place_grid(wrap, block, depth + 1, x, y);
                return;
            }
            for column in 0..cx {
                let left = x + column * (w + cut_thickness);
                let cell = self.push(parent, depth + 1, left + w, y + h);
                self.nodes[cell].item_type = Some(item_type);
            }
            return;
        }
        if vertical {
            for column in 0..cx {
                let left = x + column * (w + cut_thickness);
                let strip = self.push(parent, depth + 1, left + w, y + block.size.h);
                for row in 0..cy {
                    let bottom = y + row * (h + cut_thickness);
                    let cell = self.push(strip, depth + 2, left + w, bottom + h);
                    self.nodes[cell].item_type = Some(item_type);
                }
            }
        } else {
            for row in 0..cy {
                let bottom = y + row * (h + cut_thickness);
                let strip = self.push(parent, depth + 1, x + block.size.w, bottom + h);
                for column in 0..cx {
                    let left = x + column * (w + cut_thickness);
                    let cell = self.push(strip, depth + 2, left + w, bottom + h);
                    self.nodes[cell].item_type = Some(item_type);
                }
            }
        }
    }

    /// A block in the region of `parent` of size `available`, whose
    /// bottom-left corner is (`x`, `y`): cuts off the waste right of and
    /// above the block, then splits pairs into their halves.
    ///
    /// upstream: `place_block` in `to_solution`.
    fn place_block(
        &mut self,
        parent: usize,
        block_index: usize,
        depth: u32,
        x: i64,
        y: i64,
        available: Size,
    ) {
        let block = &self.scheme.blocks[block_index];
        let cut_thickness = self.scheme.problem.cut_thickness;
        let vertical = self.vertical(depth);

        if available.w > block.size.w {
            if !vertical {
                let wrap = self.push(parent, depth + 1, x + available.w, y + available.h);
                self.place_block(wrap, block_index, depth + 1, x, y, available);
                return;
            }
            let left = self.push(parent, depth + 1, x + block.size.w, y + available.h);
            if available.w > block.size.w + cut_thickness {
                self.push(parent, depth + 1, x + available.w, y + available.h);
            }
            let size = Size {
                w: block.size.w,
                h: available.h,
            };
            self.place_block(left, block_index, depth + 1, x, y, size);
            return;
        }
        if available.h > block.size.h {
            if vertical {
                let wrap = self.push(parent, depth + 1, x + available.w, y + available.h);
                self.place_block(wrap, block_index, depth + 1, x, y, available);
                return;
            }
            let bottom = self.push(parent, depth + 1, x + available.w, y + block.size.h);
            if available.h > block.size.h + cut_thickness {
                self.push(parent, depth + 1, x + available.w, y + available.h);
            }
            let size = Size {
                w: available.w,
                h: block.size.h,
            };
            self.place_block(bottom, block_index, depth + 1, x, y, size);
            return;
        }

        let (pair_vertical, first, second) = match block.kind {
            BlockKind::Grid { .. } => {
                self.place_grid(parent, block, depth, x, y);
                return;
            }
            BlockKind::Pair {
                vertical,
                first,
                second,
            } => (vertical, first, second),
        };
        if pair_vertical != vertical {
            let wrap = self.push(parent, depth + 1, x + block.size.w, y + block.size.h);
            self.place_block(wrap, block_index, depth + 1, x, y, block.size);
            return;
        }
        let blocks = &self.scheme.blocks;
        let (first_size, second_size, size) = (blocks[first].size, blocks[second].size, block.size);
        if pair_vertical {
            let cut = x + first_size.w;
            let left = self.push(parent, depth + 1, cut, y + size.h);
            let right = self.push(parent, depth + 1, x + size.w, y + size.h);
            let first_available = Size {
                w: first_size.w,
                h: size.h,
            };
            let second_available = Size {
                w: second_size.w,
                h: size.h,
            };
            self.place_block(left, first, depth + 1, x, y, first_available);
            self.place_block(
                right,
                second,
                depth + 1,
                cut + cut_thickness,
                y,
                second_available,
            );
        } else {
            let cut = y + first_size.h;
            let bottom = self.push(parent, depth + 1, x + size.w, cut);
            let top = self.push(parent, depth + 1, x + size.w, y + size.h);
            let first_available = Size {
                w: size.w,
                h: first_size.h,
            };
            let second_available = Size {
                w: size.w,
                h: second_size.h,
            };
            self.place_block(bottom, first, depth + 1, x, y, first_available);
            self.place_block(
                top,
                second,
                depth + 1,
                x,
                cut + cut_thickness,
                second_available,
            );
        }
    }

    /// Replays the placements in order, choosing each block's space as the
    /// search did.
    ///
    /// upstream: the replay loop of `to_solution`.
    fn replay(&mut self, placed_blocks: &[PlacedBlock]) {
        let scheme = self.scheme;
        let cut_thickness = scheme.problem.cut_thickness;
        let size = scheme.problem.bin_types[scheme.bin_type].size;
        // The empty spaces, with the tree node of each.
        let mut spaces: Vec<(EmptySpace, usize)> = vec![(
            EmptySpace {
                x: 0,
                y: 0,
                width: size.w,
                height: size.h,
                depth: 0,
                orientation: scheme.first_cut_orientation,
            },
            0,
        )];
        let mut state = scheme.root_state();

        for placed in placed_blocks {
            let just_spaces: Vec<EmptySpace> = spaces.iter().map(|(space, _)| *space).collect();
            let best = Scheme::best_space(&just_spaces).expect("a space is left");
            let (space, node) = spaces[best];
            let block = &scheme.blocks[placed.block];
            let (w, h) = (block.size.w, block.size.h);
            let depth = space.depth;
            let orientation = placed.orientation;
            let (xs, ys) = (space.x, space.y);
            let (xe, ye) = (space.x + space.width, space.y + space.height);

            let mut new_spaces = Vec::new();
            let block_area;
            if cut_is_vertical(depth, orientation) {
                let strip = self.push(node, depth + 1, xs + w, ye);
                if xe > xs + w + cut_thickness {
                    let right = self.push(node, depth + 1, xe, ye);
                    let left = xs + w + cut_thickness;
                    new_spaces.push((
                        EmptySpace {
                            x: left,
                            y: ys,
                            width: xe - left,
                            height: space.height,
                            depth: depth + 1,
                            orientation,
                        },
                        right,
                    ));
                }
                block_area = self.push(strip, depth + 2, xs + w, ys + h);
                if ye > ys + h + cut_thickness {
                    let above = self.push(strip, depth + 2, xs + w, ye);
                    let bottom = ys + h + cut_thickness;
                    new_spaces.push((
                        EmptySpace {
                            x: xs,
                            y: bottom,
                            width: w,
                            height: ye - bottom,
                            depth: depth + 2,
                            orientation,
                        },
                        above,
                    ));
                }
            } else {
                let strip = self.push(node, depth + 1, xe, ys + h);
                if ye > ys + h + cut_thickness {
                    let above = self.push(node, depth + 1, xe, ye);
                    let bottom = ys + h + cut_thickness;
                    new_spaces.push((
                        EmptySpace {
                            x: xs,
                            y: bottom,
                            width: space.width,
                            height: ye - bottom,
                            depth: depth + 1,
                            orientation,
                        },
                        above,
                    ));
                }
                block_area = self.push(strip, depth + 2, xs + w, ys + h);
                if xe > xs + w + cut_thickness {
                    let right = self.push(strip, depth + 2, xe, ys + h);
                    let left = xs + w + cut_thickness;
                    new_spaces.push((
                        EmptySpace {
                            x: left,
                            y: ys,
                            width: xe - left,
                            height: h,
                            depth: depth + 2,
                            orientation,
                        },
                        right,
                    ));
                }
            }

            self.place_block(
                block_area,
                placed.block,
                depth + 2,
                placed.x,
                placed.y,
                block.size,
            );

            spaces.remove(best);
            spaces.extend(new_spaces);

            // Mirror the search's pruning, so the same spaces are chosen.
            for &(item_type, copies) in &block.item_copies {
                state.copies[item_type] += copies;
            }
            scheme.prune_blocks(&mut state);
            let mut index = 0;
            while index < spaces.len() {
                let space = spaces[index].0;
                let fits = state
                    .valid_blocks
                    .iter()
                    .any(|&block_index| Scheme::fits(&scheme.blocks[block_index], &space));
                if fits {
                    index += 1;
                } else {
                    spaces.swap_remove(index);
                }
            }
        }
    }

    /// Feeds the subtree of `index` to the plan builder, depth first.
    ///
    /// upstream: the `dfs` of `to_solution`.
    fn feed(&self, index: usize, builder: &mut PlanBuilder) {
        let node = &self.nodes[index];
        if node.depth > 0 {
            let vertical = (self.orientation == Orientation::Vertical) == (node.depth % 2 == 1);
            let (right, top) = self.ends[index];
            builder.add_node(node.depth, if vertical { right } else { top });
            if let Some(item_type) = node.item_type {
                builder.set_last_node_item(item_type);
                return;
            }
        }
        for &child in &node.children {
            self.feed(child, builder);
        }
    }
}

impl Scheme<'_> {
    /// The block half of [`Scheme::prune`].
    fn prune_blocks(&self, state: &mut State) {
        let item_types = &self.problem.item_types;
        let copies = &state.copies;
        state.valid_blocks.retain(|&block_index| {
            self.blocks[block_index]
                .item_copies
                .iter()
                .all(|&(item_type, count)| {
                    copies[item_type] + count <= item_types[item_type].copies
                })
        });
    }
}

/// The maximal-spaces search on a one-panel problem, at `queue_size`.
///
/// upstream: tree_search_maximal_spaces.cpp, `tree_search_maximal_spaces`
/// in not-anytime-sequential mode: the knapsack and feasibility objectives
/// search directly; bin packing and the open dimensions go through
/// [`sequential_feasibility`]. `on_plan` sees each better plan as it is
/// found, so that the caller can stop the search once one reaches a bound.
pub(crate) fn tree_search_maximal_spaces(
    problem: &Problem,
    queue_size: u64,
    stop: &Stop,
    on_plan: &mut dyn FnMut(&Plan),
) -> Plan {
    debug_assert!(problem.number_of_bins <= 1, "one panel only");
    match problem.objective {
        Objective::Knapsack | Objective::Feasibility => search(problem, queue_size, stop, on_plan),
        Objective::BinPacking | Objective::OpenDimensionX | Objective::OpenDimensionY => {
            sequential_feasibility(
                problem,
                stop,
                |sub, stop| search(sub, queue_size, stop, &mut |_| {}),
                on_plan,
            )
        }
    }
}

/// The beam search itself. `on_plan` sees each better plan.
fn search(problem: &Problem, queue_size: u64, stop: &Stop, on_plan: &mut dyn FnMut(&Plan)) -> Plan {
    let scheme = Scheme::new(problem);
    // upstream: the algorithm's own end flag, set once its best plan is
    // proven optimal: for feasibility, once it cuts every part.
    let done = Arc::new(AtomicBool::new(false));
    let parameters = Parameters {
        minimum_queue_size: 1,
        growth_factor: queue_size as f64,
        maximum_queue_size: queue_size,
        stop: stop.with_flag(&done),
        ..Parameters::default()
    };
    let mut best = Plan::empty(problem);
    child_by_child_beam_search(&scheme, &parameters, |output| {
        let plan = scheme.to_plan(&output.best);
        if plan.is_better_than(&best, problem.objective) {
            best = plan;
            on_plan(&best);
            if problem.objective == Objective::Feasibility && best.feasible {
                done.store(true, Ordering::Relaxed);
            }
        }
    });
    best
}

/// Bin packing on one panel and the open dimensions: "cut everything"
/// subproblems on ever shorter panels (or fewer panels), keeping the last
/// that succeeds.
///
/// upstream: src/rectangleguillotine/sequential_feasibility.cpp,
/// `sequential_feasibility`, without the leftover objective.
pub(crate) fn sequential_feasibility(
    problem: &Problem,
    stop: &Stop,
    mut solve: impl FnMut(&Problem, &Stop) -> Plan,
    on_plan: &mut dyn FnMut(&Plan),
) -> Plan {
    let bin_type = problem.bin_type_of_position(0);
    let bin = &problem.bin_types[bin_type];
    let item_area = problem.item_area;

    let mut number_of_bins = 0;
    let mut current_x = 0;
    let mut current_y = 0;
    match problem.objective {
        Objective::BinPacking => {
            // Enough panels for twice the parts' area.
            let mut total = 0;
            for bin in &problem.bin_types {
                let area = bin.area();
                let needed = (2 * item_area - total + area - 1) / area;
                let used = needed.min(i64::from(bin.copies));
                total += used * area;
                number_of_bins += used as u32;
                if total >= 2 * item_area {
                    break;
                }
            }
        }
        Objective::OpenDimensionX => {
            current_x = ((2 * item_area + bin.size.h - 1) / bin.size.h).min(bin.size.w);
        }
        Objective::OpenDimensionY => {
            current_y = ((2 * item_area + bin.size.w - 1) / bin.size.w).min(bin.size.h);
        }
        Objective::Knapsack | Objective::Feasibility => unreachable!("searched directly"),
    }

    let mut best = Plan::empty(problem);
    loop {
        if stop.requested() {
            break;
        }

        // The "cut everything" subproblem.
        let mut bin_types = Vec::new();
        let mut original_bin_types = Vec::new();
        match problem.objective {
            Objective::BinPacking => {
                let mut remaining = number_of_bins;
                for (index, bin) in problem.bin_types.iter().enumerate() {
                    if remaining == 0 {
                        break;
                    }
                    let copies = bin.copies.min(remaining);
                    bin_types.push(BinType {
                        size: bin.size,
                        copies,
                        cost: bin.cost,
                        trims: bin.trims,
                    });
                    original_bin_types.push(index);
                    remaining -= copies;
                }
            }
            Objective::OpenDimensionX => bin_types.push(BinType {
                size: Size {
                    w: current_x,
                    h: bin.size.h,
                },
                copies: 1,
                cost: bin.cost,
                trims: bin.trims,
            }),
            _ => bin_types.push(BinType {
                size: Size {
                    w: bin.size.w,
                    h: current_y,
                },
                copies: 1,
                cost: bin.cost,
                trims: bin.trims,
            }),
        }
        let sub = Problem::from_types(
            Objective::Feasibility,
            problem.settings(),
            problem.item_types.clone(),
            bin_types,
        );

        let sub_plan = solve(&sub, stop);
        if !sub_plan.feasible {
            break;
        }

        let mut plan = Plan::empty(problem);
        if problem.objective == Objective::BinPacking {
            plan.append_bins(problem, &sub_plan, Some(&original_bin_types));
        } else {
            plan.append_bin(problem, &sub_plan, 0, 1, Some(&[bin_type]), None);
        }
        let used_bins = sub_plan.number_of_bins;
        let (used_width, used_height) = (sub_plan.width, sub_plan.height);
        if plan.is_better_than(&best, problem.objective) {
            best = plan;
            on_plan(&best);
        }

        match problem.objective {
            Objective::BinPacking => {
                number_of_bins = used_bins - 1;
                if number_of_bins == 0 {
                    break;
                }
            }
            Objective::OpenDimensionX => {
                current_x = ((0.99 * used_width as f64) as i64).min(used_width - 1);
                if current_x <= 0 {
                    break;
                }
            }
            _ => {
                current_y = ((0.99 * used_height as f64) as i64).min(used_height - 1);
                if current_y <= 0 {
                    break;
                }
            }
        }
    }
    best
}
