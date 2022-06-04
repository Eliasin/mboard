//! Positions within and spatially related to collections of pixels.
//!
//! `PixelPosition` values are strictly within a collection of pixels.
//!
//! `DrawPosition` values are allowed to be outside of a collection to support partial drawing of raster data.

use std::{convert::TryInto, ops::Add};

/// A position within a 2d collection of pixels.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PixelPosition(pub (usize, usize));

impl PixelPosition {
    pub fn translate(&self, v: (usize, usize)) -> PixelPosition {
        PixelPosition((self.0 .0 + v.1, self.0 .1 + v.0))
    }
}

/// An position at which to draw something. Does not have to be inside the
/// destination pixel collection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DrawPosition(pub (i64, i64));

/// A relative scale between two dimensions. Guaranteed to be non-negative.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Scale {
    pub width_factor: f32,
    pub height_factor: f32,
}

impl Scale {
    pub fn new(width_factor: f32, height_factor: f32) -> Option<Scale> {
        if width_factor < 0.0 || height_factor < 0.0 {
            None
        } else {
            Some(Scale {
                width_factor,
                height_factor,
            })
        }
    }

    pub fn width_factor(&self) -> f32 {
        self.width_factor
    }

    pub fn height_factor(&self) -> f32 {
        self.height_factor
    }
}

/// The dimensions of a 2d object.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Dimensions {
    pub width: usize,
    pub height: usize,
}

impl Dimensions {
    /// Transform a point from another dimension space to this one, preserving the relative
    /// offset from the top-left.
    pub fn transform_point(&self, p: PixelPosition, src_dimensions: Dimensions) -> PixelPosition {
        let x_stretch: f32 = self.width as f32 / src_dimensions.width as f32;
        let y_stretch: f32 = self.height as f32 / src_dimensions.height as f32;

        PixelPosition((
            (p.0 .0 as f32 * x_stretch).floor() as usize,
            (p.0 .1 as f32 * y_stretch).floor() as usize,
        ))
    }

    /// Scale the dimensions.
    pub fn scale(&self, scale: Scale) -> Dimensions {
        let new_width = ((self.width as f32) * scale.width_factor).round() as usize;
        let new_height = ((self.height as f32) * scale.height_factor).round() as usize;
        Dimensions {
            width: new_width,
            height: new_height,
        }
    }

    /// Gets the difference between this dimension and another.
    pub fn difference(&self, other: Dimensions) -> (i64, i64) {
        (
            self.width as i64 - other.width as i64,
            self.height as i64 - other.height as i64,
        )
    }

    /// The largest of `width` and `height`.
    pub fn largest_dimension(&self) -> usize {
        usize::max(self.width, self.height)
    }
}

/// A position referencing a pixel within a raster layer.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct LayerPosition(pub (i64, i64));

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

impl From<PixelPosition> for DrawPosition {
    fn from(p: PixelPosition) -> Self {
        DrawPosition((p.0 .0.try_into().unwrap(), p.0 .1.try_into().unwrap()))
    }
}
