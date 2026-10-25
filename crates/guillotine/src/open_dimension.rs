//! Completing open-dimension plans cut from a shorter panel.
//!
//! Not in upstream: a deliberate deviation (see `docs/TESTING.md`).
//! Upstream solves the open dimensions with unlimited stages on ever
//! shorter panels (`sequential_feasibility`) and copies the last plan that
//! fits onto the full panel (`Solution::append_bin`), which only lengthens
//! the panel itself. The rest of the panel, beyond the used length, is
//! then in no piece of the cutting tree, which the checker rightly
//! rejects: a saw has to cut it off. This adds that cut, and the remainder
//! as a leftover.

use model::{CutDirection, Instance, Length, Node, NodeKind, Solution};

/// Adds the uncut remainder of each open-dimension panel to its plan, as
/// the last first-stage piece. When the first cuts run along the open
/// side, the old plan becomes the first piece of a new first cut across
/// it. Plans that already cover their panel, and other objectives, are
/// left as they are.
pub fn complete_open_dimension(instance: &Instance, solution: &mut Solution) {
    let open_x = match instance.objective {
        model::Objective::OpenDimensionX => true,
        model::Objective::OpenDimensionY => false,
        _ => return,
    };
    let kerf = instance.saw.kerf;
    for pattern in &mut solution.patterns {
        let root = &mut pattern.root;
        if root.children.is_empty() {
            continue;
        }
        // How far the pieces reach along the open side.
        let (start, extent) = if open_x {
            (root.x, root.length)
        } else {
            (root.y, root.width)
        };
        let reach = root
            .children
            .iter()
            .map(|child| {
                if open_x {
                    child.x + child.length
                } else {
                    child.y + child.width
                }
            })
            .max()
            .expect("the root has children");
        let remainder = start + extent - (reach + kerf);
        if remainder <= Length::ZERO {
            continue;
        }

        // First cuts across the open side are already side by side along
        // it: the remainder is one more first-stage piece. Otherwise the
        // old plan is wrapped in a piece first.
        let across = pattern.first_cuts == CutDirection::AcrossLength;
        if across != open_x {
            let mut used = Node {
                kind: NodeKind::Piece,
                x: root.x,
                y: root.y,
                length: if open_x { reach - root.x } else { root.length },
                width: if open_x { root.width } else { reach - root.y },
                stage: 1,
                children: std::mem::take(&mut root.children),
            };
            for child in &mut used.children {
                deepen(child);
            }
            root.children.push(used);
            pattern.first_cuts = pattern.first_cuts.turned();
        }
        let leftover = Node {
            kind: NodeKind::Leftover,
            x: if open_x { reach + kerf } else { root.x },
            y: if open_x { root.y } else { reach + kerf },
            length: if open_x { remainder } else { root.length },
            width: if open_x { root.width } else { remainder },
            stage: 1,
            children: Vec::new(),
        };
        // A previous leftover is no longer the last piece.
        if let Some(last) = root.children.last_mut() {
            if matches!(last.kind, NodeKind::Leftover) {
                last.kind = NodeKind::Offcut;
            }
        }
        root.children.push(leftover);
    }
}

/// One stage deeper, with the whole subtree.
fn deepen(node: &mut Node) {
    node.stage += 1;
    for child in &mut node.children {
        deepen(child);
    }
}
