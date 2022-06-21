use crate::raster::iter::PixelPositionIterator;

use super::position::PixelPosition;

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

    /// Whether or not this scale is similar to another.
    pub fn similar_to(&self, other: Scale) -> bool {
        (self.width_factor - other.width_factor).abs() < 0.05
            && (self.height_factor - other.height_factor).abs() < 0.05
    }

    /// Whether or not this scale is similar to doing nothing.
    pub fn similar_to_unity(&self) -> bool {
        (self.width_factor - 1.0).abs() < 0.05 && (self.height_factor - 1.0).abs() < 0.05
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

        (
            (p.0 as f32 * x_stretch).floor() as usize,
            (p.1 as f32 * y_stretch).floor() as usize,
        )
            .into()
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

    /// The difference between this dimension and another.
    pub fn difference(&self, other: Dimensions) -> (i32, i32) {
        (
            self.width as i32 - other.width as i32,
            self.height as i32 - other.height as i32,
        )
    }

    /// The relative scale from this dimension space to another.
    pub fn relative_scale(&self, other: Dimensions) -> Scale {
        Scale {
            width_factor: self.width as f32 / other.width as f32,
            height_factor: self.height as f32 / other.height as f32,
        }
    }

    /// The largest of `width` and `height`.
    pub fn largest_dimension(&self) -> usize {
        usize::max(self.width, self.height)
    }

    /// Iterator over pixel positions in rect described by dimensions.
    pub fn iter_pixels(&self) -> PixelPositionIterator {
        PixelPositionIterator::new(*self)
    }
}
