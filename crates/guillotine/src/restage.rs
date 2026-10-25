//! Rebuilding a plan's cutting tree with as few stages as possible.
//!
//! upstream: src/rectangleguillotine/post_process.cpp,
//! `minimize_number_of_stages` and `build_guillotine_solution(s)`, without
//! trims. Only the parts' positions are kept: each piece is split into
//! groups of parts no cut can separate, in the stage's direction, and each
//! group becomes a piece cut the other way.

use crate::cpp_sort;
use crate::plan::{Content, Plan, PlanBuilder};
use crate::problem::{Orientation, Problem};

/// A part's place on its panel (upstream `ItemPlacement`).
#[derive(Debug, Clone, Copy)]
struct Placement {
    item_type: usize,
    left: i64,
    right: i64,
    bottom: i64,
    top: i64,
}

/// Parts that no cut in the stage's direction separates, and where the
/// cut after them is (upstream `CutGroup`).
struct Group {
    cut_position: i64,
    items: Vec<Placement>,
}

/// Splits parts into consecutive groups along x (`vertical`) or y: parts
/// closer than a kerf share a group.
///
/// upstream: post_process.cpp, `find_groups`.
fn find_groups(items: &[Placement], vertical: bool, cut_thickness: i64) -> Vec<Group> {
    let start = |item: &Placement| if vertical { item.left } else { item.bottom };
    let end = |item: &Placement| if vertical { item.right } else { item.top };
    let mut sorted = items.to_vec();
    cpp_sort::sort(&mut sorted, |a, b| start(a) < start(b));
    let mut groups: Vec<Group> = Vec::new();
    for item in sorted {
        match groups.last_mut() {
            Some(group) if start(&item) < group.cut_position + cut_thickness => {
                group.cut_position = group.cut_position.max(end(&item));
                group.items.push(item);
            }
            _ => groups.push(Group {
                cut_position: end(&item),
                items: vec![item],
            }),
        }
    }
    groups
}

/// Adds the cuts of the region `[left, right] × [bottom, top]`, whose node
/// is the builder's last, holding `items`, at `depth`. Fails if the parts
/// cannot be cut apart this way.
///
/// upstream: post_process.cpp, `build_region`.
#[allow(clippy::too_many_arguments)]
fn build_region(
    problem: &Problem,
    region: (i64, i64, i64, i64),
    items: &[Placement],
    depth: u32,
    orientation: Orientation,
    builder: &mut PlanBuilder,
) -> Option<()> {
    let (left, right, bottom, top) = region;
    if items.is_empty() {
        return Some(());
    }
    // At depth 1 no piece has been added yet: the panel is the last node.
    if depth > 1 && items.len() == 1 {
        let item = items[0];
        if (item.left, item.right, item.bottom, item.top) == (left, right, bottom, top) {
            return builder.try_set_last_node_item(item.item_type).ok();
        }
    }

    let cut_thickness = problem.cut_thickness;
    let vertical = (orientation == Orientation::Vertical) == (depth % 2 == 1);
    let start = |item: &Placement| if vertical { item.left } else { item.bottom };
    let mut groups = find_groups(items, vertical, cut_thickness);

    // A waste between two groups too narrow for its own cut stays at the
    // end of the first group's piece.
    for index in 0..groups.len().saturating_sub(1) {
        let next_start = start(&groups[index + 1].items[0]);
        let gap = next_start - cut_thickness - groups[index].cut_position;
        if gap > 0 && gap <= cut_thickness {
            groups[index].cut_position = next_start - cut_thickness;
        }
    }

    let mut previous = if vertical { left } else { bottom };
    for group in &groups {
        let group_start = start(&group.items[0]);
        // A waste before the group gets a piece of its own.
        if group_start - cut_thickness > previous {
            builder
                .try_add_node(depth, group_start - cut_thickness)
                .ok()?;
            previous = group_start;
        }
        // A waste too narrow for a cut before the group.
        if group_start > previous {
            return None;
        }
        builder.try_add_node(depth, group.cut_position).ok()?;
        let sub = if vertical {
            (previous, group.cut_position, bottom, top)
        } else {
            (left, right, previous, group.cut_position)
        };
        build_region(problem, sub, &group.items, depth + 1, orientation, builder)?;
        previous = group.cut_position + cut_thickness;
    }
    Some(())
}

/// The one-panel plan of `items` on `bin_type` with first cuts in
/// `orientation`, if the parts can be cut apart that way.
///
/// upstream: post_process.cpp, `try_build`; upstream catches the errors
/// the builder throws, here they are `None`.
fn try_build(
    problem: &Problem,
    bin_type: usize,
    items: &[Placement],
    orientation: Orientation,
) -> Option<Plan> {
    let size = problem.bin_types[bin_type].size;
    let mut builder = PlanBuilder::new(problem);
    builder.add_bin(bin_type, 1, orientation);
    if !items.is_empty() {
        build_region(
            problem,
            (0, size.w, 0, size.h),
            items,
            1,
            orientation,
            &mut builder,
        )?;
    }
    Some(builder.build())
}

/// The same parts, each panel rebuilt with the first-cut direction that
/// needs fewer stages (vertical on ties), or kept as it is if neither
/// works.
///
/// upstream: post_process.cpp, `minimize_number_of_stages`.
pub(crate) fn minimize_number_of_stages(problem: &Problem, plan: &Plan) -> Plan {
    let mut result = Plan::empty(problem);
    for (bin_pos, bin) in plan.bins().iter().enumerate() {
        let items: Vec<Placement> = bin
            .nodes
            .iter()
            .filter_map(|node| match node.content {
                Content::Part(item_type) => Some(Placement {
                    item_type,
                    left: node.left,
                    right: node.right,
                    bottom: node.bottom,
                    top: node.top,
                }),
                _ => None,
            })
            .collect();
        let orientations: &[Orientation] = match problem.first_stage_orientation {
            Orientation::Any => &[Orientation::Vertical, Orientation::Horizontal],
            Orientation::Vertical => &[Orientation::Vertical],
            Orientation::Horizontal => &[Orientation::Horizontal],
        };
        let built: Vec<Plan> = orientations
            .iter()
            .filter_map(|&orientation| try_build(problem, bin.bin_type, &items, orientation))
            .collect();
        let chosen = match built.as_slice() {
            [] => None,
            [only] => Some(only),
            [first, second, ..] => Some(if first.number_of_stages() <= second.number_of_stages() {
                first
            } else {
                second
            }),
        };
        match chosen {
            Some(rebuilt) => result.append_bin(problem, rebuilt, 0, bin.copies, None, None),
            None => result.append_bin(problem, plan, bin_pos, bin.copies, None, None),
        }
    }
    result
}
