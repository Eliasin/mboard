use std::{fmt::Display, mem::MaybeUninit, ops::Deref};

use bumpalo::Bump;

use crate::{
    primitives::{
        dimensions::Dimensions,
        position::{DrawPosition, PixelPosition, UncheckedIntoPosition},
        rect::{DrawRect, RasterRect},
    },
    raster::{
        source::{BoundedPosition, RasterSource, Subsource},
        Pixel,
    },
};

use super::{
    raster_chunk::{BoxRasterChunk, BumpRasterChunk, RasterChunk},
    translate_rect_position_to_flat_index,
    util::{display_raster_row, InvalidPixelSliceSize},
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
            let row_slice = self
                .row(row_num)
                .expect("row_num should always be less than height");
            s += "|";
            s += display_raster_row(row_slice).as_str();
            s += "|\n";
        }

        write!(f, "{}", s)
    }
}

impl<'a> RasterWindow<'a> {
    /// Creates a raster chunk window from a sub-rectangle of a raster chunk. The window area must be completely contained in the chunk.
    pub fn new<T: Deref<Target = [Pixel]>>(
        chunk: &'a RasterChunk<T>,
        top_left: PixelPosition,
        width: usize,
        height: usize,
    ) -> Option<RasterWindow<'a>> {
        let over_width = top_left.0 + width > chunk.dimensions().width;
        let over_height = top_left.1 + height > chunk.dimensions().height;
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

        if new_top_left.0 > self.backing_dimensions.width
            || new_top_left.1 > self.backing_dimensions.height
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
    pub fn to_chunk(&self) -> BoxRasterChunk {
        let mut chunk_pixels: Box<[MaybeUninit<Pixel>]> =
            Box::new_uninit_slice(self.dimensions.width * self.dimensions.height);

        for row in 0..self.dimensions.height {
            let row_start_position = (0, row);
            let row_start_source_index = translate_rect_position_to_flat_index(
                self.top_left + row_start_position.into(),
                self.backing_dimensions,
            )
            .expect("position should be in source by construction");
            let row_end_position = (self.dimensions.width - 1, row);
            let row_end_source_index = translate_rect_position_to_flat_index(
                self.top_left + row_end_position.into(),
                self.backing_dimensions,
            )
            .expect("position should be in source by construction");
            let row_start_new_index = row * self.dimensions.width;
            let row_end_new_index = row * self.dimensions.width + self.dimensions.width - 1;

            MaybeUninit::write_slice(
                &mut chunk_pixels[row_start_new_index..(row_end_new_index + 1)],
                &self.backing[row_start_source_index..(row_end_source_index + 1)],
            );
        }

        // We initialize the entire chunk within the for loop, so this is sound
        let chunk_pixels = unsafe { chunk_pixels.assume_init() };

        BoxRasterChunk {
            pixels: chunk_pixels,
            dimensions: self.dimensions,
        }
    }

    /// Creates a raster chunk in a bump by copying the data in a window.
    pub fn to_chunk_into_bump<'bump>(&self, bump: &'bump Bump) -> BumpRasterChunk<'bump> {
        let chunk_pixels: &'bump mut [MaybeUninit<Pixel>] = bump.alloc_slice_fill_copy(
            self.dimensions.width * self.dimensions.height,
            MaybeUninit::uninit(),
        );

        for row in 0..self.dimensions.height {
            let row_start_position = (0, row);
            let row_start_source_index =
                translate_rect_position_to_flat_index(row_start_position.into(), self.dimensions)
                    .expect("position should be in source by construction");
            let row_end_position = (self.dimensions.width - 1, row);
            let row_end_source_index =
                translate_rect_position_to_flat_index(row_end_position.into(), self.dimensions)
                    .expect("position should be in source by construction");
            let row_start_new_index = row * self.dimensions.width;
            let row_end_new_index = row * self.dimensions.width + self.dimensions.width - 1;
            MaybeUninit::write_slice(
                &mut chunk_pixels[row_start_new_index..(row_end_new_index + 1)],
                &self.backing[row_start_source_index..(row_end_source_index + 1)],
            );
        }

        // Technically we could transmute `chunk_pixels` into `bumpalo::boxed::Box` because
        // of how it's `#[repr(transparent)]` but the documentation reccomends doing
        // it this way instead
        let chunk_pixels = unsafe {
            let initialized_pixels = std::mem::transmute::<_, &'bump mut [Pixel]>(chunk_pixels);
            bumpalo::boxed::Box::from_raw(initialized_pixels)
        };

        BumpRasterChunk {
            pixels: chunk_pixels,
            dimensions: self.dimensions,
        }
    }

    pub fn dimensions(&self) -> Dimensions {
        self.dimensions
    }
}

impl<'s> Subsource for RasterWindow<'s> {
    fn subsource_at<'a>(&'a self, subrect: RasterRect) -> Option<Self>
    where
        Self: Sized,
    {
        self.dimensions
            .contains_rect(&subrect)
            .then_some(RasterWindow {
                backing: self.backing,
                backing_dimensions: self.backing_dimensions,
                top_left: self.top_left.translate(subrect.top_left.into()),
                dimensions: subrect.dimensions,
            })
    }

    fn subsource_within_at<'a, S: RasterSource>(
        &'a self,
        other: &S,
        position: DrawPosition,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let draw_rect = DrawRect {
            top_left: position,
            dimensions: self.dimensions,
        };
        let subsource_rect = draw_rect.subrect_contained_in(other.dimensions())?;
        if subsource_rect.is_degenerate() {
            None
        } else {
            self.subsource_at(subsource_rect)
        }
    }
}

impl<'s> RasterSource for RasterWindow<'s> {
    fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    fn row(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start_offset = (0, row_num).into();
        let row_end_offset = (self.dimensions.width - 1, row_num).into();

        if !self.dimensions.contains(row_start_offset) || !self.dimensions.contains(row_end_offset)
        {
            return None;
        }

        let row_start = self.top_left + row_start_offset;
        let row_end = self.top_left + row_end_offset;

        let row_start_index =
            translate_rect_position_to_flat_index(row_start, self.backing_dimensions)?;
        let row_end_index =
            translate_rect_position_to_flat_index(row_end, self.backing_dimensions)?;

        Some(&self.backing[row_start_index..row_end_index + 1])
    }

    fn subrow_from_position(
        &self,
        start_position: PixelPosition,
        width: usize,
    ) -> Option<&[Pixel]> {
        let row_end_offset = start_position + (width - 1, 0).into();

        if !self.dimensions.contains(start_position) || !self.dimensions.contains(row_end_offset) {
            return None;
        }

        let row_start_index = translate_rect_position_to_flat_index(
            self.top_left + start_position,
            self.backing_dimensions,
        )?;
        let row_end_index = translate_rect_position_to_flat_index(
            self.top_left + row_end_offset,
            self.backing_dimensions,
        )?;

        Some(&self.backing[row_start_index..row_end_index + 1])
    }

    fn bounded_subrow_from_position(&self, start_position: DrawPosition, width: usize) -> &[Pixel] {
        let end_position = self
            .dimensions
            .bound_position(start_position + (width as i32 - 1, 0).into())
            .position;
        let start_position = self.dimensions.bound_position(start_position).position;
        let row_start_index = translate_rect_position_to_flat_index(
            self.top_left + start_position,
            self.backing_dimensions,
        )
        .expect("position is bounded");
        let row_end_index = translate_rect_position_to_flat_index(
            self.top_left + end_position,
            self.backing_dimensions,
        )
        .expect("position is bounded");

        &self.backing[row_start_index..row_end_index + 1]
    }

    fn pixel_at_position(&self, position: PixelPosition) -> Option<Pixel> {
        self.dimensions
            .contains(position)
            .then_some(
                translate_rect_position_to_flat_index(
                    self.top_left + position,
                    self.backing_dimensions,
                )
                .map(|index| self.backing[index]),
            )
            .flatten()
    }

    fn pixel_at_bounded_position(&self, position: DrawPosition) -> Pixel {
        self.backing[translate_rect_position_to_flat_index(
            self.dimensions.bound_position(position).position,
            self.dimensions,
        )
        .expect("position is bounded")]
    }
}
