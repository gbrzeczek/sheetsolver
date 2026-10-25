//! The solver's view of an instance: integer sizes, resolved quantities and
//! the totals the search needs, in upstream's terms.
//!
//! This is the one place where woodworking names meet upstream's: a panel
//! or part's *length* is upstream's width (`w`, along x), its *width* is
//! upstream's height (`h`, along y), *quantity* is copies, *grain* is
//! oriented and *value* is profit (see `docs/ARCHITECTURE.md`).

use model::{CutType, FirstCuts, Instance, Objective, Stages};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// An instance ready for the search.
#[derive(Debug, Clone)]
pub(crate) struct Problem {
    pub objective: Objective,
    /// 2 or 3.
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
}

impl Problem {
    /// Converts a valid instance with 2 or 3 stages.
    ///
    /// upstream: src/rectangleguillotine/instance_builder.cpp, `InstanceBuilder::build`,
    /// for the derived totals; tools/oracle/main.cpp for the field mapping.
    pub fn new(instance: &Instance) -> Problem {
        let number_of_stages = match instance.saw.stages {
            Stages::Limited(count) => count,
            Stages::Unlimited => unreachable!("unlimited stages are solved elsewhere"),
        };
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
            })
            .collect();
        let bin_area = bin_types
            .iter()
            .map(|bin| i64::from(bin.copies) * bin.size.area())
            .sum();
        let number_of_bins = bin_types.iter().map(|bin| bin.copies).sum();

        Problem {
            objective: instance.objective,
            number_of_stages,
            cut_type: instance.saw.cut_type,
            first_stage_orientation,
            cut_thickness: instance.saw.kerf.tenths(),
            item_types,
            bin_types,
            number_of_items,
            item_area,
            item_profit,
            bin_area,
            number_of_bins,
            largest_efficiency_item_type,
        }
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
