//! Turning a search node into a cutting plan.
//!
//! upstream: src/rectangleguillotine/solution_builder.cpp (`SolutionBuilder`),
//! tree_search.cpp (`BranchingScheme::to_solution`) and solution.cpp
//! (`Solution::update_indicators`, `Solution::operator<`).
//!
//! The plan is first built the way upstream builds it, as a flat list of
//! nodes per panel, so that every node (and every offcut upstream adds to
//! close a piece) comes out in upstream's order. It is converted to the
//! public [`model::Solution`] tree only when someone asks for it.

use model::{CutDirection, Instance, Length, NodeKind, Objective, Pattern, Solution};

use crate::problem::{Orientation, Problem};
use crate::scheme::{Node, Scheme};

/// What a plan node holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Content {
    /// The panel itself.
    Panel,
    /// A piece cut further.
    Piece,
    /// A part of this part type.
    Part(usize),
    /// Waste (upstream -1).
    Offcut,
    /// The reusable remainder of the last panel (upstream -3).
    Leftover,
}

/// A rectangle of a panel's cutting tree, in the panel's own coordinates
/// (upstream `SolutionNode`).
#[derive(Debug, Clone)]
struct PlanNode {
    /// Index of the parent in the panel's node list; `None` for the panel.
    parent: Option<usize>,
    /// Stage: 0 for the panel, 1 for the pieces of the first cuts, …
    depth: u32,
    left: i64,
    right: i64,
    bottom: i64,
    top: i64,
    content: Content,
    /// Indices of the children, in cutting order.
    children: Vec<usize>,
}

/// One panel's cutting tree (upstream `SolutionBin`).
#[derive(Debug, Clone)]
struct PlanBin {
    bin_type: usize,
    first_cut_orientation: Orientation,
    nodes: Vec<PlanNode>,
}

/// A complete or partial cutting plan, with the figures upstream compares
/// plans by.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    bins: Vec<PlanBin>,
    /// Whether every part that must be cut is cut.
    pub feasible: bool,
    pub number_of_bins: u32,
    pub profit: f64,
    /// How far the cuts reach along x on the last panel (upstream `width`).
    pub width: i64,
    /// How far the cuts reach along y on the last panel (upstream `height`).
    pub height: i64,
}

impl Plan {
    /// The plan with no panel (upstream's empty `Solution`).
    pub fn empty(problem: &Problem) -> Plan {
        PlanBuilder::new(problem).build()
    }

    /// Whether `self` is strictly better than `other`.
    ///
    /// upstream: solution.cpp, `Solution::operator<` (with the operands
    /// swapped: upstream's `a < b` means `b` is better).
    pub fn is_better_than(&self, other: &Plan, objective: Objective) -> bool {
        if !self.feasible {
            return false;
        }
        if !other.feasible {
            return true;
        }
        match objective {
            Objective::BinPacking => self.number_of_bins < other.number_of_bins,
            Objective::OpenDimensionX => self.width < other.width,
            Objective::OpenDimensionY => self.height < other.height,
            Objective::Knapsack => strictly_greater_profit(self.profit, other.profit),
        }
    }

    /// The plan in the public format.
    ///
    /// upstream: tools/oracle/main.cpp, `write_solution`, which defines how
    /// upstream's nodes map to ours.
    pub fn to_solution(&self, problem: &Problem, instance: &Instance) -> Solution {
        let patterns = self
            .bins
            .iter()
            .map(|bin| Pattern {
                panel_id: instance.panels[bin.bin_type].id.clone(),
                repeat: 1,
                first_cuts: if bin.first_cut_orientation == Orientation::Vertical {
                    CutDirection::AcrossLength
                } else {
                    CutDirection::AlongLength
                },
                root: model_node(problem, instance, bin, 0),
            })
            .collect();
        Solution { patterns }
    }
}

/// Whether `v1` is greater than `v2` by more than a relative 1e-6.
///
/// upstream: include/packingsolver/algorithms/common.hpp, `strictly_greater_profit`.
pub(crate) fn strictly_greater_profit(v1: f64, v2: f64) -> bool {
    if v1 == v2 {
        return false;
    }
    let denominator = v1.max(v2);
    if denominator != 0.0 && denominator.is_finite() {
        (v1 - v2) / denominator > 1e-6
    } else {
        v1 - v2 > 1e-6
    }
}

fn model_node(problem: &Problem, instance: &Instance, bin: &PlanBin, index: usize) -> model::Node {
    let node = &bin.nodes[index];
    let length = node.right - node.left;
    let kind = match node.content {
        Content::Part(item_type) if node.children.is_empty() => {
            let size = problem.item_types[item_type].size;
            NodeKind::Part {
                part_id: instance.parts[item_type].id.clone(),
                rotated: size.w != size.h && length != size.w,
            }
        }
        Content::Offcut if node.children.is_empty() => NodeKind::Offcut,
        Content::Leftover if node.children.is_empty() => NodeKind::Leftover,
        _ => NodeKind::Piece,
    };
    model::Node {
        kind,
        x: Length::from_tenths(node.left),
        y: Length::from_tenths(node.bottom),
        length: Length::from_tenths(length),
        width: Length::from_tenths(node.top - node.bottom),
        stage: node.depth,
        children: node
            .children
            .iter()
            .map(|&child| model_node(problem, instance, bin, child))
            .collect(),
    }
}

/// Builds a plan cut by cut, the way upstream's `SolutionBuilder` does:
/// each new node is a child of the last node one stage up, and pieces are
/// closed with an offcut when the next node leaves them.
struct PlanBuilder<'a> {
    problem: &'a Problem,
    bins: Vec<PlanBin>,
}

impl<'a> PlanBuilder<'a> {
    fn new(problem: &'a Problem) -> Self {
        PlanBuilder {
            problem,
            bins: Vec::new(),
        }
    }

    /// upstream: `SolutionBuilder::add_bin`, without trims.
    fn add_bin(&mut self, bin_type: usize, first_cut_orientation: Orientation) {
        let size = self.problem.bin_types[bin_type].size;
        self.bins.push(PlanBin {
            bin_type,
            first_cut_orientation,
            nodes: vec![PlanNode {
                parent: None,
                depth: 0,
                left: 0,
                right: size.w,
                bottom: 0,
                top: size.h,
                content: Content::Panel,
                children: Vec::new(),
            }],
        });
    }

    /// Adds an offcut after the last child of `parent` if it does not
    /// reach the parent's end.
    fn close(bin: &mut PlanBin, parent: usize, cut_thickness: i64) {
        let Some(&last) = bin.nodes[parent].children.last() else {
            return;
        };
        let brother = &bin.nodes[last];
        let piece = &bin.nodes[parent];
        let (left, bottom) = if piece.right > brother.right + cut_thickness {
            (brother.right + cut_thickness, piece.bottom)
        } else if piece.top > brother.top + cut_thickness {
            (piece.left, brother.top + cut_thickness)
        } else {
            return;
        };
        let offcut = PlanNode {
            parent: Some(parent),
            depth: piece.depth + 1,
            left,
            right: piece.right,
            bottom,
            top: piece.top,
            content: Content::Offcut,
            children: Vec::new(),
        };
        let index = bin.nodes.len();
        bin.nodes[parent].children.push(index);
        bin.nodes.push(offcut);
    }

    /// Adds a node at `depth`, ending at `cut_position`, after the last
    /// child of the current piece one stage up.
    ///
    /// upstream: `SolutionBuilder::add_node`.
    fn add_node(&mut self, depth: u32, cut_position: i64) {
        let cut_thickness = self.problem.cut_thickness;
        let bin = self.bins.last_mut().expect("a panel is added first");
        let mut parent = bin.nodes.len() - 1;
        debug_assert!(depth >= 1 && depth <= bin.nodes[parent].depth + 1);
        while bin.nodes[parent].depth != depth - 1 {
            Self::close(bin, parent, cut_thickness);
            parent = bin.nodes[parent]
                .parent
                .expect("depth found below the panel");
        }

        if bin.nodes[parent].parent.is_some() {
            debug_assert!(!matches!(bin.nodes[parent].content, Content::Part(_)));
            bin.nodes[parent].content = Content::Piece;
        }
        let piece = &bin.nodes[parent];
        let previous = piece.children.last().map(|&child| &bin.nodes[child]);
        let cuts_along_x = (bin.first_cut_orientation == Orientation::Vertical) == (depth % 2 == 1);
        let (left, right, bottom, top) = if cuts_along_x {
            let left = previous.map_or(piece.left, |node| node.right + cut_thickness);
            (left, cut_position, piece.bottom, piece.top)
        } else {
            let bottom = previous.map_or(piece.bottom, |node| node.top + cut_thickness);
            (piece.left, piece.right, bottom, cut_position)
        };
        debug_assert!(left < right && right <= piece.right, "bad cut position");
        debug_assert!(bottom < top && top <= piece.top, "bad cut position");
        let node = PlanNode {
            parent: Some(parent),
            depth,
            left,
            right,
            bottom,
            top,
            content: Content::Offcut,
            children: Vec::new(),
        };
        let index = bin.nodes.len();
        bin.nodes[parent].children.push(index);
        bin.nodes.push(node);
    }

    /// Marks the last node as a part.
    ///
    /// upstream: `SolutionBuilder::set_last_node_item`.
    fn set_last_node_item(&mut self, item_type: usize) {
        let node = self
            .bins
            .last_mut()
            .and_then(|bin| bin.nodes.last_mut())
            .expect("a node is added first");
        let size = self.problem.item_types[item_type].size;
        let (w, h) = (node.right - node.left, node.top - node.bottom);
        debug_assert!((w, h) == (size.w, size.h) || (w, h) == (size.h, size.w));
        node.content = Content::Part(item_type);
    }

    /// Closes the open pieces, marks the leftover and computes the figures.
    ///
    /// upstream: `SolutionBuilder::build` and `Solution::update_indicators`.
    fn build(mut self) -> Plan {
        let cut_thickness = self.problem.cut_thickness;
        for bin in &mut self.bins {
            let mut parent = Some(bin.nodes.len() - 1);
            while let Some(index) = parent {
                Self::close(bin, index, cut_thickness);
                parent = bin.nodes[index].parent;
            }
        }
        if let Some(node) = self.bins.last_mut().and_then(|bin| bin.nodes.last_mut()) {
            if node.content == Content::Offcut && node.depth == 1 {
                node.content = Content::Leftover;
            }
        }

        let mut copies = vec![0u32; self.problem.item_types.len()];
        let mut profit = 0.0;
        let (mut width, mut height) = (0, 0);
        for bin in &self.bins {
            // upstream: the extent is that of the last panel.
            width = 0;
            height = 0;
            for node in &bin.nodes {
                if let Content::Part(item_type) = node.content {
                    if node.children.is_empty() {
                        copies[item_type] += 1;
                        profit += self.problem.item_types[item_type].profit;
                    }
                }
                if node.depth > 0 && node.content != Content::Leftover {
                    width = width.max(node.right);
                    height = height.max(node.top);
                }
            }
        }
        let feasible = self.problem.objective == Objective::Knapsack
            || copies
                .iter()
                .zip(&self.problem.item_types)
                .all(|(&cut, item)| cut == item.copies);

        Plan {
            number_of_bins: self.bins.len() as u32,
            bins: self.bins,
            feasible,
            profit,
            width,
            height,
        }
    }
}

/// The plan of a search node.
///
/// upstream: tree_search.cpp, `BranchingScheme::to_solution`, without trims
/// and defects.
pub(crate) fn plan_of(scheme: &Scheme, problem: &Problem, node: &Node) -> Plan {
    // The placements from the first to `node`.
    let mut placements: Vec<&Node> = Vec::new();
    let mut current = node;
    while let Some(parent) = &current.parent {
        placements.push(current);
        current = parent;
    }
    placements.reverse();

    let three_stages = problem.number_of_stages >= 3;
    let mut builder = PlanBuilder::new(problem);
    let mut subplate2_y2 = 0;
    for (index, placement) in placements.iter().enumerate() {
        let item_type = placement.item_type.expect("only the root has no part");
        // The strips a placement opens end where the last placement in
        // them ends.
        let later = &placements[index + 1..];

        // New panels.
        while (builder.bins.len() as u32) < placement.number_of_bins {
            let vertical =
                (placement.first_stage_orientation == Orientation::Vertical) == three_stages;
            let bin_type = scheme.bin_type_ids()[builder.bins.len()];
            builder.add_bin(
                bin_type,
                if vertical {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                },
            );
        }

        // A new level-1 strip.
        if placement.depth < 1 && three_stages {
            let x1 = later
                .iter()
                .take_while(|next| next.depth >= 1)
                .last()
                .map_or(placement.x1_curr, |next| next.x1_curr);
            builder.add_node(1, x1);
        }

        // A new level-2 piece.
        if placement.depth < 2 {
            subplate2_y2 = later
                .iter()
                .take_while(|next| next.depth >= 2)
                .last()
                .map_or(placement.y2_curr, |next| next.y2_curr);
            builder.add_node(if three_stages { 2 } else { 1 }, subplate2_y2);
        }

        // The level-3 piece holding the part.
        builder.add_node(if three_stages { 3 } else { 2 }, placement.x3_curr);

        // The part, at the bottom of its piece, and the offcut above it if
        // it does not fill it.
        let parent = placement
            .parent
            .as_deref()
            .expect("only the root has no parent");
        let part_width = placement.x3_curr - scheme.x3_prev(parent, placement.depth);
        let size = problem.item_types[item_type].size;
        let part_height = if part_width == size.w { size.h } else { size.w };
        let cut_position = placement.y2_prev + part_height;
        if cut_position != subplate2_y2 {
            builder.add_node(if three_stages { 4 } else { 3 }, cut_position);
        }
        builder.set_last_node_item(item_type);
    }
    builder.build()
}
