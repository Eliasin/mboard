use std::{
    fmt::Display,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use bumpalo::Bump;

use crate::{
    primitives::{
        dimensions::Dimensions,
        position::{DrawPosition, PixelPosition, Position, UncheckedIntoPosition},
    },
    raster::{pixels::colors, Pixel},
};

use super::{
    nn_map::{InvalidScaleError, NearestNeighbourMap},
    raster_window::RasterWindow,
    util::{
        translate_rect_position_to_flat_index, BoundedIndex, IndexableByPosition,
        InvalidPixelSliceSize,
    },
};

pub type BoxRasterChunk = RasterChunk<Box<[Pixel]>>;
pub type RcRasterChunk = RasterChunk<Rc<[Pixel]>>;
pub type BumpRasterChunk<'bump> = RasterChunk<bumpalo::boxed::Box<'bump, [Pixel]>>;

/// A square collection of pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterChunk<T> {
    pub(super) pixels: T,
    pub(super) dimensions: Dimensions,
}

impl<T: Deref<Target = [Pixel]>> Display for RasterChunk<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_window().fmt(f)
    }
}

impl<T: Deref<Target = [Pixel]>> IndexableByPosition for RasterChunk<T> {
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize> {
        translate_rect_position_to_flat_index(
            position.into(),
            self.dimensions.width,
            self.dimensions.height,
        )
    }

    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start = self.get_index_from_position((0, row_num).into())?;
        let row_end = self.get_index_from_position((self.dimensions.width - 1, row_num).into())?;

        Some(&self.pixels[row_start..row_end + 1])
    }

    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex {
        let bounded_position = self.bound_position(position);

        let index = translate_rect_position_to_flat_index(
            bounded_position.into(),
            self.dimensions.width,
            self.dimensions.height,
        )
        .expect("position is bounded");

        BoundedIndex {
            index,
            x_delta: bounded_position.0 as i32 - position.0,
            y_delta: bounded_position.1 as i32 - position.1,
        }
    }

    fn bound_position(&self, position: DrawPosition) -> PixelPosition {
        (
            position.0.min(self.dimensions.width as i32 - 1).max(0),
            position.1.min(self.dimensions.height as i32 - 1).max(0),
        )
            .unchecked_into_position()
    }
}

type RowOperation = fn(&mut [Pixel], &[Pixel]) -> ();

impl<T: Deref<Target = [Pixel]>> RasterChunk<T> {
    /// Takes the whole chunk as a raster window.
    pub fn as_window(&self) -> RasterWindow {
        RasterWindow {
            backing: self.pixels.as_ref(),
            top_left: (0, 0).into(),
            dimensions: self.dimensions,
            backing_dimensions: self.dimensions,
        }
    }

    /// Shrinks a raster window to the sub-window that is contained within
    /// the current raster chunk. Returns `None` if the resultant window is empty.
    fn shrink_window_to_contain<'a>(
        &self,
        source: &RasterWindow<'a>,
        dest_position: DrawPosition,
    ) -> Option<RasterWindow<'a>> {
        if source.dimensions().width == 0 || source.dimensions().height == 0 {
            return None;
        }

        let source_top_left_in_dest = self.get_index_from_bounded_position(dest_position);

        let bottom_right = (
            (source.dimensions().width - 1)
                .try_into()
                .expect("dimensions are checked to be greater than 0"),
            (source.dimensions().height - 1)
                .try_into()
                .expect("dimensions are checked to be greater than 0"),
        )
            .into();

        let source_bottom_right_in_dest =
            self.get_index_from_bounded_position(dest_position + bottom_right);

        let top_left_past_bottom_right =
            source_top_left_in_dest.y_delta < 0 || source_top_left_in_dest.x_delta < 0;
        let bottom_right_past_top_left =
            source_bottom_right_in_dest.y_delta > 0 || source_bottom_right_in_dest.x_delta > 0;
        if top_left_past_bottom_right || bottom_right_past_top_left {
            // Source is completely outside of dest
            return None;
        }

        let shrink_top = source_top_left_in_dest
            .y_delta
            .try_into()
            .expect("this y delta is checked to be over 0");
        let shrink_bottom = (-source_bottom_right_in_dest.y_delta)
            .try_into()
            .expect("this y delta is checked to be over 0");

        let shrink_left = source_top_left_in_dest
            .x_delta
            .try_into()
            .expect("this x delta is checked to be over 0");
        let shrink_right = (-source_bottom_right_in_dest.x_delta)
            .try_into()
            .expect("this x delta is checked to be under 0");

        source.shrink(shrink_top, shrink_bottom, shrink_left, shrink_right)
    }
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }

    pub fn dimensions(&self) -> Dimensions {
        self.dimensions
    }
}

impl<T: DerefMut<Target = [Pixel]>> RasterChunk<T> {
    /// Performs an operation on the raster chunk row-wise.
    fn perform_row_operation<F>(
        &mut self,
        dest_position: DrawPosition,
        width: usize,
        height: usize,
        operation: &mut F,
    ) where
        F: FnMut(&mut [Pixel]),
    {
        for row_num in 0..height {
            if row_num >= self.dimensions.height {
                break;
            }

            let start = self
                .get_index_from_bounded_position(dest_position + (0_i32, row_num as i32).into())
                .index;

            let end = self
                .get_index_from_bounded_position(
                    dest_position + (width as i32 - 1, row_num as i32).into(),
                )
                .index;

            let dest_slice = &mut self.pixels[start..end + 1];
            operation(dest_slice)
        }
    }

    /// Performs an operation on a `zipped` representation of the source raster window
    /// and the raster chunk. The operation will be given a `mut` reference to each
    /// row of the chunk and a shared reference to the corresponding source row.
    fn perform_zipped_row_operation(
        &mut self,
        source: &RasterWindow,
        dest_position: DrawPosition,
        operation: RowOperation,
    ) {
        let bounded_top_left = self.bound_position(dest_position);
        if let Some(shrunk_source) = self.shrink_window_to_contain(source, dest_position) {
            for row_num in 0..shrunk_source.dimensions().height {
                let source_row = shrunk_source.get_row_slice(row_num);

                let row_start_position = bounded_top_left + (0_usize, row_num).into();
                let row_start_index = self
                    .get_index_from_position(row_start_position)
                    .expect("position should always be in source by construction");
                let row_end_position =
                    bounded_top_left + (shrunk_source.dimensions().width - 1, row_num).into();
                let row_end_index = self
                    .get_index_from_position(row_end_position)
                    .expect("position should always be in source by construction");

                if let Some(source_row) = source_row {
                    let dest_slice = &mut self.pixels[row_start_index..row_end_index + 1];

                    operation(dest_slice, source_row);
                }
            }
        }
    }

    /// Blits a render window onto the raster chunk at `dest_position`.
    /// If the window at `dest_position` is not contained within the chunk,
    /// the portion of the destination outside the chunk is ignored.
    pub fn blit(&mut self, source: &RasterWindow, dest_position: DrawPosition) {
        // Optimization for blittig something completely over a chunk
        if source.dimensions().width == self.dimensions.width
            && source.dimensions().height == self.dimensions.height
            && source.backing.len() == self.pixels.len()
            && dest_position == DrawPosition::from((0, 0))
        {
            self.pixels.copy_from_slice(source.backing);
            return;
        }

        self.perform_zipped_row_operation(source, dest_position, |d, s| d.copy_from_slice(s));
    }

    /// Fills a rect with a specified pixel value, lower memory footprint than creating
    /// a raster chunk full of a single source pixel to blit.
    pub fn fill_rect(
        &mut self,
        pixel: Pixel,
        dest_position: DrawPosition,
        width: usize,
        height: usize,
    ) {
        self.perform_row_operation(dest_position, width, height, &mut |d| d.fill(pixel));
    }

    /// Draws a render window onto the raster chunk at `dest_position` using alpha compositing.
    /// If the window at `dest_position` is not contained within the chunk,
    /// the portion of the destination outside the chunk is ignored.
    pub fn composite_over(&mut self, source: &RasterWindow, dest_position: DrawPosition) {
        self.perform_zipped_row_operation(source, dest_position, |d, s| {
            for (pixel_d, pixel_s) in d.iter_mut().zip(s.iter()) {
                pixel_d.composite_over(pixel_s);
            }
        });
    }

    /// Shift the pixels in a raster chunk horizontally to the left. Pixels
    /// are shifted into from `outside` the chunk have unspecified values.
    pub fn horizontal_shift_left(&mut self, shift: usize) {
        if shift > self.dimensions.width {
            // Everything is shifted in from outside, so the whole
            // chunk is unspecified and we can just return now
            return;
        }

        let num_pixels_in_dest_row = self.dimensions.width - shift;

        let shift_start_column = shift;

        for row in 0..self.dimensions.height {
            let row_start_position = row * self.dimensions.width;

            let shift_start_position = row_start_position + shift_start_column;
            let shift_end_position = shift_start_position + num_pixels_in_dest_row;

            self.pixels
                .copy_within(shift_start_position..shift_end_position, row_start_position);
        }
    }

    /// Shift the pixels in a raster chunk horizontally to the right. Pixels
    /// are shifted into from `outside` the chunk have unspecified values.
    pub fn horizontal_shift_right(&mut self, shift: usize) {
        if shift > self.dimensions.width {
            // Everything is shifted in from outside, so the whole
            // chunk is unspecified and we can just return now
            return;
        }

        let num_pixels_in_dest_row = self.dimensions.width - shift;

        for row in 0..self.dimensions.height {
            let row_start_position = row * self.dimensions.width;

            let shift_start_position = row_start_position;
            let shift_end_position = shift_start_position + num_pixels_in_dest_row;

            self.pixels.copy_within(
                shift_start_position..shift_end_position,
                row_start_position + shift,
            );
        }
    }

    /// Shift the pixels in a raster chunk vertically down. Pixels
    /// are shifted into from `outside` the chunk have unspecified values.
    pub fn vertical_shift_down(&mut self, shift: usize) {
        if shift == 0 {
            return;
        }

        let shift_end = self.pixels.len() - shift * self.dimensions.width;
        self.pixels
            .copy_within(0..shift_end, shift * self.dimensions.width);
    }

    /// Shift the pixels in a raster chunk vertically up. Pixels
    /// are shifted into from `outside` the chunk have unspecified values.
    pub fn vertical_shift_up(&mut self, shift: usize) {
        if shift == 0 {
            return;
        }
        let len = self.pixels.len();

        let shift_start = shift * self.dimensions.width;
        self.pixels.copy_within(shift_start..len, 0);
    }
}

impl BoxRasterChunk {
    pub fn into_pixels(self) -> Box<[Pixel]> {
        self.pixels
    }

    /// Create a new raster chunk filled in with a pixel value.
    pub fn new_fill(pixel: Pixel, width: usize, height: usize) -> BoxRasterChunk {
        let pixels = vec![pixel; width * height];

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            dimensions: Dimensions { width, height },
        }
    }

    /// Create a new raster chunk where each pixel value is filled in by a closure given the pixel's location.
    pub fn new_fill_dynamic<F>(f: &mut F, width: usize, height: usize) -> BoxRasterChunk
    where
        F: FnMut(PixelPosition) -> Pixel,
    {
        let mut pixels = vec![colors::transparent(); width * height];

        for row in 0..width {
            for column in 0..height {
                pixels[row * width + column] = f(PixelPosition::from((row, column)));
            }
        }

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            dimensions: Dimensions { width, height },
        }
    }

    /// Create a new raster chunk that is completely transparent.
    pub fn new(width: usize, height: usize) -> BoxRasterChunk {
        BoxRasterChunk::new_fill(colors::transparent(), width, height)
    }

    /// Derive a sub-chunk from a raster chunk. If the sub-chunk positioned at `position` is not fully contained by the source chunk,
    /// any regions outside the source chunk will be filled in as transparent.
    pub fn clone_square(
        &self,
        position: PixelPosition,
        width: usize,
        height: usize,
    ) -> BoxRasterChunk {
        let mut rect = Vec::<Pixel>::with_capacity(width * height);

        for row in 0..height {
            for column in 0..width {
                let source_position = (column + position.0, row + position.1);

                if let Some(source_index) = self.get_index_from_position(source_position.into()) {
                    rect.push(self.pixels[source_index]);
                } else {
                    rect.push(colors::transparent());
                }
            }
        }

        RasterChunk {
            pixels: rect.into_boxed_slice(),
            dimensions: Dimensions { width, height },
        }
    }

    /// Creates a raster chunk from
    pub fn from_vec(
        pixels: Vec<Pixel>,
        width: usize,
        height: usize,
    ) -> Result<RasterChunk<Box<[Pixel]>>, InvalidPixelSliceSize> {
        if width * height != pixels.len() {
            Err(InvalidPixelSliceSize {
                desired_height: height,
                desired_width: width,
                buffer_size: pixels.len(),
            })
        } else {
            Ok(RasterChunk {
                pixels: pixels.into_boxed_slice(),
                dimensions: Dimensions { width, height },
            })
        }
    }

    /// Scales the chunk by to a new size using the nearest-neighbour algorithm.
    pub fn nn_scale(&mut self, new_size: Dimensions) {
        if new_size == self.dimensions {
            return;
        }

        let mut new_chunk = BoxRasterChunk::new(new_size.width, new_size.height);

        for row in 0..new_size.height {
            for column in 0..new_size.width {
                let nearest = self
                    .dimensions
                    .transform_point((column, row).into(), new_size);

                let source_index = self
                    .get_index_from_position(nearest)
                    .expect("transformation source position should be contained in source");

                let new_index = new_chunk
                    .get_index_from_position((column, row).into())
                    .expect("transformation source position should be contained in source");
                new_chunk.pixels[new_index] = self.pixels[source_index];
            }
        }

        *self = new_chunk;
    }

    /// A chunk scaled to a new size using the nearest-neighbour algorithm.
    pub fn nn_scaled(&mut self, new_size: Dimensions) -> BoxRasterChunk {
        let mut new_chunk = BoxRasterChunk::new(new_size.width, new_size.height);

        for row in 0..new_size.height {
            for column in 0..new_size.width {
                let nearest = self
                    .dimensions
                    .transform_point((column, row).into(), new_size);

                let source_index = self
                    .get_index_from_position(nearest)
                    .expect("transformation source position should be contained in source");

                let new_index = new_chunk
                    .get_index_from_position((column, row).into())
                    .expect("transformation source position should be contained in source");
                new_chunk.pixels[new_index] = self.pixels[source_index];
            }
        }

        new_chunk
    }

    /// Scales the chunk to a new size with a precalculated nearest-neighbour mapped.
    pub fn nn_scale_with_map(
        &mut self,
        nn_map: &NearestNeighbourMap,
    ) -> Result<(), InvalidScaleError> {
        if nn_map.destination_dimensions() == self.dimensions {
            return Ok(());
        }

        let destination_dimensions = nn_map.destination_dimensions();
        let mut new_chunk =
            BoxRasterChunk::new(destination_dimensions.width, destination_dimensions.height);

        nn_map.scale_using_map(self, &mut new_chunk)?;

        *self = new_chunk;

        Ok(())
    }

    /// A scaled chunk of a new size with a precalculated nearest-neighbour mapped.
    pub fn nn_scaled_with_map(
        &self,
        nn_map: &NearestNeighbourMap,
    ) -> Result<BoxRasterChunk, InvalidScaleError> {
        let destination_dimensions = nn_map.destination_dimensions();
        let mut new_chunk =
            BoxRasterChunk::new(destination_dimensions.width, destination_dimensions.height);

        nn_map.scale_using_map(self, &mut new_chunk)?;

        Ok(new_chunk)
    }

    /// Scales the chunk by a factor using the nearest-neighbour algorithm and
    /// place the result into a bump.
    pub fn nn_scale_into_bump<'bump>(
        &mut self,
        new_size: Dimensions,
        bump: &'bump Bump,
    ) -> BumpRasterChunk<'bump> {
        let mut new_chunk = BumpRasterChunk::new(new_size.width, new_size.height, bump);

        for Position::<usize>(column, row) in new_size.iter_pixels() {
            let nearest = self
                .dimensions
                .transform_point((column, row).into(), new_size);

            let source_index = self
                .get_index_from_position(nearest)
                .expect("transformation source position should be contained in source");
            let new_index = new_chunk
                .get_index_from_position((column, row).into())
                .expect(
                "position should always be contained in new chunk as position should be bounded",
            );
            new_chunk.pixels[new_index] = self.pixels[source_index];
        }

        new_chunk
    }

    /// Scales the chunk to a new size with a precalculated nearest-neighbour mapped
    /// and place the result into a bump.
    pub fn nn_scale_with_map_into_bump<'bump>(
        &mut self,
        nn_map: &NearestNeighbourMap,
        bump: &'bump Bump,
    ) -> Result<BumpRasterChunk<'bump>, InvalidScaleError> {
        nn_map.scale_using_map_into_bump(self, bump)
    }
}

impl<'bump> BumpRasterChunk<'bump> {
    pub fn into_pixels(self) -> bumpalo::boxed::Box<'bump, [Pixel]> {
        self.pixels
    }

    /// Create a new raster chunk filled in with a pixel value.
    pub fn new_fill(pixel: Pixel, width: usize, height: usize, bump: &Bump) -> BumpRasterChunk {
        let pixels = bumpalo::vec![in bump; pixel; width * height];

        BumpRasterChunk {
            pixels: pixels.into_boxed_slice(),
            dimensions: Dimensions { width, height },
        }
    }

    /// Create a new raster chunk where each pixel value is filled in by a closure given the pixel's location.
    pub fn new_fill_dynamic(
        f: fn(PixelPosition) -> Pixel,
        width: usize,
        height: usize,
        bump: &Bump,
    ) -> BumpRasterChunk {
        let dimensions = Dimensions { width, height };
        let pixels = bumpalo::boxed::Box::from_iter_in(dimensions.iter_pixels().map(f), bump);

        BumpRasterChunk { pixels, dimensions }
    }

    /// Create a new raster chunk that is completely transparent.
    pub fn new(width: usize, height: usize, bump: &Bump) -> BumpRasterChunk {
        BumpRasterChunk::new_fill(colors::transparent(), width, height, bump)
    }

    /// Scales the chunk by a factor using the nearest-neighbour algorithm and
    /// place the result into a bump.
    pub fn nn_scale_into_bump<'other_bump>(
        &mut self,
        new_size: Dimensions,
        bump: &'other_bump Bump,
    ) -> BumpRasterChunk<'other_bump> {
        let mut new_chunk = BumpRasterChunk::new(new_size.width, new_size.height, bump);

        for Position::<usize>(column, row) in new_size.iter_pixels() {
            let nearest = self
                .dimensions
                .transform_point((column, row).into(), new_size);

            let source_index = self
                .get_index_from_position(nearest)
                .expect("transformation source point should always be contained in source");
            let new_index = new_chunk
                .get_index_from_position((column, row).into())
                .expect("position should always be in new chunk as it is bounded");
            new_chunk.pixels[new_index] = self.pixels[source_index];
        }

        new_chunk
    }

    /// Scales the chunk to a new size with a precalculated nearest-neighbour mapped
    /// and place the result into a bump.
    pub fn nn_scale_with_map_into_bump<'other_bump>(
        &mut self,
        nn_map: &NearestNeighbourMap,
        bump: &'other_bump Bump,
    ) -> Result<BumpRasterChunk<'other_bump>, InvalidScaleError> {
        nn_map.scale_using_map_into_bump(self, bump)
    }
}

impl RcRasterChunk {
    /// Create a new raster chunk filled in with a pixel value.
    pub fn new_fill(pixel: Pixel, width: usize, height: usize) -> RcRasterChunk {
        let pixels = vec![pixel; width * height];

        RasterChunk {
            pixels: Rc::from(pixels.into_boxed_slice()),
            dimensions: Dimensions { width, height },
        }
    }

    /// Create a new raster chunk where each pixel value is filled in by a closure given the pixel's location.
    pub fn new_fill_dynamic(
        f: fn(PixelPosition) -> Pixel,
        width: usize,
        height: usize,
    ) -> RcRasterChunk {
        let mut pixels = vec![colors::transparent(); width * height];

        for row in 0..width {
            for column in 0..height {
                pixels[row * width + column] = f(PixelPosition::from((row, column)));
            }
        }

        RasterChunk {
            pixels: Rc::from(pixels.into_boxed_slice()),
            dimensions: Dimensions { width, height },
        }
    }

    /// Create a new raster chunk that is completely transparent.
    pub fn new(width: usize, height: usize) -> RcRasterChunk {
        RcRasterChunk::new_fill(colors::transparent(), width, height)
    }

    /// Derive a sub-chunk from a raster chunk. If the sub-chunk positioned at `position` is not fully contained by the source chunk,
    /// any regions outside the source chunk will be filled in as transparent.
    pub fn clone_square(
        &self,
        position: PixelPosition,
        width: usize,
        height: usize,
    ) -> RcRasterChunk {
        let mut rect = Vec::<Pixel>::with_capacity(width * height);

        for row in 0..height {
            for column in 0..width {
                let source_position = (column + position.0, row + position.1);

                if let Some(source_index) = self.get_index_from_position(source_position.into()) {
                    rect.push(self.pixels[source_index]);
                } else {
                    rect.push(colors::transparent());
                }
            }
        }

        RasterChunk {
            pixels: Rc::from(rect.into_boxed_slice()),
            dimensions: Dimensions { width, height },
        }
    }
}

impl RcRasterChunk {
    pub fn get_mut(&mut self) -> Option<RasterChunk<&mut [Pixel]>> {
        let pixels = Rc::get_mut(&mut self.pixels)?;

        Some(RasterChunk {
            pixels,
            dimensions: self.dimensions,
        })
    }

    pub fn diverge(&self) -> Self {
        let mut pixels = Box::new_uninit_slice(self.pixels.len());

        MaybeUninit::write_slice(&mut pixels, &*self.pixels);

        let pixels = unsafe { pixels.assume_init() };
        let pixels = Rc::from(pixels);

        RcRasterChunk {
            pixels,
            dimensions: self.dimensions,
        }
    }
}

impl From<BoxRasterChunk> for RcRasterChunk {
    fn from(box_raster_chunk: BoxRasterChunk) -> Self {
        RcRasterChunk {
            pixels: Rc::from(box_raster_chunk.pixels),
            dimensions: box_raster_chunk.dimensions,
        }
    }
}
