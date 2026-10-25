//! The solver's view of an instance: integer sizes, resolved quantities and
//! the totals the search needs, in upstream's terms.
//!
//! This is the one place where woodworking names meet upstream's: a panel
//! or part's *length* is upstream's width (`w`, along x), its *width* is
//! upstream's height (`h`, along y), *quantity* is copies, *grain* is
//! oriented and *value* is profit (see `docs/ARCHITECTURE.md`).

use model::{CutType, FirstCuts, Instance, Stages};

/// What a plan optimizes, in upstream's terms.
///
/// upstream: include/packingsolver/algorithms/common.hpp, `Objective`: the
/// four objectives users choose from, plus `Feasibility`, which only the
/// subproblems of upstream's algorithms use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Objective {
    BinPacking,
    Knapsack,
    OpenDimensionX,
    OpenDimensionY,
    /// Cut every part from the panels given: a plan is feasible once every
    /// part is cut, and is then as good as any other.
    Feasibility,
}

impl From<model::Objective> for Objective {
    fn from(objective: model::Objective) -> Self {
        match objective {
            model::Objective::BinPacking => Objective::BinPacking,
            model::Objective::Knapsack => Objective::Knapsack,
            model::Objective::OpenDimensionX => Objective::OpenDimensionX,
            model::Objective::OpenDimensionY => Objective::OpenDimensionY,
        }
    }
}

/// The direction of the first cuts of a panel, as upstream names it.
///
/// upstream: include/packingsolver/rectangleguillotine/instance.hpp, `CutOrientation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    /// Cuts at a fixed x, across the panel's length.
    Vertical,
    /// Cuts at a fixed y, along the panel's length.
    Horizontal,
    /// Either; only used before a direction has been chosen.
    Any,
}

/// A rectangle's size, in tenths of a millimetre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Size {
    /// Along x.
    pub w: i64,
    /// Along y.
    pub h: i64,
}

impl Size {
    /// The size with its sides swapped.
    pub fn flipped(self) -> Size {
        Size {
            w: self.h,
            h: self.w,
        }
    }

    /// Area in hundredths of mm².
    pub fn area(self) -> i64 {
        self.w * self.h
    }

    /// Width along x when turned (`rotate`) or not.
    pub fn width(self, rotate: bool) -> i64 {
        if rotate { self.h } else { self.w }
    }

    /// Height along y when turned (`rotate`) or not.
    pub fn height(self, rotate: bool) -> i64 {
        if rotate { self.w } else { self.h }
    }
}

/// A part type (upstream `ItemType`). Each part type is its own stack in
/// upstream's terms: stack `j` holds the copies of part type `j`.
#[derive(Debug, Clone)]
pub(crate) struct ItemType {
    /// Size in the instance's own coordinates.
    pub size: Size,
    /// Whether the part keeps its grain direction: never turned.
    pub oriented: bool,
    /// How many copies to cut (at most, for the knapsack objective).
    pub copies: u32,
    /// Value of one copy.
    pub profit: f64,
}

/// A panel type (upstream `BinType`).
#[derive(Debug, Clone)]
pub(crate) struct BinType {
    /// Size in the instance's own coordinates.
    pub size: Size,
    /// How many panels of this type there are, resolved: "as many as
    /// needed" is one per part, as upstream does.
    pub copies: u32,
    /// Cost of one panel (by default its area in mm²).
    pub cost: f64,
}

/// The saw settings of a problem (upstream `Parameters`, for the In
/// features).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings {
    pub number_of_stages: u32,
    pub cut_type: CutType,
    pub first_stage_orientation: Orientation,
    pub cut_thickness: i64,
}

/// An instance ready for the search.
#[derive(Debug, Clone)]
pub(crate) struct Problem {
    pub objective: Objective,
    /// 2 or 3, or for unlimited stages the largest length plus width of a
    /// panel type: no cutting tree can be deeper (upstream's convention).
    pub number_of_stages: u32,
    pub cut_type: CutType,
    pub first_stage_orientation: Orientation,
    /// Saw kerf, in tenths of a millimetre.
    pub cut_thickness: i64,
    pub item_types: Vec<ItemType>,
    pub bin_types: Vec<BinType>,
    /// Total number of part copies.
    pub number_of_items: u32,
    /// Total area of the part copies.
    pub item_area: i64,
    /// Total value of the part copies.
    pub item_profit: f64,
    /// Total area of the panels.
    pub bin_area: i64,
    /// Total number of panels.
    pub number_of_bins: u32,
    /// The part type with the highest value per area (the first one on
    /// ties).
    pub largest_efficiency_item_type: usize,
    /// upstream `number_of_stacks()`. Each part type is its own stack, but
    /// a subproblem's part types keep the stack numbers of the problem they
    /// come from, so a subproblem without the first part types has more
    /// stacks than part types, which changes upstream's choice of
    /// algorithm (see `selection.rs`).
    pub number_of_stacks: usize,
}

impl Problem {
    /// Converts a valid instance.
    ///
    /// upstream: src/rectangleguillotine/instance_builder.cpp, `InstanceBuilder::build`,
    /// for the derived totals; tools/oracle/main.cpp for the field mapping.
    pub fn new(instance: &Instance) -> Problem {
        let first_stage_orientation = match instance.saw.first_cuts {
            FirstCuts::Either => Orientation::Any,
            FirstCuts::AlongLength => Orientation::Horizontal,
            FirstCuts::AcrossLength => Orientation::Vertical,
        };

        let item_types: Vec<ItemType> = instance
            .parts
            .iter()
            .map(|part| ItemType {
                size: Size {
                    w: part.length.tenths(),
                    h: part.width.tenths(),
                },
                oriented: part.grain,
                copies: part.quantity,
                profit: part.effective_value(),
            })
            .collect();
        let number_of_items: u32 = item_types.iter().map(|item| item.copies).sum();

        let bin_types: Vec<BinType> = instance
            .panels
            .iter()
            .map(|panel| BinType {
                size: Size {
                    w: panel.length.tenths(),
                    h: panel.width.tenths(),
                },
                // Open dimension objectives have a single panel; upstream
                // rejects any other count.
                copies: if instance.objective.is_open_dimension() {
                    1
                } else {
                    panel.quantity.unwrap_or(number_of_items)
                },
                cost: panel.effective_cost(),
            })
            .collect();

        let (number_of_stages, cut_type, first_stage_orientation) = match instance.saw.stages {
            Stages::Limited(count) => (count, instance.saw.cut_type, first_stage_orientation),
            // upstream: unlimited stages are exact cuts in either direction,
            // whatever the cut type and first cuts the instance gives.
            Stages::Unlimited => {
                let deepest = bin_types
                    .iter()
                    .map(|bin| (bin.size.w + bin.size.h) as u32)
                    .max()
                    .unwrap_or(0);
                (deepest, CutType::Exact, Orientation::Any)
            }
        };

        Problem::from_types(
            instance.objective.into(),
            Settings {
                number_of_stages,
                cut_type,
                first_stage_orientation,
                cut_thickness: instance.saw.kerf.tenths(),
            },
            item_types,
            bin_types,
        )
    }

    /// A problem from its part and panel types, with the totals derived as
    /// upstream's `InstanceBuilder::build` derives them.
    pub fn from_types(
        objective: Objective,
        settings: Settings,
        item_types: Vec<ItemType>,
        bin_types: Vec<BinType>,
    ) -> Problem {
        let number_of_items: u32 = item_types.iter().map(|item| item.copies).sum();
        let mut item_area = 0;
        let mut item_profit = 0.0;
        let mut largest_efficiency_item_type = 0;
        for (index, item) in item_types.iter().enumerate() {
            item_area += i64::from(item.copies) * item.size.area();
            // upstream: `copies * profit`, summed in order, so the total
            // compares equal to upstream's.
            item_profit += f64::from(item.copies) * item.profit;
            let best = &item_types[largest_efficiency_item_type];
            let efficiency = |item: &ItemType| item.profit / item.size.area() as f64;
            if efficiency(best) < efficiency(item) {
                largest_efficiency_item_type = index;
            }
        }
        let bin_area = bin_types
            .iter()
            .map(|bin| i64::from(bin.copies) * bin.size.area())
            .sum();
        let number_of_bins = bin_types.iter().map(|bin| bin.copies).sum();
        let item_types_len = item_types.len();

        Problem {
            objective,
            number_of_stages: settings.number_of_stages,
            cut_type: settings.cut_type,
            first_stage_orientation: settings.first_stage_orientation,
            cut_thickness: settings.cut_thickness,
            item_types,
            bin_types,
            number_of_items,
            item_area,
            item_profit,
            bin_area,
            number_of_bins,
            largest_efficiency_item_type,
            number_of_stacks: item_types_len,
        }
    }

    /// The saw settings, to build subproblems with the same ones
    /// (upstream `InstanceBuilder::set_parameters`).
    pub fn settings(&self) -> Settings {
        Settings {
            number_of_stages: self.number_of_stages,
            cut_type: self.cut_type,
            first_stage_orientation: self.first_stage_orientation,
            cut_thickness: self.cut_thickness,
        }
    }

    /// Whether the number of stages is unlimited: no panel type could be
    /// cut deeper.
    ///
    /// upstream: instance.hpp, `Instance::number_of_stages_unlimited`.
    pub fn unlimited_stages(&self) -> bool {
        let deepest = self
            .bin_types
            .iter()
            .map(|bin| bin.size.w + bin.size.h)
            .max()
            .unwrap_or(0);
        i64::from(self.number_of_stages) >= deepest
    }

    /// The panel type of the panel at `position`, panels taken in the
    /// order listed (upstream `Instance::bin_type_id`).
    pub fn bin_type_of_position(&self, position: u32) -> usize {
        let mut remaining = position;
        for (bin_type, bin) in self.bin_types.iter().enumerate() {
            if remaining < bin.copies {
                return bin_type;
            }
            remaining -= bin.copies;
        }
        panic!("panel position {position} beyond the last panel")
    }

    /// The size of a part type, seen with first cuts in `orientation`:
    /// horizontal first cuts are searched on the flipped instance, where
    /// every length and width is swapped.
    ///
    /// upstream: src/rectangleguillotine/instance_flipper.cpp, `InstanceFlipper::flip`.
    pub fn item_size(&self, item_type: usize, orientation: Orientation) -> Size {
        let size = self.item_types[item_type].size;
        if orientation == Orientation::Vertical {
            size
        } else {
            size.flipped()
        }
    }

    /// The size of a panel type, seen with first cuts in `orientation`
    /// (see [`Problem::item_size`]).
    pub fn bin_size(&self, bin_type: usize, orientation: Orientation) -> Size {
        let size = self.bin_types[bin_type].size;
        if orientation == Orientation::Vertical {
            size
        } else {
            size.flipped()
        }
    }
}
