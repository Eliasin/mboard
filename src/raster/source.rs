use crate::primitives::{
    dimensions::Dimensions,
    position::{DrawPosition, PixelPosition},
    rect::RasterRect,
};

use super::Pixel;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BoundedPosition {
    pub position: PixelPosition,
    pub delta: (i32, i32),
}

pub trait Subsource {
    fn subsource_at<'a>(&'a self, subrect: RasterRect) -> Option<Self>
    where
        Self: Sized;
    fn subsource_within_at<'a, S: RasterSource>(
        &'a self,
        other: &S,
        position: DrawPosition,
    ) -> Option<Self>
    where
        Self: Sized;
}

pub trait RasterSource {
    fn dimensions(&self) -> Dimensions;
    /// Bounds a position into the underlying collection.
    fn bound_position(&self, position: DrawPosition) -> BoundedPosition {
        self.dimensions().bound_position(position)
    }
    /// A slice of the row within the raster source.
    fn row(&self, row_num: usize) -> Option<&[Pixel]>;
    fn subrow_from_position(&self, start_position: PixelPosition, width: usize)
        -> Option<&[Pixel]>;
    fn bounded_subrow_from_position(&self, start_position: DrawPosition, width: usize) -> &[Pixel];
    fn pixel_at_position(&self, position: PixelPosition) -> Option<Pixel>;
    fn pixel_at_bounded_position(&self, position: DrawPosition) -> Pixel;
}

pub trait MutRasterSource: RasterSource {
    fn mut_row(&mut self, row_num: usize) -> Option<&mut [Pixel]>;
    fn mut_subrow_from_position(
        &mut self,
        start_position: PixelPosition,
        width: usize,
    ) -> Option<&mut [Pixel]>;
    fn mut_bounded_subrow_from_position(
        &mut self,
        start_position: DrawPosition,
        width: usize,
    ) -> &mut [Pixel];
    fn mut_pixel_at_position(&mut self, position: PixelPosition) -> Option<&mut Pixel>;
    fn mut_pixel_at_bounded_position(&mut self, position: DrawPosition) -> &mut Pixel;
}
