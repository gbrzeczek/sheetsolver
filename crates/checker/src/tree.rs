//! Rules about the shape of one pattern's cutting tree: the trims cut off
//! first, every cut is a straight guillotine cut, pieces sit exactly one
//! kerf apart, parts have their part's size and stay out of the trims, and
//! offcuts are at least the minimum offcut width.

use model::{CutDirection, Instance, Length, Node, NodeKind, Panel, Pattern, Stages};

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

    let panel = instance.panel(&pattern.panel_id);
    let checker = TreeChecker {
        instance,
        panel,
        kerf: instance.saw.kerf,
        minimum_offcut_width: instance.saw.minimum_offcut_width,
        leftover_allowed: is_last_pattern && pattern.repeat == 1,
    };

    // A panel with trims cut off first is cut into those trims and the
    // trimmed panel before anything else; the trimmed panel is then cut as
    // a panel would be.
    let every_trim_cut_first = every_trim_cut_off_first(instance);
    let expected_trims = panel
        .map(|panel| trims_cut_off_first(panel, every_trim_cut_first))
        .unwrap_or_default();
    if expected_trims.is_empty() {
        checker.check_children(root, pattern.first_cuts, "", &mut report);
        return;
    }
    let (trims, rest) = root
        .children
        .split_at(root.children.len().saturating_sub(1));
    let trimmed = rest.first();
    let trims_match = trims.len() == expected_trims.len()
        && trims.iter().zip(&expected_trims).all(|(node, (_, rect))| {
            node.kind == NodeKind::Trim
                && node.stage == 0
                && *rect == rectangle(node)
                && node.children.is_empty()
        });
    let inner = panel.map(|panel| trimmed_panel(panel, every_trim_cut_first));
    let trimmed_matches = trimmed.is_some_and(|node| {
        node.kind == NodeKind::Piece && node.stage == 0 && Some(rectangle(node)) == inner
    });
    if !(trims_match && trimmed_matches) {
        let expected = expected_trims
            .iter()
            .map(|(edge, rect)| {
                let width = if matches!(*edge, "left" | "right") {
                    rect.2
                } else {
                    rect.3
                };
                format!("{edge} {width}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        report("", Problem::WrongTrims { expected });
        return;
    }
    let trimmed = trimmed.expect("checked above");
    let path = child_path("", root.children.len() - 1);
    if trimmed.children.is_empty() {
        report(&path, Problem::PieceWithoutChildren);
    }
    checker.check_children(trimmed, pattern.first_cuts, &path, &mut report);
}

/// A rectangle as (x, y, length, width).
type Rect = (Length, Length, Length, Length);

fn rectangle(node: &Node) -> Rect {
    (node.x, node.y, node.length, node.width)
}

/// The strips cut off first, in the order left, right, bottom, top, and
/// where each is. The left and right ones run the panel's full width; the
/// bottom and top ones run between them.
fn trims_cut_off_first(panel: &Panel) -> Vec<(&'static str, Rect)> {
    let trims = &panel.trims;
    let (length, width) = (panel.length, panel.width);
    let cut = |trim: model::Trim| {
        if trim.is_cut_off_first() {
            trim.width
        } else {
            Length::ZERO
        }
    };
    let (left, right, bottom, top) = (
        cut(trims.left),
        cut(trims.right),
        cut(trims.bottom),
        cut(trims.top),
    );
    let between = length - left - right;
    let mut strips = Vec::new();
    if left > Length::ZERO {
        strips.push(("left", (Length::ZERO, Length::ZERO, left, width)));
    }
    if right > Length::ZERO {
        strips.push(("right", (length - right, Length::ZERO, right, width)));
    }
    if bottom > Length::ZERO {
        strips.push(("bottom", (left, Length::ZERO, between, bottom)));
    }
    if top > Length::ZERO {
        strips.push(("top", (left, width - top, between, top)));
    }
    strips
}

/// The panel without its trims cut off first.
fn trimmed_panel(panel: &Panel) -> Rect {
    let trims = &panel.trims;
    let cut = |trim: model::Trim| {
        if trim.is_cut_off_first() {
            trim.width
        } else {
            Length::ZERO
        }
    };
    let (left, right, bottom, top) = (
        cut(trims.left),
        cut(trims.right),
        cut(trims.bottom),
        cut(trims.top),
    );
    (
        left,
        bottom,
        panel.length - left - right,
        panel.width - bottom - top,
    )
}

struct TreeChecker<'a> {
    instance: &'a Instance,
    /// The pattern's panel, if the instance has it.
    panel: Option<&'a Panel>,
    kerf: Length,
    minimum_offcut_width: Length,
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

            self.check_node(node, index, child, direction, &child_path, report);
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

    /// Checks one child by its kind, and recurses into pieces. `direction`
    /// is that of the cuts that freed it from `parent`.
    fn check_node(
        &self,
        parent: &Node,
        index: usize,
        node: &Node,
        direction: CutDirection,
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
                self.check_children(node, direction.turned(), path, report);
            }
            NodeKind::Part { part_id, rotated } => {
                self.check_part(node, part_id, *rotated, path, report)
            }
            NodeKind::Offcut => self.check_offcut_width(node, "offcut", direction, path, report),
            NodeKind::Leftover => {
                let is_last_first_stage_piece =
                    parent.stage == 0 && index + 1 == parent.children.len();
                if !(self.leftover_allowed && is_last_first_stage_piece) {
                    report(path, Problem::MisplacedLeftover);
                }
                self.check_offcut_width(node, "leftover", direction, path, report);
            }
            NodeKind::Trim => report(path, Problem::MisplacedTrim),
        }
    }

    /// An offcut or leftover is at least the minimum offcut width across
    /// the cut that freed it (`docs/SCOPE.md`, decisions log, 2026-10-27):
    /// its size along the axis its siblings sit on.
    fn check_offcut_width(
        &self,
        node: &Node,
        kind: &'static str,
        direction: CutDirection,
        path: &str,
        report: &mut impl FnMut(&str, Problem),
    ) {
        let width = node.size(direction);
        if width < self.minimum_offcut_width {
            report(
                path,
                Problem::OffcutTooNarrow {
                    kind,
                    width,
                    minimum: self.minimum_offcut_width,
                },
            );
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
        // Every trim, cut off first or left on the offcut, is kept free
        // of parts.
        if let Some(panel) = self.panel {
            let trims = &panel.trims;
            let reaches = [
                ("left", node.x < trims.left.width),
                (
                    "right",
                    node.x + node.length > panel.length - trims.right.width,
                ),
                ("bottom", node.y < trims.bottom.width),
                ("top", node.y + node.width > panel.width - trims.top.width),
            ];
            for (edge, inside) in reaches {
                if inside {
                    report(
                        path,
                        Problem::PartInTrim {
                            part_id: part_id.to_string(),
                            edge,
                        },
                    );
                }
            }
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
