//! Rules about how many times the saw turns: the number of cutting stages
//! and the cut type.
//!
//! The definitions follow upstream's own solution check
//! (`src/rectangleguillotine/solution.cpp`, `Solution::append_bin`), which
//! is the problem definition we port.

use std::collections::BTreeSet;

use model::{CutType, Instance, Node, NodeKind, Pattern, Stages};

use crate::tree::{child_path, location};
use crate::violation::{Problem, Violation};

/// Checks the number of stages and the cut type of one pattern.
pub(crate) fn check_pattern(
    instance: &Instance,
    pattern: &Pattern,
    pattern_name: &str,
    violations: &mut Vec<Violation>,
) {
    let Stages::Limited(allowed) = instance.saw.stages else {
        // Unlimited stages: any guillotine pattern is allowed, and trimming
        // is just another stage, so the cut type does not apply.
        return;
    };

    let used = stages_used(&pattern.root, instance.saw.cut_type);
    if used > allowed {
        violations.push(Violation {
            location: location(pattern_name, ""),
            problem: Problem::TooManyStages { allowed, used },
        });
    }

    if instance.saw.cut_type == CutType::Homogenous && allowed >= 1 {
        check_one_part_per_strip(&pattern.root, allowed - 1, "", pattern_name, violations);
    }
}

/// The number of stages a pattern counts as.
///
/// A node only counts as cut if it differs from its parent: a piece that
/// passes through a stage unchanged costs nothing. With trimming allowed
/// ([`CutType::NonExact`]), the deepest level does not count as a full
/// stage if every piece cut at that level only trims one part free from at
/// most one offcut.
pub(crate) fn stages_used(root: &Node, cut_type: CutType) -> u32 {
    let deepest = deepest_real_cut(root, None);
    if cut_type == CutType::NonExact && deepest > 0 && deepest_level_only_trims(root, deepest - 1) {
        deepest - 1
    } else {
        deepest
    }
}

/// The deepest stage of any node that differs from its parent.
fn deepest_real_cut(node: &Node, parent: Option<&Node>) -> u32 {
    let own = match parent {
        Some(parent) if !node.same_rectangle(parent) => node.stage,
        _ => 0,
    };
    node.children
        .iter()
        .map(|child| deepest_real_cut(child, Some(node)))
        .fold(own, u32::max)
}

/// Whether every piece at `stage` (other than the panel) has exactly one
/// part child and at most one other child, leftovers aside: a trimming cut.
/// False if there is no such piece.
fn deepest_level_only_trims(root: &Node, stage: u32) -> bool {
    let pieces: Vec<&Node> = root
        .descendants()
        .into_iter()
        .filter(|node| node.stage == stage && node.stage > 0 && !node.children.is_empty())
        .collect();
    !pieces.is_empty()
        && pieces.iter().all(|piece| {
            let counted = piece
                .children
                .iter()
                .filter(|child| child.kind != NodeKind::Leftover);
            let parts = counted
                .clone()
                .filter(|child| matches!(child.kind, NodeKind::Part { .. }));
            parts.count() == 1 && counted.count() <= 2
        })
}

/// With one part per strip, every piece at `strip_stage` (the strips the
/// last stage cuts into parts) holds copies of a single part.
fn check_one_part_per_strip(
    node: &Node,
    strip_stage: u32,
    path: &str,
    pattern_name: &str,
    violations: &mut Vec<Violation>,
) {
    if node.stage == strip_stage {
        let part_ids: BTreeSet<&str> = node
            .descendants()
            .into_iter()
            .filter_map(|n| match &n.kind {
                NodeKind::Part { part_id, .. } => Some(part_id.as_str()),
                _ => None,
            })
            .collect();
        if part_ids.len() > 1 {
            let names: Vec<&str> = part_ids.into_iter().collect();
            violations.push(Violation {
                location: location(pattern_name, path),
                problem: Problem::MixedStrip(names.join(", ")),
            });
        }
        return;
    }
    for (index, child) in node.children.iter().enumerate() {
        check_one_part_per_strip(
            child,
            strip_stage,
            &child_path(path, index),
            pattern_name,
            violations,
        );
    }
}
