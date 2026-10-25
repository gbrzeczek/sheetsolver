//! Rules about the shape of one pattern's cutting tree: every cut is a
//! straight guillotine cut, pieces sit exactly one kerf apart, and parts
//! have their part's size.

use model::{CutDirection, Instance, Length, Node, NodeKind, Pattern};

use crate::violation::{Problem, Violation};

/// Checks one pattern's tree and pushes every violation found.
pub(crate) fn check_pattern(
    instance: &Instance,
    pattern: &Pattern,
    pattern_name: &str,
    is_last_pattern: bool,
    violations: &mut Vec<Violation>,
) {
    let mut report = |path: &str, problem: Problem| {
        violations.push(Violation {
            location: location(pattern_name, path),
            problem,
        });
    };

    let root = &pattern.root;
    let root_is_panel = root.kind == NodeKind::Piece
        && root.stage == 0
        && root.x == Length::ZERO
        && root.y == Length::ZERO
        && instance
            .panel(&pattern.panel_id)
            .is_none_or(|panel| root.length == panel.length && root.width == panel.width);
    if !root_is_panel {
        report("", Problem::RootIsNotThePanel);
    }

    let checker = TreeChecker {
        instance,
        kerf: instance.saw.kerf,
        leftover_allowed: is_last_pattern && pattern.repeat == 1,
    };
    checker.check_children(root, pattern.first_cuts, "", &mut report);
}

struct TreeChecker<'a> {
    instance: &'a Instance,
    kerf: Length,
    /// Whether this pattern may end with a leftover.
    leftover_allowed: bool,
}

impl TreeChecker<'_> {
    /// Checks how `node` is cut into its children by cuts in `direction`,
    /// then each child.
    fn check_children(
        &self,
        node: &Node,
        direction: CutDirection,
        path: &str,
        report: &mut impl FnMut(&str, Problem),
    ) {
        let cross = direction.turned();
        let mut expected_start = node.start(direction);
        for (index, child) in node.children.iter().enumerate() {
            let child_path = child_path(path, index);

            if child.stage != node.stage + 1 {
                report(
                    &child_path,
                    Problem::WrongStage {
                        expected: node.stage + 1,
                        found: child.stage,
                    },
                );
            }
            if child.length <= Length::ZERO || child.width <= Length::ZERO {
                report(&child_path, Problem::EmptyRectangle);
            }
            if child.start(cross) != node.start(cross) || child.size(cross) != node.size(cross) {
                let axis = match cross {
                    CutDirection::AcrossLength => "length",
                    CutDirection::AlongLength => "width",
                };
                report(&child_path, Problem::NotSpanningParent { axis });
            }
            if child.start(direction) != expected_start {
                report(
                    &child_path,
                    Problem::WrongPosition {
                        expected: expected_start,
                        found: child.start(direction),
                    },
                );
            }
            expected_start = child.end(direction) + self.kerf;

            self.check_node(node, index, child, cross, &child_path, report);
        }

        if let Some(last) = node.children.last() {
            let children_end = last.end(direction);
            let piece_end = node.end(direction);
            if children_end > piece_end {
                report(
                    path,
                    Problem::BeyondParent {
                        children_end,
                        piece_end,
                    },
                );
            } else if piece_end - children_end > self.kerf {
                // Upstream adds an offcut for anything wider than a kerf;
                // up to one kerf may be lost in the saw cut at the edge.
                report(
                    path,
                    Problem::UnaccountedRemainder {
                        remainder: piece_end - children_end,
                    },
                );
            }
        }
    }

    /// Checks one child by its kind, and recurses into pieces.
    fn check_node(
        &self,
        parent: &Node,
        index: usize,
        node: &Node,
        next_direction: CutDirection,
        path: &str,
        report: &mut impl FnMut(&str, Problem),
    ) {
        if node.kind != NodeKind::Piece && !node.children.is_empty() {
            report(path, Problem::LeafWithChildren);
        }
        match &node.kind {
            NodeKind::Piece => {
                if node.children.is_empty() {
                    report(path, Problem::PieceWithoutChildren);
                }
                self.check_children(node, next_direction, path, report);
            }
            NodeKind::Part { part_id, rotated } => {
                self.check_part(node, part_id, *rotated, path, report)
            }
            NodeKind::Offcut => {}
            NodeKind::Leftover => {
                let is_last_first_stage_piece =
                    parent.stage == 0 && index + 1 == parent.children.len();
                if !(self.leftover_allowed && is_last_first_stage_piece) {
                    report(path, Problem::MisplacedLeftover);
                }
            }
        }
    }

    fn check_part(
        &self,
        node: &Node,
        part_id: &str,
        rotated: bool,
        path: &str,
        report: &mut impl FnMut(&str, Problem),
    ) {
        let Some(part) = self.instance.part(part_id) else {
            report(path, Problem::UnknownPart(part_id.to_string()));
            return;
        };
        let (length, width) = if rotated {
            (part.width, part.length)
        } else {
            (part.length, part.width)
        };
        if (node.length, node.width) != (length, width) {
            report(
                path,
                Problem::WrongPartSize {
                    part_id: part_id.to_string(),
                    rotated,
                    length: node.length,
                    width: node.width,
                },
            );
        }
        if rotated && part.grain {
            report(path, Problem::GrainTurned(part_id.to_string()));
        }
    }
}

/// The path of a node's `index`-th child, numbering children from 1.
pub(crate) fn child_path(path: &str, index: usize) -> String {
    if path.is_empty() {
        (index + 1).to_string()
    } else {
        format!("{path}.{}", index + 1)
    }
}

/// A human-readable location: "pattern 2, panel" or "pattern 2, piece 1.3".
pub(crate) fn location(pattern_name: &str, path: &str) -> String {
    if path.is_empty() {
        format!("{pattern_name}, panel")
    } else {
        format!("{pattern_name}, piece {path}")
    }
}
