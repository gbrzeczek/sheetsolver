//! Cutting plans as upstream builds and compares them.
//!
//! upstream: src/rectangleguillotine/solution_builder.cpp (`SolutionBuilder`),
//! tree_search.cpp (`BranchingScheme::to_solution`) and solution.cpp
//! (`Solution::update_indicators`, `append_bin`, `operator<`).
//!
//! A plan is built the way upstream builds it, as a flat list of nodes per
//! panel, so that every node (and every offcut upstream adds to close a
//! piece) comes out in upstream's order. It is converted to the public
//! [`model::Solution`] tree only when someone asks for it.

use model::{CutDirection, Instance, Length, NodeKind, Pattern, Solution};

use crate::problem::{Objective, Orientation, Problem};
use crate::scheme::{Node, Scheme};

/// What a plan node holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Content {
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
    /// The panel without its hard trims (upstream's depth-0 node under the
    /// root). It is not among the root's children, as in upstream, whose
    /// `SolutionBuilder::add_bin` adds the trims and this node to a copy of
    /// the root; the trims themselves are not stored (`to_solution` writes
    /// them from the panel's trims).
    Trimmed,
}

/// A rectangle of a panel's cutting tree, in the panel's own coordinates
/// (upstream `SolutionNode`).
#[derive(Debug, Clone)]
pub(crate) struct PlanNode {
    /// Index of the parent in the panel's node list; `None` for the panel.
    pub parent: Option<usize>,
    /// Stage: 0 for the panel, 1 for the pieces of the first cuts, …
    pub depth: u32,
    pub left: i64,
    pub right: i64,
    pub bottom: i64,
    pub top: i64,
    pub content: Content,
    /// Indices of the children, in cutting order.
    pub children: Vec<usize>,
}

/// One panel's cutting tree, cut `copies` times (upstream `SolutionBin`).
#[derive(Debug, Clone)]
pub(crate) struct PlanBin {
    pub bin_type: usize,
    pub copies: u32,
    pub first_cut_orientation: Orientation,
    pub nodes: Vec<PlanNode>,
}

/// A complete or partial cutting plan, with the figures upstream compares
/// plans by.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    bins: Vec<PlanBin>,
    /// Whether the plan breaks no rule and, except for the knapsack
    /// objective, cuts every part.
    pub feasible: bool,
    /// Panels used, repeats included.
    pub number_of_bins: u32,
    /// Parts cut, repeats included.
    pub number_of_items: u32,
    /// Copies cut of each part type.
    pub item_copies: Vec<u32>,
    pub profit: f64,
    /// How far the cuts reach along x on the last panel (upstream `width`).
    pub width: i64,
    /// How far the cuts reach along y on the last panel (upstream `height`).
    pub height: i64,
}

impl Plan {
    /// The plan with no panel (upstream's empty `Solution`).
    pub fn empty(problem: &Problem) -> Plan {
        Plan::evaluate(Vec::new(), problem)
    }

    /// The panels of the plan, in cutting order.
    pub fn bins(&self) -> &[PlanBin] {
        &self.bins
    }

    /// Panels of `bin_type` used, repeats included.
    pub fn bin_copies(&self, bin_type: usize) -> u32 {
        self.bins
            .iter()
            .filter(|bin| bin.bin_type == bin_type)
            .map(|bin| bin.copies)
            .sum()
    }

    /// The figures of a plan made of `bins`.
    ///
    /// upstream: solution.cpp, `Solution::update_indicators`, run on each
    /// panel in turn; the figures it accumulates are summed in the same
    /// order here.
    fn evaluate(bins: Vec<PlanBin>, problem: &Problem) -> Plan {
        let mut item_copies = vec![0u32; problem.item_types.len()];
        let mut number_of_bins = 0;
        let mut number_of_items = 0;
        let mut profit = 0.0;
        let (mut width, mut height) = (0, 0);
        for bin in &bins {
            number_of_bins += bin.copies;
            // upstream: the extent is that of the last panel.
            width = 0;
            height = 0;
            for node in &bin.nodes {
                if let Content::Part(item_type) = node.content {
                    number_of_items += bin.copies;
                    profit += f64::from(bin.copies) * problem.item_types[item_type].profit;
                    item_copies[item_type] += bin.copies;
                }
                if node.depth > 0 && node.content != Content::Leftover {
                    width = width.max(node.right);
                    height = height.max(node.top);
                }
            }
        }
        let copies_fit = item_copies
            .iter()
            .zip(&problem.item_types)
            .all(|(&cut, item)| cut <= item.copies);
        let all_cut = item_copies
            .iter()
            .zip(&problem.item_types)
            .all(|(&cut, item)| cut == item.copies);
        // upstream also checks the number of stages, but no plan built here
        // can break it: the tree search builds at most the stages allowed,
        // and the other algorithms only run with unlimited stages.
        let feasible = copies_fit && (problem.objective == Objective::Knapsack || all_cut);
        Plan {
            bins,
            feasible,
            number_of_bins,
            number_of_items,
            item_copies,
            profit,
            width,
            height,
        }
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
            Objective::Knapsack | Objective::Feasibility => {
                strictly_greater_profit(self.profit, other.profit)
            }
        }
    }

    /// The number of stages of the plan, counting every level of cuts.
    ///
    /// upstream: solution.cpp, `Solution::update_indicators` ("staging
    /// analysis") for exact cuts: the deepest node that differs from its
    /// parent.
    pub fn number_of_stages(&self) -> u32 {
        self.bins.iter().map(stages_used_exact).max().unwrap_or(0)
    }

    /// Appends panel `bin_pos` of `from`, a plan of another problem, cut
    /// `copies` times. `bin_types` and `item_types` map `from`'s panel and
    /// part types to this problem's (`None`: the same).
    ///
    /// upstream: solution.cpp, `Solution::append_bin`. The panel takes the
    /// size of its new panel type (a subproblem may have cut a shorter
    /// one), and the previous last panel's leftover becomes an offcut.
    pub fn append_bin(
        &mut self,
        problem: &Problem,
        from: &Plan,
        bin_pos: usize,
        copies: u32,
        bin_types: Option<&[usize]>,
        item_types: Option<&[usize]>,
    ) {
        if let Some(node) = self.bins.last_mut().and_then(|bin| bin.nodes.last_mut()) {
            if node.content == Content::Leftover {
                node.content = Content::Offcut;
            }
        }
        let old = &from.bins[bin_pos];
        let bin_type = bin_types.map_or(old.bin_type, |map| map[old.bin_type]);
        let size = problem.bin_types[bin_type].size;
        let nodes = old
            .nodes
            .iter()
            .map(|node| {
                let mut node = node.clone();
                match node.content {
                    Content::Panel => {
                        node.right = size.w;
                        node.top = size.h;
                    }
                    Content::Part(item_type) => {
                        node.content =
                            Content::Part(item_types.map_or(item_type, |map| map[item_type]));
                    }
                    _ => {}
                }
                node
            })
            .collect();
        let mut bins = std::mem::take(&mut self.bins);
        bins.push(PlanBin {
            bin_type,
            copies,
            first_cut_orientation: old.first_cut_orientation,
            nodes,
        });
        *self = Plan::evaluate(bins, problem);
    }

    /// Appends every panel of `from`, once each.
    ///
    /// upstream: solution.cpp, `Solution::append_bins`.
    pub fn append_bins(&mut self, problem: &Problem, from: &Plan, bin_types: Option<&[usize]>) {
        for bin_pos in 0..from.bins.len() {
            self.append_bin(problem, from, bin_pos, 1, bin_types, None);
        }
    }

    /// Appends `copies` uncut panels of `bin_type`.
    ///
    /// upstream: solution.cpp, `Solution::append_empty_bin`.
    pub fn append_empty_bin(&mut self, problem: &Problem, bin_type: usize, copies: u32) {
        let orientation = match problem.first_stage_orientation {
            Orientation::Any => Orientation::Vertical,
            orientation => orientation,
        };
        let mut builder = PlanBuilder::new(problem);
        builder.add_bin(bin_type, copies, orientation);
        let empty = builder.build();
        self.append_bins(problem, &empty, None);
    }

    /// The plan in the public format.
    ///
    /// upstream: tools/oracle/main.cpp, `write_solution`, which defines how
    /// upstream's nodes map to ours.
    ///
    /// Upstream's leftover belongs to the very last panel only: a last
    /// panel cut several times becomes two patterns, the copies but the
    /// last with the leftover as an offcut, then the last one (the oracle
    /// writes it the same way).
    pub fn to_solution(&self, problem: &Problem, instance: &Instance) -> Solution {
        let pattern = |bin: &PlanBin, repeat: u32, leftover_as_offcut: bool| Pattern {
            panel_id: instance.panels[bin.bin_type].id.clone(),
            repeat,
            first_cuts: if bin.first_cut_orientation == Orientation::Vertical {
                CutDirection::AcrossLength
            } else {
                CutDirection::AlongLength
            },
            root: model_root(problem, instance, bin, leftover_as_offcut),
        };
        let mut patterns = Vec::new();
        for bin in &self.bins {
            let has_leftover = bin
                .nodes
                .iter()
                .any(|node| node.content == Content::Leftover);
            if has_leftover && bin.copies > 1 {
                patterns.push(pattern(bin, bin.copies - 1, true));
                patterns.push(pattern(bin, 1, false));
            } else {
                patterns.push(pattern(bin, bin.copies, false));
            }
        }
        let mut solution = Solution { patterns };
        crate::open_dimension::complete_open_dimension(instance, &mut solution);
        solution
    }
}

/// The deepest stage of a node that differs from its parent: the number of
/// stages of the panel with exact cuts.
fn stages_used_exact(bin: &PlanBin) -> u32 {
    bin.nodes
        .iter()
        .filter(|node| {
            node.parent.is_none_or(|parent| {
                let parent = &bin.nodes[parent];
                (node.left, node.right, node.bottom, node.top)
                    != (parent.left, parent.right, parent.bottom, parent.top)
            })
        })
        .map(|node| node.depth)
        .max()
        .unwrap_or(0)
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

/// A panel's cutting tree in the public format. With trims cut off first,
/// the panel is cut into those trims, then the trimmed panel, all at
/// stage 0: the left and right trims run the panel's full width, the
/// bottom and top ones between them (the oracle writes them so; upstream
/// writes the corners as pieces of their own).
fn model_root(
    problem: &Problem,
    instance: &Instance,
    bin: &PlanBin,
    leftover_as_offcut: bool,
) -> model::Node {
    let panel = &instance.panels[bin.bin_type];
    let t = &panel.trims;
    let cut_first = |trim: model::Trim| {
        if problem.trims_moved || trim.is_cut_off_first() {
            trim.width
        } else {
            Length::ZERO
        }
    };
    let (left, right, bottom, top) = (
        cut_first(t.left),
        cut_first(t.right),
        cut_first(t.bottom),
        cut_first(t.top),
    );
    if (left, right, bottom, top) == (Length::ZERO, Length::ZERO, Length::ZERO, Length::ZERO) {
        return model_node(problem, instance, bin, 0, leftover_as_offcut);
    }
    let trimmed = if problem.trims_moved {
        // The plan was made on the panel without its trims.
        let mut trimmed = model_node(problem, instance, bin, 0, leftover_as_offcut);
        shift(&mut trimmed, left, bottom);
        trimmed
    } else {
        model_node(problem, instance, bin, 1, leftover_as_offcut)
    };
    let (length, width) = (panel.length, panel.width);
    let rectangle = |kind, x, y, length, width| model::Node {
        kind,
        x,
        y,
        length,
        width,
        stage: 0,
        children: Vec::new(),
    };
    let between = length - left - right;
    let mut children = Vec::new();
    if left > Length::ZERO {
        children.push(rectangle(
            NodeKind::Trim,
            Length::ZERO,
            Length::ZERO,
            left,
            width,
        ));
    }
    if right > Length::ZERO {
        children.push(rectangle(
            NodeKind::Trim,
            length - right,
            Length::ZERO,
            right,
            width,
        ));
    }
    if bottom > Length::ZERO {
        children.push(rectangle(
            NodeKind::Trim,
            left,
            Length::ZERO,
            between,
            bottom,
        ));
    }
    if top > Length::ZERO {
        children.push(rectangle(NodeKind::Trim, left, width - top, between, top));
    }
    children.push(trimmed);
    model::Node {
        children,
        ..rectangle(NodeKind::Piece, Length::ZERO, Length::ZERO, length, width)
    }
}

/// Moves a subtree by (`dx`, `dy`).
fn shift(node: &mut model::Node, dx: Length, dy: Length) {
    node.x = node.x + dx;
    node.y = node.y + dy;
    for child in &mut node.children {
        shift(child, dx, dy);
    }
}

fn model_node(
    problem: &Problem,
    instance: &Instance,
    bin: &PlanBin,
    index: usize,
    leftover_as_offcut: bool,
) -> model::Node {
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
        Content::Leftover if node.children.is_empty() && leftover_as_offcut => NodeKind::Offcut,
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
            .map(|&child| model_node(problem, instance, bin, child, leftover_as_offcut))
            .collect(),
    }
}

/// Why a cut could not be added (upstream throws).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuildError;

/// Builds a plan cut by cut, the way upstream's `SolutionBuilder` does:
/// each new node is a child of the last node one stage up, and pieces are
/// closed with an offcut when the next node leaves them.
pub(crate) struct PlanBuilder<'a> {
    problem: &'a Problem,
    bins: Vec<PlanBin>,
}

impl<'a> PlanBuilder<'a> {
    pub fn new(problem: &'a Problem) -> Self {
        PlanBuilder {
            problem,
            bins: Vec::new(),
        }
    }

    /// upstream: `SolutionBuilder::add_bin`. With hard trims, the panel's
    /// cuts start from the trimmed panel.
    pub fn add_bin(&mut self, bin_type: usize, copies: u32, first_cut_orientation: Orientation) {
        let bin = &self.problem.bin_types[bin_type];
        let size = bin.size;
        let mut nodes = vec![PlanNode {
            parent: None,
            depth: 0,
            left: 0,
            right: size.w,
            bottom: 0,
            top: size.h,
            content: Content::Panel,
            children: Vec::new(),
        }];
        let t = bin.trims;
        if t.any_hard() {
            let hard = |trim: i64, soft: bool| if soft { 0 } else { trim };
            nodes.push(PlanNode {
                parent: Some(0),
                depth: 0,
                left: hard(t.left, t.left_soft),
                right: size.w - hard(t.right, t.right_soft),
                bottom: hard(t.bottom, t.bottom_soft),
                top: size.h - hard(t.top, t.top_soft),
                content: Content::Trimmed,
                children: Vec::new(),
            });
        }
        self.bins.push(PlanBin {
            bin_type,
            copies,
            first_cut_orientation,
            nodes,
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
    /// child of the current piece one stage up. Fails, like upstream, if
    /// the depth skips a stage, the parent holds a part, or the cut is
    /// outside the piece.
    ///
    /// upstream: `SolutionBuilder::add_node`.
    pub fn try_add_node(&mut self, depth: u32, cut_position: i64) -> Result<(), BuildError> {
        let cut_thickness = self.problem.cut_thickness;
        let bin = self.bins.last_mut().ok_or(BuildError)?;
        let mut parent = bin.nodes.len() - 1;
        if depth < 1 || depth > bin.nodes[parent].depth + 1 {
            return Err(BuildError);
        }
        while bin.nodes[parent].depth != depth - 1 {
            Self::close(bin, parent, cut_thickness);
            parent = bin.nodes[parent].parent.ok_or(BuildError)?;
        }

        if bin.nodes[parent].parent.is_some() && bin.nodes[parent].content != Content::Trimmed {
            if matches!(bin.nodes[parent].content, Content::Part(_)) {
                return Err(BuildError);
            }
            bin.nodes[parent].content = Content::Piece;
        }
        let piece = &bin.nodes[parent];
        let previous = piece.children.last().map(|&child| &bin.nodes[child]);
        let cuts_along_x = (bin.first_cut_orientation == Orientation::Vertical) == (depth % 2 == 1);
        let (left, right, bottom, top) = if cuts_along_x {
            let left = previous.map_or(piece.left, |node| node.right + cut_thickness);
            if cut_position <= left || cut_position > piece.right {
                return Err(BuildError);
            }
            (left, cut_position, piece.bottom, piece.top)
        } else {
            let bottom = previous.map_or(piece.bottom, |node| node.top + cut_thickness);
            if cut_position <= bottom || cut_position > piece.top {
                return Err(BuildError);
            }
            (piece.left, piece.right, bottom, cut_position)
        };
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
        Ok(())
    }

    /// [`PlanBuilder::try_add_node`] for callers whose cuts are valid by
    /// construction.
    pub fn add_node(&mut self, depth: u32, cut_position: i64) {
        self.try_add_node(depth, cut_position)
            .expect("the cut fits its piece");
    }

    /// Marks the last node as a part. Fails, like upstream, if the node
    /// does not have the part's size.
    ///
    /// upstream: `SolutionBuilder::set_last_node_item`.
    pub fn try_set_last_node_item(&mut self, item_type: usize) -> Result<(), BuildError> {
        let size = self.problem.item_types[item_type].size;
        let node = self
            .bins
            .last_mut()
            .and_then(|bin| bin.nodes.last_mut())
            .ok_or(BuildError)?;
        let (w, h) = (node.right - node.left, node.top - node.bottom);
        if (w, h) != (size.w, size.h) && (w, h) != (size.h, size.w) {
            return Err(BuildError);
        }
        node.content = Content::Part(item_type);
        Ok(())
    }

    /// [`PlanBuilder::try_set_last_node_item`] for callers whose nodes
    /// have the part's size by construction.
    pub fn set_last_node_item(&mut self, item_type: usize) {
        self.try_set_last_node_item(item_type)
            .expect("the node has the part's size");
    }

    /// Closes the open pieces, marks the leftover and computes the figures.
    ///
    /// upstream: `SolutionBuilder::build`.
    pub fn build(mut self) -> Plan {
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
        Plan::evaluate(self.bins, self.problem)
    }
}

/// The plan of a search node.
///
/// upstream: tree_search.cpp, `BranchingScheme::to_solution`, without
/// defects.
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

        // The strip of a soft left or bottom trim is an offcut cut off
        // first: by a first cut with 3 stages, by a cut in each strip
        // otherwise. Trims are read as the search sees them.
        let kerf = problem.cut_thickness;
        let orientation = placement.first_stage_orientation;

        // New panels.
        while (builder.bins.len() as u32) < placement.number_of_bins {
            let trims = scheme.trims(builder.bins.len() as u32, orientation);
            let vertical =
                (placement.first_stage_orientation == Orientation::Vertical) == three_stages;
            let bin_type = scheme.bin_type_ids()[builder.bins.len()];
            builder.add_bin(
                bin_type,
                1,
                if vertical {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                },
            );
            if three_stages {
                if trims.has_soft_left() {
                    builder.add_node(1, scheme.x_start(&trims) - kerf);
                }
            } else if trims.has_soft_bottom() {
                builder.add_node(1, scheme.y_start(&trims) - kerf);
            }
        }

        let trims = scheme.trims(placement.number_of_bins - 1, orientation);

        // A new level-1 strip.
        if placement.depth < 1 && three_stages {
            let x1 = later
                .iter()
                .take_while(|next| next.depth >= 1)
                .last()
                .map_or(placement.x1_curr, |next| next.x1_curr);
            builder.add_node(1, x1);
            if trims.has_soft_bottom() {
                builder.add_node(2, scheme.y_start(&trims) - kerf);
            }
        }

        // A new level-2 piece.
        if placement.depth < 2 {
            subplate2_y2 = later
                .iter()
                .take_while(|next| next.depth >= 2)
                .last()
                .map_or(placement.y2_curr, |next| next.y2_curr);
            builder.add_node(if three_stages { 2 } else { 1 }, subplate2_y2);
            if !three_stages && trims.has_soft_left() {
                builder.add_node(2, scheme.x_start(&trims) - kerf);
            }
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
