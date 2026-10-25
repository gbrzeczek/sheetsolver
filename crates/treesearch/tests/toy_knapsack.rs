//! The beam search must reproduce upstream's exactly on a toy knapsack
//! scheme: same node counters, same best solution, same callbacks.
//!
//! The scheme mirrors `tools/oracle/treesearch_toy.cpp`; keep them in sync.
//! Reference outputs: `data/treesearch/golden/` (see `tools/treesearch_toy.py`).

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde_json::{Value, json};
use treesearch::{BranchingScheme, Guide, Parameters, Stop, iterative_beam_search};

struct Item {
    weight: i64,
    profit: i64,
}

struct Node {
    parent: Option<Rc<Node>>,
    id: u64,
    /// Items before `next` have been decided.
    next: usize,
    /// Item added by this node; `None` for the root.
    item: Option<usize>,
    weight: i64,
    profit: i64,
}

struct ToyKnapsack {
    capacity: i64,
    items: Vec<Item>,
    /// Numbers nodes in creation order, like upstream's `node_id_`.
    next_id: Cell<u64>,
}

impl ToyKnapsack {
    fn new_id(&self) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }

    fn fits(&self, node: &Node, item: &Item) -> bool {
        node.weight + item.weight <= self.capacity
    }
}

impl BranchingScheme for ToyKnapsack {
    type Node = Rc<Node>;
    type Priority = (Guide, u64);
    type DominanceKey = usize;

    fn root(&self) -> Rc<Node> {
        Rc::new(Node {
            parent: None,
            id: self.new_id(),
            next: 0,
            item: None,
            weight: 0,
            profit: 0,
        })
    }

    fn children(&self, parent: &Rc<Node>) -> Vec<Rc<Node>> {
        (parent.next..self.items.len())
            .filter(|&j| self.fits(parent, &self.items[j]))
            .map(|j| {
                Rc::new(Node {
                    parent: Some(parent.clone()),
                    id: self.new_id(),
                    next: j + 1,
                    item: Some(j),
                    weight: parent.weight + self.items[j].weight,
                    profit: parent.profit + self.items[j].profit,
                })
            })
            .collect()
    }

    fn depth(&self, node: &Rc<Node>) -> Option<usize> {
        Some(node.next)
    }

    fn is_leaf(&self, node: &Rc<Node>) -> bool {
        !self.items[node.next..]
            .iter()
            .any(|item| self.fits(node, item))
    }

    fn priority(&self, node: &Rc<Node>) -> (Guide, u64) {
        (
            Guide::new(-(node.profit as f64) / (node.weight + 1) as f64),
            node.id,
        )
    }

    fn is_better(&self, node: &Rc<Node>, other: &Rc<Node>) -> bool {
        node.profit > other.profit
    }

    fn cannot_beat(&self, node: &Rc<Node>, incumbent: &Rc<Node>) -> bool {
        let upper_bound: i64 = node.profit
            + self.items[node.next..]
                .iter()
                .filter(|item| self.fits(node, item))
                .map(|item| item.profit)
                .sum::<i64>();
        upper_bound <= incumbent.profit
    }

    fn dominance_key(&self, node: &Rc<Node>) -> Option<usize> {
        (node.next % 3 != 0).then_some(node.next)
    }

    fn dominates(&self, node: &Rc<Node>, other: &Rc<Node>) -> bool {
        node.weight <= other.weight && node.profit >= other.profit
    }
}

fn node_json(node: &Rc<Node>) -> Value {
    let mut items = Vec::new();
    let mut current = Some(node);
    while let Some(n) = current {
        if let Some(item) = n.item {
            items.insert(0, item);
        }
        current = n.parent.as_ref();
    }
    json!({ "profit": node.profit, "weight": node.weight, "items": items })
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/treesearch")
}

fn load(instance: &Path) -> ToyKnapsack {
    let json: Value = serde_json::from_str(&fs::read_to_string(instance).unwrap()).unwrap();
    ToyKnapsack {
        capacity: json["capacity"].as_i64().unwrap(),
        items: json["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| Item {
                weight: item["weight"].as_i64().unwrap(),
                profit: item["profit"].as_i64().unwrap(),
            })
            .collect(),
        next_id: Cell::new(0),
    }
}

/// Runs the search with the golden file's parameters and returns the same
/// JSON the C++ reference writes.
fn run(scheme: &ToyKnapsack, golden: &Value) -> Value {
    let p = &golden["parameters"];
    let maximum_nodes_expanded = p["maximum_nodes_expanded"].as_i64().unwrap();
    let parameters = Parameters {
        minimum_queue_size: p["minimum_queue_size"].as_u64().unwrap(),
        growth_factor: p["growth_factor"].as_f64().unwrap(),
        maximum_queue_size: p["maximum_queue_size"].as_u64().unwrap(),
        maximum_nodes_expanded: (maximum_nodes_expanded >= 0)
            .then_some(maximum_nodes_expanded as u64),
        stop: Stop::default(),
    };
    let mut callbacks = Vec::new();
    let output = iterative_beam_search(scheme, &parameters, |output| {
        callbacks.push(json!({
            "queue_size": output.queue_size,
            "optimal": output.optimal,
            "best": node_json(&output.best),
        }));
    });
    json!({
        "parameters": golden["parameters"],
        "best": node_json(&output.best),
        "optimal": output.optimal,
        "final_queue_size": output.queue_size,
        "nodes_generated": output.nodes_generated,
        "nodes_added": output.nodes_added,
        "nodes_processed": output.nodes_processed,
        "nodes_expanded": output.nodes_expanded,
        "callbacks": callbacks,
    })
}

#[test]
fn reproduces_upstream_on_every_toy_instance() {
    let mut golden_files: Vec<PathBuf> = fs::read_dir(data_dir().join("golden"))
        .unwrap()
        .flat_map(|dir| fs::read_dir(dir.unwrap().path()).unwrap())
        .map(|entry| entry.unwrap().path())
        .collect();
    golden_files.sort();
    assert!(!golden_files.is_empty(), "no golden files");

    let mut failures = Vec::new();
    for path in &golden_files {
        let name = path.file_name().unwrap().to_str().unwrap();
        let instance_name = name.split('.').next().unwrap();
        let scheme = load(
            &data_dir()
                .join("instances")
                .join(format!("{instance_name}.json")),
        );
        let golden: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        let ours = run(&scheme, &golden);
        if ours != golden {
            let differing: Vec<&str> = golden
                .as_object()
                .unwrap()
                .keys()
                .filter(|key| ours[key.as_str()] != golden[key.as_str()])
                .map(String::as_str)
                .collect();
            failures.push(format!("{name}: {differing:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        golden_files.len(),
        failures.join("\n")
    );
}

fn large_instance() -> ToyKnapsack {
    load(&data_dir().join("instances/large.json"))
}

#[test]
fn stops_when_cancelled_from_a_callback() {
    let scheme = large_instance();
    let cancel = Arc::new(AtomicBool::new(false));
    let parameters = Parameters {
        stop: Stop {
            deadline: None,
            cancel: Some(cancel.clone()),
            ..Stop::default()
        },
        ..Parameters::default()
    };
    let mut solutions = 0;
    let output = iterative_beam_search(&scheme, &parameters, |_| {
        solutions += 1;
        if solutions == 3 {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    // Children of the node being expanded are still processed, so more
    // solutions may arrive, but the next expansion is refused.
    assert!(solutions >= 3);
    assert!(!output.optimal);
    let unlimited = iterative_beam_search(&large_instance(), &Parameters::default(), |_| {});
    assert!(output.nodes_expanded < unlimited.nodes_expanded / 10);
}

#[test]
fn stops_at_the_deadline() {
    let scheme = large_instance();
    let parameters = Parameters {
        stop: Stop {
            deadline: Some(Instant::now()),
            cancel: None,
            ..Stop::default()
        },
        ..Parameters::default()
    };
    let output = iterative_beam_search(&scheme, &parameters, |_| {});
    // The root is taken from the queue, then the deadline stops the search
    // before its first expansion.
    assert_eq!((output.nodes_processed, output.nodes_expanded), (1, 0));
    assert_eq!(output.best.profit, 0);
}

#[test]
fn fixed_queue_size_matches_upstream_not_anytime_settings() {
    let parameters = Parameters::fixed_queue_size(64);
    assert_eq!(
        (
            parameters.minimum_queue_size,
            parameters.growth_factor,
            parameters.maximum_queue_size
        ),
        (1, 64.0, 65)
    );
}
