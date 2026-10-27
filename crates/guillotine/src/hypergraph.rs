//! The hypergraph search with infinite copies: one panel, unlimited
//! stages, value objective.
//!
//! upstream: src/rectangleguillotine/tree_search_hypergraph_infinite_copies.cpp.
//!
//! Dynamic programming over the sub-rectangles of the panel whose sides
//! are *raster points* (the lengths a combination of parts and kerfs can
//! leave), ignoring how many copies each part has. The best value of a
//! rectangle is the best of: a smaller rectangle, one part, or two
//! rectangles side by side. A plan is read back from the table, and only
//! kept if it respects the part quantities; when it does not, the program
//! is rerun with fewer part types (the most valuable per area, or the ones
//! whose copies cannot run out).

use treesearch::Stop;

use crate::cpp_sort;
use crate::plan::{Plan, PlanBuilder};
use crate::problem::{Orientation, Problem};
use crate::restage::minimize_number_of_stages;

/// upstream: optimize.hpp,
/// `not_anytime_tree_search_hypergraph_infinite_copies_maximum_array_size`.
const MAXIMUM_ARRAY_SIZE: usize = 1_500_000;

/// The positions along one side of the panel that the program uses, and
/// for every length, the largest of them not larger than it.
///
/// upstream: `NormalPositions`.
struct Positions {
    /// Increasing; the first is 0.
    positions: Vec<i64>,
    /// `floor[x]`: the index of the largest position not larger than `x`.
    floor: Vec<usize>,
}

fn floors(positions: &[i64], length: i64) -> Vec<usize> {
    // Every length from one position up to the next has the same floor:
    // fill each such run at once (upstream assigns them one by one).
    let mut floor = vec![0; length as usize + 1];
    for (index, &start) in positions.iter().enumerate() {
        let end = positions
            .get(index + 1)
            .map_or(length + 1, |&next| next.min(length + 1));
        if start <= length {
            floor[start as usize..end as usize].fill(index);
        }
    }
    floor
}

/// The lengths of the combinations of `dimensions`, a kerf between two,
/// up to `length` (Herz 1972).
///
/// upstream: `compute_normal_positions`.
fn normal_positions(dimensions: &[i64], length: i64, cut_thickness: i64) -> Positions {
    let end = (length + cut_thickness) as usize;
    let mut reached = vec![false; end + 1];
    reached[0] = true;
    for x in 0..=end {
        if !reached[x] {
            continue;
        }
        for &dimension in dimensions {
            let next = x + (dimension + cut_thickness) as usize;
            if next <= end {
                reached[next] = true;
            }
        }
    }
    let mut positions = vec![0];
    for (x, _) in reached
        .iter()
        .enumerate()
        .skip(cut_thickness as usize + 1)
        .filter(|(_, reached)| **reached)
    {
        positions.push(x as i64 - cut_thickness);
    }
    let floor = floors(&positions, length);
    Positions { positions, floor }
}

/// What is left of `length` after a normal position and a kerf, rounded
/// down to a normal position (Scheithauer and Terno, 1996).
///
/// upstream: `compute_raster_points`.
fn raster_points(normal: &Positions, length: i64, cut_thickness: i64) -> Positions {
    let mut is_raster_point = vec![false; length as usize + 1];
    is_raster_point[0] = true;
    is_raster_point[normal.positions[normal.floor[length as usize]] as usize] = true;
    for &position in &normal.positions {
        if position == 0 {
            continue;
        }
        let rest = length - position - cut_thickness;
        if rest < 0 {
            break;
        }
        is_raster_point[normal.positions[normal.floor[rest as usize]] as usize] = true;
    }
    let positions: Vec<i64> = (0..=length)
        .filter(|&x| is_raster_point[x as usize])
        .collect();
    let floor = floors(&positions, length);
    Positions { positions, floor }
}

/// The raster points along x and y with the part types in `used`.
///
/// upstream: `compute_positions`.
fn positions(problem: &Problem, used: &[bool]) -> (Positions, Positions) {
    let size = problem.bin_types[0].size;
    let mut along_x = Vec::new();
    let mut along_y = Vec::new();
    for (item, _) in problem
        .item_types
        .iter()
        .zip(used)
        .filter(|(_, used)| **used)
    {
        along_x.push(item.size.w);
        along_y.push(item.size.h);
        if !item.oriented {
            along_x.push(item.size.h);
            along_y.push(item.size.w);
        }
    }
    let cut_thickness = problem.cut_thickness;
    (
        raster_points(
            &normal_positions(&along_x, size.w, cut_thickness),
            size.w,
            cut_thickness,
        ),
        raster_points(
            &normal_positions(&along_y, size.h, cut_thickness),
            size.h,
            cut_thickness,
        ),
    )
}

/// The table: the best value of each sub-rectangle whose sides are
/// raster points.
///
/// upstream: `DynamicProgram`.
struct Table {
    x: Positions,
    y: Positions,
    /// `values[ix + x.positions.len() * iy]`.
    values: Vec<f64>,
}

impl Table {
    /// The value of a rectangle: that of the largest one with raster-point
    /// sides it contains.
    fn value(&self, width: i64, height: i64) -> f64 {
        self.values
            [self.x.floor[width as usize] + self.x.positions.len() * self.y.floor[height as usize]]
    }
}

/// Fills the table, or returns `None` if stopped.
///
/// upstream: `compute_dp`. A cut is only tried at a breakpoint of its
/// smaller side, where the value increases (Russo et al., 2014).
fn compute_table(problem: &Problem, used: &[bool], stop: &Stop) -> Option<Table> {
    let cut_thickness = problem.cut_thickness;
    let (x, y) = positions(problem, used);
    let (nx, ny) = (x.positions.len(), y.positions.len());
    let mut values = vec![0.0f64; nx * ny];
    let mut breakpoints_x: Vec<Vec<usize>> = vec![Vec::new(); ny];
    let mut breakpoints_y: Vec<Vec<usize>> = vec![Vec::new(); nx];
    // The splits along y read the table by columns: a transposed copy.
    let mut transposed = vec![0.0f64; nx * ny];
    // Only the part types in use, gathered once rather than at every cell.
    let items: Vec<_> = problem
        .item_types
        .iter()
        .zip(used)
        .filter(|(_, used)| **used)
        .map(|(item, _)| item)
        .collect();

    for iy in 1..ny {
        if stop.requested() {
            return None;
        }
        let height = y.positions[iy];
        for ix in 1..nx {
            let width = x.positions[ix];
            let mut value = cpp_max(values[ix - 1 + nx * iy], values[ix + nx * (iy - 1)]);

            for item in &items {
                if item.size.w <= width && item.size.h <= height {
                    value = cpp_max(value, item.profit);
                }
                if !item.oriented && item.size.h <= width && item.size.w <= height {
                    value = cpp_max(value, item.profit);
                }
            }

            for &ic in &breakpoints_x[iy] {
                let cut = x.positions[ic];
                if 2 * cut + cut_thickness > width {
                    break;
                }
                let rest = x.floor[(width - cut - cut_thickness) as usize];
                value = cpp_max(value, values[ic + nx * iy] + values[rest + nx * iy]);
            }

            let column = &transposed[ny * ix..ny * (ix + 1)];
            for &ic in &breakpoints_y[ix] {
                let cut = y.positions[ic];
                if 2 * cut + cut_thickness > height {
                    break;
                }
                let rest = y.floor[(height - cut - cut_thickness) as usize];
                value = cpp_max(value, column[ic] + column[rest]);
            }
            let below = column[iy - 1];
            transposed[iy + ny * ix] = value;
            values[ix + nx * iy] = value;

            if value > values[ix - 1 + nx * iy] {
                breakpoints_x[iy].push(ix);
            }
            if value > below {
                breakpoints_y[ix].push(iy);
            }
        }
    }
    Some(Table { x, y, values })
}

/// `std::max(a, b)` on values that are never NaN: `f64::max` gives the
/// same result but pays for NaN handling in the innermost loop.
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// Reads a rectangle's plan back from the table: one part in its corner,
/// or a cut in this depth's direction into two rectangles, or a cut that
/// changes nothing (to cut the other way one stage down).
///
/// upstream: `reconstruct_rec`.
struct Reader<'a> {
    problem: &'a Problem,
    used: &'a [bool],
    table: &'a Table,
    orientation: Orientation,
}

impl Reader<'_> {
    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        builder: &mut PlanBuilder,
        width: i64,
        height: i64,
        depth: u32,
        x: i64,
        y: i64,
        pass_through: bool,
    ) {
        let table = self.table;
        let cut_thickness = self.problem.cut_thickness;
        let value = table.value(width, height);
        if value == 0.0 {
            return;
        }
        let vertical = (self.orientation == Orientation::Vertical) == (depth % 2 == 1);

        for (item_type, item) in self.problem.item_types.iter().enumerate() {
            if !self.used[item_type] || item.profit != value {
                continue;
            }
            let (item_width, item_height) = if item.size.w <= width && item.size.h <= height {
                (item.size.w, item.size.h)
            } else if !item.oriented && item.size.h <= width && item.size.w <= height {
                (item.size.h, item.size.w)
            } else {
                continue;
            };
            if item_width == width && item_height == height {
                builder.set_last_node_item(item_type);
                return;
            }
            if vertical {
                builder.add_node(depth, x + item_width);
                if item_height < height {
                    builder.add_node(depth + 1, y + item_height);
                }
            } else {
                builder.add_node(depth, y + item_height);
                if item_width < width {
                    builder.add_node(depth + 1, x + item_width);
                }
            }
            builder.set_last_node_item(item_type);
            return;
        }

        // The rectangle's side and position along the cut direction, and
        // the value of a rectangle of side `side` there.
        let (side, start, positions) = if vertical {
            (width, x, &table.x.positions)
        } else {
            (height, y, &table.y.positions)
        };
        let part = |side_along: i64| {
            if vertical {
                table.value(side_along, height)
            } else {
                table.value(width, side_along)
            }
        };
        let shifted = |offset: i64| {
            if vertical {
                (x + offset, y)
            } else {
                (x, y + offset)
            }
        };
        let size = |side_along: i64| {
            if vertical {
                (side_along, height)
            } else {
                (width, side_along)
            }
        };
        for &cut in positions {
            if cut == 0 {
                continue;
            }
            if cut >= side {
                break;
            }
            if cut + cut_thickness >= side {
                // The rest is too small for a cut: it is waste.
                if part(cut) == value {
                    builder.add_node(depth, start + cut);
                    let (w, h) = size(cut);
                    self.read(builder, w, h, depth + 1, x, y, false);
                    return;
                }
                break;
            }
            let rest = side - cut - cut_thickness;
            if part(cut) + part(rest) == value {
                builder.add_node(depth, start + cut);
                let (w, h) = size(cut);
                self.read(builder, w, h, depth + 1, x, y, false);
                builder.add_node(depth, start + side);
                let (w, h) = size(rest);
                let (rest_x, rest_y) = shifted(cut + cut_thickness);
                self.read(builder, w, h, depth + 1, rest_x, rest_y, false);
                return;
            }
        }
        if pass_through {
            return;
        }
        builder.add_node(depth, start + side);
        self.read(builder, width, height, depth + 1, x, y, true);
    }
}

/// The plan of a table.
///
/// upstream: `reconstruct_solution`, without trims.
fn read_plan(problem: &Problem, used: &[bool], table: &Table) -> Plan {
    let size = problem.bin_types[0].size;
    let orientation = match problem.first_stage_orientation {
        Orientation::Any => Orientation::Vertical,
        orientation => orientation,
    };
    let mut builder = PlanBuilder::new(problem);
    builder.add_bin(0, 1, orientation);
    if table.value(size.w, size.h) > 0.0 {
        builder.add_node(1, size.w);
        let reader = Reader {
            problem,
            used,
            table,
            orientation,
        };
        reader.read(&mut builder, size.w, size.h, 2, 0, 0, false);
    }
    builder.build()
}

/// The search: the best plan found, possibly the empty one.
///
/// upstream: `tree_search_hypergraph_infinite_copies` with upstream's
/// not-anytime maximum array size.
pub(crate) fn tree_search_hypergraph_infinite_copies(problem: &Problem, stop: &Stop) -> Plan {
    let mut best = Plan::empty(problem);
    let consider = |plan: Plan, best: &mut Plan| {
        let plan = minimize_number_of_stages(problem, &plan);
        // The copies of the part types may not be respected.
        if plan.feasible && plan.is_better_than(best, problem.objective) {
            *best = plan;
        }
    };
    let number_of_item_types = problem.item_types.len();

    // The part types of value, the most valuable per area first.
    let mut sorted: Vec<usize> = (0..number_of_item_types)
        .filter(|&item_type| problem.item_types[item_type].profit > 0.0)
        .collect();
    cpp_sort::sort(&mut sorted, |&a, &b| {
        let (a, b) = (&problem.item_types[a], &problem.item_types[b]);
        a.profit * b.size.area() as f64 > b.profit * a.size.area() as f64
    });
    let first = |count: usize| {
        let mut used = vec![false; number_of_item_types];
        for &item_type in &sorted[..count] {
            used[item_type] = true;
        }
        used
    };

    // How many of the first part types fit in an array size, with the
    // array size of each count computed once.
    let mut array_sizes: Vec<usize> = Vec::new();
    let mut fitting = |maximum: usize| {
        let mut count = 0;
        while count < sorted.len() {
            if count == array_sizes.len() {
                let (x, y) = positions(problem, &first(count + 1));
                array_sizes.push(x.positions.len() * y.positions.len());
            }
            if array_sizes[count] > maximum {
                break;
            }
            count += 1;
        }
        count
    };
    // A quarter of the maximum size, then the maximum size, then all the
    // part types if they fit.
    let mut counts: Vec<usize> = Vec::new();
    let mut all = true;
    let mut maximum = MAXIMUM_ARRAY_SIZE / 4;
    loop {
        let mut last = false;
        if maximum >= MAXIMUM_ARRAY_SIZE {
            maximum = MAXIMUM_ARRAY_SIZE;
            last = true;
        }
        let count = fitting(maximum);
        if count == sorted.len() {
            break;
        }
        if count > 0 && counts.last().is_none_or(|&previous| count > previous) {
            counts.push(count);
        }
        if last {
            all = false;
            break;
        }
        maximum *= 4;
    }
    for &count in &counts {
        let used = first(count);
        let Some(table) = compute_table(problem, &used, stop) else {
            return best;
        };
        consider(read_plan(problem, &used, &table), &mut best);
    }
    if !all {
        return best;
    }

    let every = vec![true; number_of_item_types];
    let Some(table) = compute_table(problem, &every, stop) else {
        return best;
    };
    let plan = minimize_number_of_stages(problem, &read_plan(problem, &every, &table));
    if plan.feasible {
        if plan.is_better_than(&best, problem.objective) {
            best = plan;
        }
        return best;
    }

    // Otherwise, only the part types whose copies cannot run out in the
    // panel.
    let area = problem.bin_types[0].area();
    let infinite: Vec<bool> = problem
        .item_types
        .iter()
        .map(|item| i64::from(item.copies) >= area / item.size.area())
        .collect();
    if infinite.iter().all(|&infinite| infinite) || !infinite.iter().any(|&infinite| infinite) {
        return best;
    }
    let Some(table) = compute_table(problem, &infinite, stop) else {
        return best;
    };
    consider(read_plan(problem, &infinite, &table), &mut best);
    best
}
