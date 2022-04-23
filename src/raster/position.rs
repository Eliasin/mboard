//! Positions within and spatially related to collections of pixels.
//!
//! `PixelPosition` values are strictly within a collection of pixels.
//!
//! `DrawPosition` values are allowed to be outside of a collection to support partial drawing of raster data.

use std::ops::Add;

/// A position within a 2d collection of pixels.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PixelPosition(pub (usize, usize));

/// An position at which to draw something. Does not have to be inside the
/// destination pixel collection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DrawPosition(pub (i64, i64));

impl Add<PixelPosition> for PixelPosition {
    type Output = PixelPosition;

    fn add(self, rhs: Self) -> Self::Output {
        PixelPosition((self.0 .0 + rhs.0 .0, self.0 .1 + rhs.0 .1))
    }
}

impl Add<(usize, usize)> for PixelPosition {
    type Output = PixelPosition;

    fn add(self, rhs: (usize, usize)) -> Self::Output {
        PixelPosition((self.0 .0 + rhs.0, self.0 .1 + rhs.1))
    }
}

impl Add<DrawPosition> for PixelPosition {
    type Output = DrawPosition;

    fn add(self, rhs: DrawPosition) -> Self::Output {
        DrawPosition((self.0 .0 as i64 + rhs.0 .0, self.0 .1 as i64 + rhs.0 .1))
    }
}
impl Add<(i64, i64)> for DrawPosition {
    type Output = DrawPosition;

    fn add(self, rhs: (i64, i64)) -> Self::Output {
        DrawPosition((self.0 .0 + rhs.0, self.0 .1 + rhs.1))
    }
}

impl From<(i64, i64)> for DrawPosition {
    fn from(p: (i64, i64)) -> Self {
        DrawPosition(p)
    }
}

impl From<(usize, usize)> for PixelPosition {
    fn from(p: (usize, usize)) -> Self {
        PixelPosition(p)
    }
}
