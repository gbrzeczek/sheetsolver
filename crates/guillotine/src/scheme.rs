//! The guillotine branching scheme: how a partial cutting plan grows, one
//! part at a time.
//!
//! upstream: src/rectangleguillotine/tree_search.hpp and tree_search.cpp,
//! `BranchingScheme`.
//!
//! A node is a partial plan built by placing parts left to right, bottom to
//! top, in a 3-level structure of nested strips:
//!
//! - level 1: strips across the whole panel, between 1-cuts at x
//!   positions (`x1_prev` .. `x1_curr` for the current one);
//! - level 2: pieces of the current level-1 strip, between 2-cuts at y
//!   positions (`y2_prev` .. `y2_curr`);
//! - level 3: pieces of the current level-2 piece, between 3-cuts at x
//!   positions (ending at `x3_curr`), each holding one part at its bottom
//!   and, if needed, an offcut above it (a 4th, trimming cut).
//!
//! Each child places one part in a new level-3 piece, at one of these
//! "depths" (upstream `df`): 2 = in the current level-2 piece, 1 = in a new
//! level-2 piece of the current level-1 strip, 0 = in a new level-1 strip,
//! negative = on a new panel (-1: vertical first cuts, -2: horizontal). The
//! current strips grow when a placed part needs more room.
//!
//! Horizontal first cuts are searched on the flipped instance (every size
//! turned), so the code always cuts vertically first. Two stages are three
//! levels where every level-1 strip spans the whole panel: the level-2
//! cuts are then the real first cuts, so first cuts in one direction are
//! searched with level-1 strips in the other.
//!
//! Upstream also handles trims, defects, minimum waste, cut distances, cut
//! counts, cutting costs, resources, stacks and two parts per level-3
//! piece (roadef2018). None of them is In (`docs/SCOPE.md`), and with their
//! neutral values most of upstream's `update()` reduces to the few rules
//! kept in [`Scheme::update`]; the comments there say what was dropped.

use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use model::CutType;
use treesearch::{BranchingScheme, Guide};

use crate::problem::{Objective, Orientation, Problem};

/// Which guide orders the nodes in a queue.
///
/// upstream: tree_search.hpp, `BranchingScheme::operator()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuideKind {
    /// Upstream guide 0: used area per part area.
    AreaPerItemArea,
    /// Upstream guide 1: waste rate per average part area.
    WasteRatePerAverageItemArea,
    /// Upstream guide 4: used area per value.
    AreaPerProfit,
    /// Upstream guide 5: used area per value per average part area.
    AreaPerProfitPerAverageItemArea,
}

/// A node of the search tree: a partial cutting plan.
///
/// Parts are placed one per node; the plan is the chain of nodes from the
/// root (see [`crate::plan`]).
#[derive(Debug)]
pub(crate) struct Node {
    /// Creation order, for tie-breaking (upstream `id`).
    pub id: u64,
    /// The node this one extends; `None` for the root.
    pub parent: Option<Rc<Node>>,
    /// The part type placed by this node; `None` for the root.
    pub item_type: Option<usize>,
    /// Where the part was placed (upstream `df`, see the module doc).
    pub depth: i32,
    /// Right end of the current level-1 strip.
    pub x1_curr: i64,
    /// Left end of the current level-1 strip.
    pub x1_prev: i64,
    /// Top of the current level-2 piece.
    pub y2_curr: i64,
    /// Bottom of the current level-2 piece.
    pub y2_prev: i64,
    /// Right end of the last level-3 piece.
    pub x3_curr: i64,
    /// Copies of each part type placed so far (upstream `pos_stack`).
    pub copies_placed: Vec<u32>,
    /// Panels used, the current one included.
    pub number_of_bins: u32,
    /// First-cut direction of the current panel.
    pub first_stage_orientation: Orientation,
    /// Parts placed.
    pub number_of_items: u32,
    /// Total area of the parts placed.
    pub item_area: i64,
    /// Area used so far: the closed panels and the part of the current one
    /// left of and below the current strips.
    pub current_area: i64,
    /// `current_area` minus `item_area`.
    pub waste: i64,
    /// Total value of the parts placed.
    pub profit: f64,
    /// Queue order, computed once.
    guide: Guide,
}

/// A handle on a node, as the search stores it.
pub(crate) type NodeRef = Rc<Node>;

/// The frontier of a partial plan on its current panel, used to compare
/// plans for dominance (upstream `Front`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Front {
    /// Index of the current panel.
    bin: u32,
    orientation: Orientation,
    x1_prev: i64,
    x3_curr: i64,
    x1_curr: i64,
    y2_prev: i64,
    y2_curr: i64,
}

/// A candidate child before it is built (upstream `Insertion`, restricted
/// to one part).
#[derive(Debug, Clone, Copy)]
struct Insertion {
    item_type: usize,
    depth: i32,
    x1: i64,
    y2: i64,
    x3: i64,
}

/// Nodes with the same copies placed may dominate each other (upstream
/// `NodeHasher`). Holds the node so the key costs no copy.
#[derive(Debug, Clone)]
pub(crate) struct CopiesPlaced(NodeRef);

impl PartialEq for CopiesPlaced {
    fn eq(&self, other: &Self) -> bool {
        self.0.copies_placed == other.0.copies_placed
    }
}

impl Eq for CopiesPlaced {}

impl Hash for CopiesPlaced {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.copies_placed.hash(state);
    }
}

/// The guillotine branching scheme for one guide and one first-cut
/// direction.
pub(crate) struct Scheme<'a> {
    problem: &'a Problem,
    guide: GuideKind,
    /// The first-cut direction of the level-1 strips: the direction asked
    /// for with 3 stages, the other one with 2 (see the module doc), or
    /// `Any` to try both on every panel.
    ///
    /// upstream: `first_stage_orientation_`.
    first_stage_orientation: Orientation,
    /// The panel type of each panel, in the order the search uses them.
    bin_type_ids: Vec<usize>,
    /// Total area of the panels before each one.
    previous_bins_area: Vec<i64>,
    /// For each part type, the previous part type that is identical to it,
    /// if any. Its copies are placed first, so identical part types are
    /// not explored in every order (upstream `stack_pred_`).
    identical_predecessor: Vec<Option<usize>>,
    /// Numbers the nodes in creation order (upstream `node_id_`).
    next_id: Cell<u64>,
}

impl<'a> Scheme<'a> {
    /// upstream: tree_search.cpp, `BranchingScheme::BranchingScheme`.
    pub fn new(problem: &'a Problem, guide: GuideKind, orientation: Orientation) -> Self {
        // The knapsack objective may use any panels, so it tries the
        // smallest ones first; the others use panels in the order listed.
        let mut bin_type_order: Vec<usize> = (0..problem.bin_types.len()).collect();
        if problem.objective == Objective::Knapsack {
            // A stable sort, like upstream's.
            bin_type_order.sort_by_key(|&bin_type| problem.bin_types[bin_type].size.area());
        }
        let mut bin_type_ids = Vec::new();
        let mut previous_bins_area = Vec::new();
        let mut area = 0;
        for bin_type in bin_type_order {
            let bin = &problem.bin_types[bin_type];
            for _ in 0..bin.copies {
                bin_type_ids.push(bin_type);
                previous_bins_area.push(area);
                area += bin.size.area();
            }
        }

        let orientation = if problem.first_stage_orientation != Orientation::Any {
            problem.first_stage_orientation
        } else {
            orientation
        };
        let first_stage_orientation = match (problem.number_of_stages, orientation) {
            (2, Orientation::Vertical) => Orientation::Horizontal,
            (2, Orientation::Horizontal) => Orientation::Vertical,
            (_, orientation) => orientation,
        };

        let identical_predecessor = (0..problem.item_types.len())
            .map(|item_type| {
                (0..item_type)
                    .rev()
                    .find(|&previous| identical_item_types(problem, item_type, previous))
            })
            .collect();

        Scheme {
            problem,
            guide,
            first_stage_orientation,
            bin_type_ids,
            previous_bins_area,
            identical_predecessor,
            next_id: Cell::new(0),
        }
    }

    fn new_id(&self) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }

    /// Whether every part has been placed.
    fn full(&self, node: &Node) -> bool {
        node.number_of_items == self.problem.number_of_items
    }

    /// The used length of the last panel, for "Shortest length from one
    /// panel" (upstream `width`). Upstream's `height` is the same.
    fn width(&self, node: &Node) -> i64 {
        if self.problem.number_of_stages == 3 {
            node.x1_curr
        } else {
            node.y2_curr
        }
    }

    /// Upper bound on the value reachable from `node` (upstream `ubkp`).
    fn knapsack_upper_bound(&self, node: &Node) -> f64 {
        let remaining_item_area = self.problem.item_area - node.item_area;
        let remaining_packable_area = self.problem.bin_area - node.current_area;
        if remaining_packable_area >= remaining_item_area {
            self.problem.item_profit
        } else {
            let best = &self.problem.item_types[self.problem.largest_efficiency_item_type];
            let efficiency = best.profit / best.size.area() as f64;
            node.profit + remaining_packable_area as f64 * efficiency
        }
    }

    /// upstream: `BranchingScheme::operator()`, the guide part.
    fn guide_value(&self, node: &Node) -> f64 {
        let current_area = node.current_area as f64;
        let item_area = node.item_area as f64;
        let number_of_items = f64::from(node.number_of_items);
        match self.guide {
            GuideKind::AreaPerItemArea => current_area / item_area,
            GuideKind::WasteRatePerAverageItemArea => {
                let mut waste_rate = (node.current_area - node.item_area) as f64 / current_area;
                if waste_rate < 0.02 {
                    waste_rate = 0.01 + waste_rate / 2.0;
                }
                waste_rate / item_area * number_of_items
            }
            GuideKind::AreaPerProfit => current_area / node.profit,
            GuideKind::AreaPerProfitPerAverageItemArea => {
                current_area / node.profit / item_area * number_of_items
            }
        }
    }

    // Where a part placed at `depth` starts. Negative depths open a new
    // panel, whose strips start at 0 (no trims).

    /// Left end of the new level-3 piece (upstream `x3_prev(node, df)`).
    pub fn x3_prev(&self, node: &Node, depth: i32) -> i64 {
        match depth {
            0 => node.x1_curr + self.problem.cut_thickness,
            1 => node.x1_prev,
            2 => node.x3_curr + self.problem.cut_thickness,
            _ => 0,
        }
    }

    /// Bottom of the level-2 piece (upstream `y2_prev(node, df)`).
    fn y2_prev(&self, node: &Node, depth: i32) -> i64 {
        match depth {
            1 => node.y2_curr + self.problem.cut_thickness,
            2 => node.y2_prev,
            _ => 0,
        }
    }

    /// Index of the panel a part placed at `depth` goes on (upstream
    /// `last_bin`).
    fn last_bin(&self, node: &Node, depth: i32) -> u32 {
        if depth >= 0 {
            node.number_of_bins - 1
        } else {
            node.number_of_bins + (-depth - 1) as u32 / 2
        }
    }

    /// First-cut direction of the panel a part placed at `depth` goes on
    /// (upstream `last_bin_orientation`).
    fn last_bin_orientation(&self, node: &Node, depth: i32) -> Orientation {
        if depth >= 0 {
            node.first_stage_orientation
        } else if depth % 2 == 0 {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        }
    }

    /// Height of panel `bin` seen in `orientation`.
    fn bin_height(&self, bin: u32, orientation: Orientation) -> i64 {
        self.problem
            .bin_size(self.bin_type_ids[bin as usize], orientation)
            .h
    }

    /// The node's frontier (upstream `front(node)`).
    fn front(&self, node: &Node) -> Front {
        Front {
            bin: node.number_of_bins - 1,
            orientation: node.first_stage_orientation,
            x1_prev: node.x1_prev,
            x3_curr: node.x3_curr,
            x1_curr: node.x1_curr,
            y2_prev: node.y2_prev,
            y2_curr: node.y2_curr,
        }
    }

    /// The frontier of the child `insertion` would create (upstream
    /// `front(node, insertion)`). Unlike a node's own frontier, the
    /// previous cut positions do not include the kerf; upstream does the
    /// same.
    fn insertion_front(&self, node: &Node, insertion: &Insertion) -> Front {
        let (x1_prev, y2_prev) = match insertion.depth {
            0 => (node.x1_curr, 0),
            1 => (node.x1_prev, node.y2_curr),
            2 => (node.x1_prev, node.y2_prev),
            _ => (0, 0),
        };
        Front {
            bin: self.last_bin(node, insertion.depth),
            orientation: self.last_bin_orientation(node, insertion.depth),
            x1_prev,
            x3_curr: insertion.x3,
            x1_curr: insertion.x1,
            y2_prev,
            y2_curr: insertion.y2,
        }
    }

    /// Whether every completion of a plan with frontier `f2` can also
    /// complete a plan with frontier `f1`.
    ///
    /// upstream: tree_search.hpp, `BranchingScheme::dominates(Front, Front)`.
    fn front_dominates(&self, f1: &Front, f2: &Front) -> bool {
        if f1.bin < f2.bin {
            return true;
        }
        if f1.bin == f2.bin && f1.orientation == f2.orientation {
            if f1.x1_curr <= f2.x1_prev {
                return true;
            }
            if f1.x1_prev <= f2.x1_prev && f1.x1_curr <= f2.x1_curr && f1.y2_curr <= f2.y2_prev {
                return true;
            }
            let h = self.bin_height(f1.bin, f1.orientation);
            let current_strip_within = f1.x3_curr <= f2.x3_curr
                && f1.x1_curr <= f2.x1_curr
                && f1.y2_prev <= f2.y2_prev
                && f1.y2_curr <= f2.y2_curr;
            if f1.y2_curr != h && f1.x1_prev <= f2.x1_prev && current_strip_within {
                return true;
            }
            if f2.y2_curr == h && f1.x1_prev >= f2.x1_prev && current_strip_within {
                return true;
            }
            if f1.y2_curr != h && f2.y2_curr == h && current_strip_within {
                return true;
            }
        }
        false
    }

    /// Adds the children placing one copy of `item_type`, turned or not, at
    /// `depth`.
    ///
    /// upstream: tree_search.cpp, `BranchingScheme::insertion_1_item`.
    fn insert_item(
        &self,
        children: &mut Vec<NodeRef>,
        parent: &NodeRef,
        item_type: usize,
        rotate: bool,
        depth: i32,
    ) {
        let orientation = self.last_bin_orientation(parent, depth);
        let bin = self.last_bin(parent, depth);
        let item = self.problem.item_size(item_type, orientation);
        let panel = self
            .problem
            .bin_size(self.bin_type_ids[bin as usize], orientation);
        let x = self.x3_prev(parent, depth) + item.width(rotate);
        let y = self.y2_prev(parent, depth) + item.height(rotate);
        if x > panel.w || y > panel.h {
            return;
        }
        // One part type per last-stage strip.
        if depth == 2
            && self.problem.cut_type == CutType::Homogenous
            && parent.item_type != Some(item_type)
        {
            return;
        }
        let insertion = Insertion {
            item_type,
            depth,
            x1: x,
            y2: y,
            x3: x,
        };
        self.update(children, parent, insertion);
    }

    /// Fits the current strips around the new part, then adds the child
    /// unless an existing child dominates it, removing the children it
    /// dominates.
    ///
    /// upstream: tree_search.cpp, `BranchingScheme::update`. With no
    /// minimum waste (upstream's `z1`/`z2` are then 1, or 2 for a level-2
    /// piece whose height is fixed), no trims, defects, cut distances or
    /// cut counts, only the steps below change anything; upstream's other
    /// checks cannot fail.
    fn update(&self, children: &mut Vec<NodeRef>, parent: &NodeRef, mut insertion: Insertion) {
        let orientation = self.last_bin_orientation(parent, insertion.depth);
        let bin = self.last_bin(parent, insertion.depth);
        let panel = self
            .problem
            .bin_size(self.bin_type_ids[bin as usize], orientation);

        // With 2 stages, every level-1 strip spans the panel.
        if self.problem.number_of_stages == 2 {
            insertion.x1 = panel.w;
        }

        // A part in the current level-1 strip widens it, never narrows it.
        if insertion.depth >= 1 && insertion.x1 <= parent.x1_curr {
            insertion.x1 = parent.x1_curr;
        }

        // A part in the current level-2 piece: with trimming allowed, the
        // piece grows to fit it. Without (exact and homogenous cuts), parts
        // fill their piece exactly, so the part must have its height.
        if insertion.depth == 2 {
            if self.problem.cut_type == CutType::NonExact {
                if insertion.y2 <= parent.y2_curr {
                    insertion.y2 = parent.y2_curr;
                }
            } else if insertion.y2 != parent.y2_curr {
                return;
            }
        }

        debug_assert!(insertion.x1 <= panel.w && insertion.y2 <= panel.h);

        // Dominance against the children found so far, of the same part
        // type.
        let front = self.insertion_front(parent, &insertion);
        let mut index = 0;
        while index < children.len() {
            let child = &children[index];
            if child.item_type == Some(insertion.item_type) {
                let child_front = self.front(child);
                if self.front_dominates(&child_front, &front) {
                    return;
                }
                if self.front_dominates(&front, &child_front) {
                    // upstream: the last child takes its place, and is
                    // checked next.
                    children.swap_remove(index);
                    continue;
                }
            }
            index += 1;
        }

        children.push(Rc::new(self.child(parent, &insertion)));
    }

    /// Builds the child of `parent` for `insertion`.
    ///
    /// upstream: tree_search.cpp, `BranchingScheme::child_tmp`, without the
    /// cut counters and costs, which only serve out-of-scope features.
    fn child(&self, parent: &NodeRef, insertion: &Insertion) -> Node {
        let problem = self.problem;
        let depth = insertion.depth;

        let (number_of_bins, first_stage_orientation) = if depth < 0 {
            (
                parent.number_of_bins + (-depth - 1) as u32 / 2 + 1,
                self.last_bin_orientation(parent, depth),
            )
        } else {
            (parent.number_of_bins, parent.first_stage_orientation)
        };
        let bin = (number_of_bins - 1) as usize;
        let panel = problem.bin_size(self.bin_type_ids[bin], first_stage_orientation);

        let (x1_prev, y2_prev) = match depth {
            0 => (parent.x1_curr + problem.cut_thickness, 0),
            1 => (parent.x1_prev, parent.y2_curr + problem.cut_thickness),
            2 => (parent.x1_prev, parent.y2_prev),
            _ => (0, 0),
        };

        let item = &problem.item_types[insertion.item_type];
        let mut copies_placed = parent.copies_placed.clone();
        copies_placed[insertion.item_type] += 1;
        let number_of_items = parent.number_of_items + 1;
        let item_area = parent.item_area + item.size.area();
        let profit = parent.profit + item.profit;

        let (x1_curr, y2_curr, x3_curr) = (insertion.x1, insertion.y2, insertion.x3);
        // Once every part is placed, the last panel is used up to its last
        // cut, kerf included.
        let mut current_area = self.previous_bins_area[bin];
        if number_of_items == problem.number_of_items {
            current_area += if problem.number_of_stages >= 3 {
                (x1_curr + problem.cut_thickness).min(panel.w) * panel.h
            } else {
                (y2_curr + problem.cut_thickness).min(panel.h) * panel.w
            };
        } else {
            current_area += x1_prev * panel.h
                + (x1_curr.min(panel.w) - x1_prev) * y2_prev
                + (x3_curr.min(panel.w) - x1_prev) * (y2_curr.min(panel.h) - y2_prev);
        }
        let waste = current_area - item_area;
        debug_assert!(waste >= 0, "negative waste");

        let mut node = Node {
            id: self.new_id(),
            parent: Some(Rc::clone(parent)),
            item_type: Some(insertion.item_type),
            depth,
            x1_curr,
            x1_prev,
            y2_curr,
            y2_prev,
            x3_curr,
            copies_placed,
            number_of_bins,
            first_stage_orientation,
            number_of_items,
            item_area,
            current_area,
            waste,
            profit,
            guide: Guide::new(0.0),
        };
        node.guide = Guide::new(self.guide_value(&node));
        node
    }

    /// upstream: tree_search.cpp, `BranchingScheme::children`.
    fn expand(&self, parent: &NodeRef) -> Vec<NodeRef> {
        let mut children: Vec<NodeRef> = Vec::new();
        let is_root = parent.parent.is_none();
        // The root's only children open the first panel.
        let max_depth = if is_root { -1 } else { 2 };
        // Part sizes are read in the level-1 strips' direction to decide
        // which way to try first; with `Any` upstream reads them flipped.
        let view = if self.first_stage_orientation == Orientation::Vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };

        let mut depth = max_depth;
        loop {
            let bin = self.last_bin(parent, depth);
            let orientation = self.last_bin_orientation(parent, depth);
            if self.first_stage_orientation != Orientation::Any
                && orientation != self.first_stage_orientation
            {
                depth -= 1;
                continue;
            }
            if bin >= self.problem.number_of_bins {
                break;
            }

            // A deeper insertion that keeps the current strips beats
            // opening new ones, and one panel is enough.
            let current_strips_kept = children.iter().any(|child| match depth {
                1 => child.x1_curr == parent.x1_curr && child.y2_curr == parent.y2_curr,
                0 => child.x1_curr == parent.x1_curr,
                _ => depth < 0,
            });
            if current_strips_kept {
                break;
            }

            for (item_type, item) in self.problem.item_types.iter().enumerate() {
                if parent.copies_placed[item_type] == item.copies {
                    continue;
                }
                if let Some(previous) = self.identical_predecessor[item_type] {
                    if parent.copies_placed[previous] <= parent.copies_placed[item_type] {
                        continue;
                    }
                }
                if item.oriented {
                    self.insert_item(&mut children, parent, item_type, false, depth);
                } else {
                    let size = self.problem.item_size(item_type, view);
                    let wider = size.w > size.h;
                    self.insert_item(&mut children, parent, item_type, !wider, depth);
                    self.insert_item(&mut children, parent, item_type, wider, depth);
                }
            }
            depth -= 1;
        }

        if !is_root {
            // Drop children that open new strips when another child fits in
            // the current ones.
            let same_bin = |child: &NodeRef| child.number_of_bins == parent.number_of_bins;
            let has_depth_0 = children.iter().any(same_bin);
            let has_depth_1 = children
                .iter()
                .any(|child| same_bin(child) && child.x1_curr == parent.x1_curr);
            let has_depth_2 = children.iter().any(|child| {
                same_bin(child)
                    && child.x1_curr == parent.x1_curr
                    && child.y2_curr == parent.y2_curr
            });
            let mut index = 0;
            while index < children.len() {
                let child = &children[index];
                let new_bin = child.number_of_bins > parent.number_of_bins;
                let dominated = if has_depth_2 {
                    new_bin || child.x1_prev >= parent.x1_curr || child.y2_prev >= parent.y2_curr
                } else if has_depth_1 {
                    new_bin || child.x1_prev >= parent.x1_curr
                } else {
                    has_depth_0 && new_bin
                };
                if dominated {
                    children.swap_remove(index);
                } else {
                    index += 1;
                }
            }
        }

        children
    }

    /// The panel order of the search, for building solutions.
    pub fn bin_type_ids(&self) -> &[usize] {
        &self.bin_type_ids
    }
}

/// Whether two part types are interchangeable: same size (either way for
/// parts without grain), grain, value and quantity.
///
/// upstream: tree_search.cpp, `BranchingScheme::equals(StackId, StackId)`.
fn identical_item_types(problem: &Problem, first: usize, second: usize) -> bool {
    let a = &problem.item_types[first];
    let b = &problem.item_types[second];
    if a.copies != b.copies || a.profit != b.profit {
        return false;
    }
    if a.oriented && b.oriented {
        a.size == b.size
    } else {
        !a.oriented && !b.oriented && (a.size == b.size || a.size == b.size.flipped())
    }
}

impl BranchingScheme for Scheme<'_> {
    type Node = NodeRef;
    type Priority = (Guide, u64);
    type DominanceKey = CopiesPlaced;

    /// upstream: tree_search.cpp, `BranchingScheme::root`.
    fn root(&self) -> NodeRef {
        Rc::new(Node {
            id: self.new_id(),
            parent: None,
            item_type: None,
            depth: -1,
            x1_curr: 0,
            x1_prev: 0,
            y2_curr: 0,
            y2_prev: 0,
            x3_curr: 0,
            copies_placed: vec![0; self.problem.item_types.len()],
            number_of_bins: 0,
            first_stage_orientation: Orientation::Vertical,
            number_of_items: 0,
            item_area: 0,
            current_area: 0,
            waste: 0,
            profit: 0.0,
            // Upstream's guides are 0/0 on the root, which is alone in its
            // queue, so any value works.
            guide: Guide::new(0.0),
        })
    }

    fn children(&self, node: &NodeRef) -> Vec<NodeRef> {
        self.expand(node)
    }

    fn depth(&self, node: &NodeRef) -> Option<usize> {
        Some(node.number_of_items as usize)
    }

    fn is_leaf(&self, node: &NodeRef) -> bool {
        self.full(node)
    }

    fn priority(&self, node: &NodeRef) -> (Guide, u64) {
        (node.guide, node.id)
    }

    /// upstream: tree_search.cpp, `BranchingScheme::better`.
    fn is_better(&self, node: &NodeRef, other: &NodeRef) -> bool {
        match self.problem.objective {
            Objective::Knapsack => other.profit < node.profit,
            objective => {
                if !self.full(node) {
                    return false;
                }
                if !self.full(other) {
                    return true;
                }
                match objective {
                    Objective::BinPacking => other.number_of_bins > node.number_of_bins,
                    _ => self.width(other) > self.width(node),
                }
            }
        }
    }

    /// upstream: tree_search.cpp, `BranchingScheme::bound`.
    fn cannot_beat(&self, node: &NodeRef, incumbent: &NodeRef) -> bool {
        let problem = self.problem;
        match problem.objective {
            Objective::Knapsack => self.knapsack_upper_bound(node) <= incumbent.profit,
            Objective::BinPacking => {
                if !self.full(incumbent) {
                    return false;
                }
                // The fewest panels that could hold the parts and the waste
                // so far.
                let mut area = problem.item_area + node.waste;
                let mut bins_needed = 0;
                while area > 0 {
                    if bins_needed >= problem.number_of_bins {
                        return true;
                    }
                    let bin_type = self.bin_type_ids[bins_needed as usize];
                    area -= problem.bin_types[bin_type].size.area();
                    bins_needed += 1;
                }
                bins_needed >= incumbent.number_of_bins
            }
            Objective::Feasibility => unreachable!("feasibility subproblems use other searches"),
            Objective::OpenDimensionX | Objective::OpenDimensionY => {
                if !self.full(incumbent) {
                    return false;
                }
                // upstream divides by the panel's fixed side in the
                // instance's own coordinates, whatever the first cuts.
                let panel = problem.bin_types[0].size;
                let fixed_side = if problem.objective == Objective::OpenDimensionX {
                    panel.h
                } else {
                    panel.w
                };
                let area_bound = (node.waste + problem.item_area - 1) / fixed_side + 1;
                self.width(node).max(area_bound) >= self.width(incumbent)
            }
        }
    }

    fn dominance_key(&self, node: &NodeRef) -> Option<CopiesPlaced> {
        Some(CopiesPlaced(Rc::clone(node)))
    }

    fn dominates(&self, node: &NodeRef, other: &NodeRef) -> bool {
        self.front_dominates(&self.front(node), &self.front(other))
    }
}
