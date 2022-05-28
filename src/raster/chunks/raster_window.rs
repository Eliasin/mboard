use std::fmt::Display;

use crate::raster::{
    position::{Dimensions, DrawPosition, PixelPosition},
    Pixel,
};

use super::{
    raster_chunk::RasterChunk,
    util::{
        display_raster_row, translate_rect_position_to_flat_index, BoundedIndex,
        IndexableByPosition, InvalidPixelSliceSize,
    },
};

/// A reference to a sub-rectangle of a raster chunk.
#[derive(Debug, Clone, Copy)]
pub struct RasterWindow<'a> {
    pub(super) backing: &'a [Pixel],
    pub(super) top_left: PixelPosition,
    pub(super) dimensions: Dimensions,
    pub(super) backing_dimensions: Dimensions,
}

impl<'a> Display for RasterWindow<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for row_num in 0..self.dimensions.height {
            let row_slice = self.get_row_slice(row_num).unwrap();
            s += "|";
            s += display_raster_row(row_slice).as_str();
            s += "|\n";
        }

        write!(f, "{}", s)
    }
}

impl<'a> RasterWindow<'a> {
    /// Creates a raster chunk window from a sub-rectangle of a raster chunk. The window area must be completely contained in the chunk.
    pub fn new(
        chunk: &'a RasterChunk,
        top_left: PixelPosition,
        width: usize,
        height: usize,
    ) -> Option<RasterWindow<'a>> {
        let over_width = top_left.0 .0 + width > chunk.dimensions().width;
        let over_height = top_left.0 .1 + height > chunk.dimensions().height;
        if over_width || over_height {
            None
        } else {
            Some(RasterWindow {
                backing: chunk.pixels(),
                backing_dimensions: chunk.dimensions(),
                top_left,
                dimensions: Dimensions { width, height },
            })
        }
    }

    /// Creates a window from the entirety of a slice, the rectangle's area must be exactly the size of the slice.
    pub fn from_slice(
        slice: &'a [Pixel],
        width: usize,
        height: usize,
    ) -> Result<RasterWindow<'a>, InvalidPixelSliceSize> {
        if width * height != slice.len() {
            Err(InvalidPixelSliceSize {
                desired_height: height,
                desired_width: width,
                buffer_size: slice.len(),
            })
        } else {
            Ok(RasterWindow {
                backing: slice,
                backing_dimensions: Dimensions { width, height },
                top_left: (0, 0).into(),
                dimensions: Dimensions { width, height },
            })
        }
    }

    /// Creates a new window by shrinking the current window. Will return `None` if resulting
    /// window is of zero size.
    pub fn shrink(
        &self,
        top: usize,
        bottom: usize,
        left: usize,
        right: usize,
    ) -> Option<RasterWindow<'a>> {
        if left + right >= self.dimensions.width || top + bottom >= self.dimensions.height {
            return None;
        }

        let new_top_left = self.top_left + PixelPosition::from((left, top));

        let new_width = self.dimensions.width - right - left;
        let new_height = self.dimensions.height - bottom - top;

        if new_top_left.0 .0 > self.backing_dimensions.width
            || new_top_left.0 .1 > self.backing_dimensions.height
        {
            return None;
        }

        Some(RasterWindow {
            backing: self.backing,
            top_left: new_top_left,
            dimensions: Dimensions {
                width: new_width,
                height: new_height,
            },
            backing_dimensions: self.backing_dimensions,
        })
    }

    /// Creates a raster chunk by copying the data in a window.
    pub fn to_chunk(&self) -> RasterChunk {
        let mut rect = Vec::<Pixel>::with_capacity(self.dimensions.width * self.dimensions.height);

        for row in 0..self.dimensions.height {
            for column in 0..self.dimensions.width {
                let source_position = (column, row);

                let source_index = self
                    .get_index_from_position(source_position.into())
                    .unwrap();
                rect.push(self.backing[source_index]);
            }
        }

        RasterChunk {
            pixels: rect.into_boxed_slice(),
            dimensions: self.dimensions,
        }
    }

    pub fn dimensions(&self) -> Dimensions {
        self.dimensions
    }
}

impl<'a> IndexableByPosition for RasterWindow<'a> {
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize> {
        if position.0 .0 > self.dimensions.width || position.0 .1 > self.dimensions.height {
            None
        } else {
            translate_rect_position_to_flat_index(
                (position + self.top_left).0,
                self.backing_dimensions.width,
                self.backing_dimensions.height,
            )
        }
    }

    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start = self.get_index_from_position((0, row_num).into())?;
        let row_end = self.get_index_from_position((self.dimensions.width - 1, row_num).into())?;

        Some(&self.backing[row_start..row_end + 1])
    }

    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex {
        let bounded_position = self.bound_position(position);

        // Since we bound x and y, this is guaranteed to not panic as long as the total area is
        // not 0.
        let index = translate_rect_position_to_flat_index(
            (bounded_position + self.top_left).0,
            self.backing_dimensions.width,
            self.backing_dimensions.height,
        )
        .unwrap();

        BoundedIndex {
            index,
            x_delta: TryInto::<i64>::try_into(bounded_position.0 .0).unwrap() - position.0 .0,
            y_delta: TryInto::<i64>::try_into(bounded_position.0 .1).unwrap() - position.0 .1,
        }
    }

    fn bound_position(&self, position: DrawPosition) -> PixelPosition {
        PixelPosition((
            (TryInto::<usize>::try_into(position.0 .0.max(0)).unwrap())
                .min(self.dimensions.width - 1),
            (TryInto::<usize>::try_into(position.0 .1.max(0)).unwrap())
                .min(self.dimensions.height - 1),
        ))
    }
}