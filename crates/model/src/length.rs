//! Lengths and areas.
//!
//! Users enter millimetres with at most one decimal. Inside the code every
//! length is a whole number of tenths of a millimetre, so geometry is exact
//! integer arithmetic and 3.2 mm kerfs need no floats.

use std::fmt;
use std::ops::{Add, Sub};

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// Tenths of a millimetre in one millimetre.
const TENTHS_PER_MM: i64 = 10;

/// A length, stored as a whole number of tenths of a millimetre.
///
/// In JSON it is a number of millimetres with at most one decimal
/// (`720`, `3.2`). More decimals are rejected rather than rounded, so a
/// typo never silently changes a cutting plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Length(i64);

impl Length {
    /// Zero length.
    pub const ZERO: Length = Length(0);

    /// The longest length accepted: 100 m, far beyond any sheet. Areas are
    /// `i64` products of two lengths, summed over many panels; without a
    /// bound, a huge size overflows them into nonsense plans.
    pub const MAX: Length = Length::from_mm(100_000);

    /// A length of `tenths` tenths of a millimetre.
    pub const fn from_tenths(tenths: i64) -> Self {
        Length(tenths)
    }

    /// A length of a whole number of millimetres.
    pub const fn from_mm(mm: i64) -> Self {
        Length(mm * TENTHS_PER_MM)
    }

    /// Converts millimetres with at most one decimal, or returns `None` if
    /// the value has more decimals or is not finite.
    pub fn try_from_mm(mm: f64) -> Option<Self> {
        let tenths = mm * TENTHS_PER_MM as f64;
        let rounded = tenths.round();
        // Tolerance for binary floating point: 3.2 * 10 is 32.000000000000004.
        if !tenths.is_finite() || (tenths - rounded).abs() > 1e-6 {
            return None;
        }
        Some(Length(rounded as i64))
    }

    /// The length in tenths of a millimetre.
    pub const fn tenths(self) -> i64 {
        self.0
    }

    /// The length in millimetres.
    pub fn mm(self) -> f64 {
        self.0 as f64 / TENTHS_PER_MM as f64
    }

    /// The area of a `self` × `other` rectangle.
    pub const fn times(self, other: Length) -> Area {
        Area(self.0 * other.0)
    }
}

impl Add for Length {
    type Output = Length;
    fn add(self, other: Length) -> Length {
        Length(self.0 + other.0)
    }
}

impl Sub for Length {
    type Output = Length;
    fn sub(self, other: Length) -> Length {
        Length(self.0 - other.0)
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 % TENTHS_PER_MM == 0 {
            write!(f, "{} mm", self.0 / TENTHS_PER_MM)
        } else {
            write!(f, "{:.1} mm", self.mm())
        }
    }
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Whole millimetres are written as integers (`720`, not `720.0`).
        if self.0 % TENTHS_PER_MM == 0 {
            serializer.serialize_i64(self.0 / TENTHS_PER_MM)
        } else {
            serializer.serialize_f64(self.mm())
        }
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mm = f64::deserialize(deserializer)?;
        Length::try_from_mm(mm).ok_or_else(|| {
            de::Error::custom(format!(
                "length {mm} mm has more than one decimal; lengths are in steps of 0.1 mm"
            ))
        })
    }
}

/// An area, stored as a whole number of hundredths of a square millimetre
/// (the product of two [`Length`]s).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Area(i64);

impl Area {
    /// Zero area.
    pub const ZERO: Area = Area(0);

    /// The area in hundredths of a square millimetre.
    pub const fn hundredths_mm2(self) -> i64 {
        self.0
    }

    /// The area in square millimetres.
    pub fn mm2(self) -> f64 {
        self.0 as f64 / (TENTHS_PER_MM * TENTHS_PER_MM) as f64
    }

    /// The area repeated `count` times.
    pub const fn times(self, count: u32) -> Area {
        Area(self.0 * count as i64)
    }
}

impl Add for Area {
    type Output = Area;
    fn add(self, other: Area) -> Area {
        Area(self.0 + other.0)
    }
}

impl Sub for Area {
    type Output = Area;
    fn sub(self, other: Area) -> Area {
        Area(self.0 - other.0)
    }
}

impl std::iter::Sum for Area {
    fn sum<I: Iterator<Item = Area>>(iter: I) -> Area {
        iter.fold(Area::ZERO, Add::add)
    }
}

impl Serialize for Area {
    /// Areas are written in square millimetres.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.mm2())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_whole_and_one_decimal_millimetres() {
        assert_eq!(Length::try_from_mm(720.0), Some(Length::from_tenths(7200)));
        assert_eq!(Length::try_from_mm(3.2), Some(Length::from_tenths(32)));
        assert_eq!(Length::try_from_mm(0.1), Some(Length::from_tenths(1)));
    }

    #[test]
    fn rejects_more_than_one_decimal() {
        assert_eq!(Length::try_from_mm(3.25), None);
        assert_eq!(Length::try_from_mm(f64::NAN), None);
        let error = serde_json::from_str::<Length>("3.25").unwrap_err();
        assert!(error.to_string().contains("more than one decimal"));
    }

    #[test]
    fn writes_whole_millimetres_as_integers() {
        assert_eq!(serde_json::to_string(&Length::from_mm(720)).unwrap(), "720");
        assert_eq!(
            serde_json::to_string(&Length::from_tenths(32)).unwrap(),
            "3.2"
        );
    }

    #[test]
    fn round_trips_through_json() {
        for tenths in [0, 1, 32, 7200, 28_032] {
            let length = Length::from_tenths(tenths);
            let json = serde_json::to_string(&length).unwrap();
            assert_eq!(serde_json::from_str::<Length>(&json).unwrap(), length);
        }
    }

    #[test]
    fn area_is_in_hundredths_of_square_millimetres() {
        let area = Length::from_mm(2).times(Length::from_tenths(15));
        assert_eq!(area.hundredths_mm2(), 300);
        assert_eq!(area.mm2(), 3.0);
    }
}
