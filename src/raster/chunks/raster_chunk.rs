use std::{
    fmt::Display,
    mem::MaybeUninit,
    ops::{Deref, DerefMut, Sub},
    rc::Rc,
};

use bumpalo::Bump;

use crate::{
    primitives::{
        dimensions::Dimensions,
        position::{DrawPosition, PixelPosition, UncheckedIntoPosition},
        rect::DrawRect,
    },
    raster::{
        iter::NearestNeighbourMappingIterator,
        pixels::colors,
        source::{BoundedPosition, MutRasterSource, RasterSource, Subsource},
        Pixel,
    },
};

use super::{
    nn_map::{InvalidScaleError, NearestNeighbourMap},
    raster_window::RasterWindow,
    translate_rect_position_to_flat_index,
    util::InvalidPixelSliceSize,
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

impl Subsource for BoxRasterChunk {
    fn subsource_at<'a>(&'a self, subrect: crate::primitives::rect::RasterRect) -> Option<Self>
    where
        Self: Sized,
    {
        Some(self.as_window().subsource_at(subrect)?.to_chunk())
    }

    fn subsource_within_at<'a, S: RasterSource>(
        &'a self,
        other: &S,
        position: DrawPosition,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        Some(
            self.as_window()
                .subsource_within_at(other, position)?
                .to_chunk(),
        )
    }
}

impl<T: Deref<Target = [Pixel]>> RasterSource for RasterChunk<T> {
    fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    fn row(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start_index =
            translate_rect_position_to_flat_index((0, row_num).into(), self.dimensions)?;
        let row_end_index = translate_rect_position_to_flat_index(
            (self.dimensions.width - 1, row_num).into(),
            self.dimensions,
        )?;

        Some(&self.pixels[row_start_index..row_end_index + 1])
    }

    fn subrow_from_position(
        &self,
        start_position: PixelPosition,
        width: usize,
    ) -> Option<&[Pixel]> {
        let row_start_index =
            translate_rect_position_to_flat_index(start_position, self.dimensions)?;
        let row_end_index = translate_rect_position_to_flat_index(
            start_position + (width - 1, 0).into(),
            self.dimensions,
        )?;

        Some(&self.pixels[row_start_index..row_end_index + 1])
    }

    fn bounded_subrow_from_position(&self, start_position: DrawPosition, width: usize) -> &[Pixel] {
        let end_position = self
            .dimensions
            .bound_position(start_position + (width as i32 - 1, 0).into())
            .position;
        let start_position = self.dimensions.bound_position(start_position).position;
        let row_start_index =
            translate_rect_position_to_flat_index(start_position, self.dimensions)
                .expect("position is bounded");
        let row_end_index = translate_rect_position_to_flat_index(end_position, self.dimensions)
            .expect("position is bounded");

        &self.pixels[row_start_index..row_end_index + 1]
    }

    fn pixel_at_position(&self, position: PixelPosition) -> Option<Pixel> {
        translate_rect_position_to_flat_index(position, self.dimensions)
            .map(|index| self.pixels[index])
    }

    fn pixel_at_bounded_position(&self, position: DrawPosition) -> Pixel {
        self.pixels[translate_rect_position_to_flat_index(
            self.dimensions.bound_position(position).position,
            self.dimensions,
        )
        .expect("position is bounded")]
    }
}

impl<T: DerefMut<Target = [Pixel]>> MutRasterSource for RasterChunk<T> {
    fn mut_row(&mut self, row_num: usize) -> Option<&mut [Pixel]> {
        let row_start_index =
            translate_rect_position_to_flat_index((0, row_num).into(), self.dimensions)?;
        let row_end_index = translate_rect_position_to_flat_index(
            (self.dimensions.width - 1, row_num).into(),
            self.dimensions,
        )?;

        Some(&mut self.pixels[row_start_index..row_end_index + 1])
    }

    fn mut_subrow_from_position(
        &mut self,
        start_position: PixelPosition,
        width: usize,
    ) -> Option<&mut [Pixel]> {
        let row_start_index =
            translate_rect_position_to_flat_index(start_position, self.dimensions)?;
        let row_end_index = translate_rect_position_to_flat_index(
            start_position + (width - 1, 0).into(),
            self.dimensions,
        )?;

        Some(&mut self.pixels[row_start_index..row_end_index + 1])
    }

    fn mut_bounded_subrow_from_position(
        &mut self,
        start_position: DrawPosition,
        width: usize,
    ) -> &mut [Pixel] {
        let end_position = self
            .dimensions
            .bound_position(start_position + (width as i32 - 1, 0).into())
            .position;
        let start_position = self.dimensions.bound_position(start_position).position;
        let row_start_index =
            translate_rect_position_to_flat_index(start_position, self.dimensions)
                .expect("position is bounded");
        let row_end_index = translate_rect_position_to_flat_index(end_position, self.dimensions)
            .expect("position is bounded");

        &mut self.pixels[row_start_index..row_end_index + 1]
    }

    fn mut_pixel_at_position(&mut self, position: PixelPosition) -> Option<&mut Pixel> {
        translate_rect_position_to_flat_index(position, self.dimensions)
            .map(|index| &mut self.pixels[index])
    }

    fn mut_pixel_at_bounded_position(&mut self, position: DrawPosition) -> &mut Pixel {
        &mut self.pixels[translate_rect_position_to_flat_index(
            self.dimensions.bound_position(position).position,
            self.dimensions,
        )
        .expect("position is bounded")]
    }
}

impl<T: Deref<Target = [Pixel]>> Display for RasterChunk<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_window().fmt(f)
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

    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }

    pub fn dimensions(&self) -> Dimensions {
        self.dimensions
    }
}

impl<T: DerefMut<Target = [Pixel]>> RasterChunk<T> {
    fn perform_row_operation<F>(&mut self, draw_rect: DrawRect, operation: &mut F)
    where
        F: FnMut(&mut [Pixel]),
    {
        for row_num in 0..draw_rect.dimensions.height {
            if row_num >= self.dimensions.height + draw_rect.top_left.1 as usize {
                break;
            }

            let dest_slice = self.mut_bounded_subrow_from_position(
                draw_rect.top_left + (0, row_num as i32).into(),
                draw_rect.dimensions.width,
            );
            operation(dest_slice)
        }
    }

    fn perform_zipped_row_operation<S: RasterSource + Subsource>(
        &mut self,
        source: &S,
        dest_position: DrawPosition,
        operation: RowOperation,
    ) {
        let bounded_top_left = self.bound_position(dest_position);
        if let Some(shrunk_source) = source.subsource_within_at(&*self, dest_position) {
            for row_num in 0..shrunk_source.dimensions().height {
                let source_row = shrunk_source.row(row_num);

                let row_start_position = bounded_top_left.position + (0_usize, row_num).into();

                if let Some(source_row) = source_row {
                    let dest_slice = self
                        .mut_subrow_from_position(
                            row_start_position.unchecked_into_position(),
                            shrunk_source.dimensions().width,
                        )
                        .expect("subrow should never be larger than source here");

                    operation(dest_slice, source_row);
                }
            }
        }
    }

    /// Blits a render window onto the raster chunk at `dest_position`.
    /// If the window at `dest_position` is not contained within the chunk,
    /// the portion of the destination outside the chunk is ignored.
    pub fn blit<S: RasterSource + Subsource>(&mut self, source: &S, dest_position: DrawPosition) {
        self.perform_zipped_row_operation(source, dest_position, |d, s| d.copy_from_slice(s));
    }

    pub fn fill_rect(&mut self, pixel: Pixel, draw_rect: DrawRect) {
        self.perform_row_operation(draw_rect, &mut |d| d.fill(pixel));
    }

    /// Draws a render window onto the raster chunk at `dest_position` using alpha compositing.
    /// If the window at `dest_position` is not contained within the chunk,
    /// the portion of the destination outside the chunk is ignored.
    pub fn composite_over<S: RasterSource + Subsource>(
        &mut self,
        source: &S,
        dest_position: DrawPosition,
    ) {
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

        *self = self.nn_scaled(new_size);
    }

    /// A chunk scaled to a new size using the nearest-neighbour algorithm.
    pub fn nn_scaled(&mut self, new_size: Dimensions) -> BoxRasterChunk {
        let mut new_chunk = BoxRasterChunk::new(new_size.width, new_size.height);

        for (dest_position, source_position) in
            NearestNeighbourMappingIterator::new(self.dimensions, new_size)
        {
            let new_chunk_pixel = new_chunk
                .mut_pixel_at_position(dest_position)
                .expect("position should be contained in new chunk");

            *new_chunk_pixel = self
                .pixel_at_position(source_position)
                .expect("nn transformation result should always be in source");
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

        for (dest_position, source_position) in
            NearestNeighbourMappingIterator::new(self.dimensions, new_size)
        {
            let new_chunk_pixel = new_chunk
                .mut_pixel_at_position(dest_position)
                .expect("position should be contained in new chunk");

            *new_chunk_pixel = self
                .pixel_at_position(source_position)
                .expect("nn transformation result should always be in source");
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

        for (dest_position, source_position) in
            NearestNeighbourMappingIterator::new(self.dimensions, new_size)
        {
            let new_chunk_pixel = new_chunk
                .mut_pixel_at_position(dest_position)
                .expect("position should be contained in new chunk");

            *new_chunk_pixel = self
                .pixel_at_position(source_position)
                .expect("nn transformation result should always be in source");
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
