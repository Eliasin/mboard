use std::convert::TryInto;

use super::pixels::{colors, Pixel};
use super::position::{DrawPosition, PixelPosition};

/// A square collection of pixels.
#[derive(Debug)]
pub struct RasterChunk {
    pixels: Box<[Pixel]>,
    size: usize,
}

/// A reference to a sub-rectangle of a raster chunk.
#[derive(Debug)]
pub struct RasterWindow<'a> {
    backing: &'a [Pixel],
    top_left: PixelPosition,
    width: usize,
    height: usize,
    backing_width: usize,
    backing_height: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct BoundedIndex {
    index: usize,
    x_delta: i32,
    y_delta: i32,
}

/// A value that can be indexed by `PixelPosition`, providing pixels. It must make sense to get slices representing rows from the value.
trait IndexableByPosition {
    /// Returns an index to the backing collection that corresponds to the position supplied.
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize>;
    /// Returns a bounded index to the backing collection along with the shift applied to bound the
    /// position within the collection.
    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex;
    /// Returns a bit position bounded into the underlying collection.
    fn bound_position(&self, position: DrawPosition) -> PixelPosition;
    /// Returns a slice representing a row of pixels.
    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]>;
}

/// Converts the entire chunk into a window.
impl<'a> Into<RasterWindow<'a>> for &'a RasterChunk {
    fn into(self) -> RasterWindow<'a> {
        RasterWindow {
            backing: self.pixels.as_ref(),
            top_left: (0, 0).into(),
            width: self.size,
            height: self.size,
            backing_height: self.size,
            backing_width: self.size,
        }
    }
}

#[derive(Debug)]
pub struct InvalidPixelSliceSize {
    desired_width: usize,
    desired_height: usize,
    buffer_size: usize,
}

impl std::fmt::Display for InvalidPixelSliceSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot make ({}, {}) from buffer of size {}",
            self.desired_width, self.desired_height, self.buffer_size
        )
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
        if top_left.0 .0 + width > chunk.size {
            None
        } else if top_left.0 .1 + height > chunk.size {
            None
        } else {
            Some(RasterWindow {
                backing: chunk.pixels.as_ref(),
                backing_height: chunk.size,
                backing_width: chunk.size,
                top_left,
                width,
                height,
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
                backing_height: height,
                backing_width: width,
                top_left: (0, 0).into(),
                height,
                width,
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
        if left + right >= self.width || top + bottom >= self.height {
            return None;
        }

        let new_top_left = self.top_left + PixelPosition::from((top, left));

        let new_width = self.width - right - left;
        let new_height = self.height - bottom - top;

        if new_top_left.0 .0 > self.backing_width || new_top_left.0 .1 > self.backing_height {
            return None;
        }

        Some(RasterWindow {
            backing: self.backing,
            top_left: new_top_left,
            height: new_height,
            width: new_width,
            backing_height: self.backing_height,
            backing_width: self.backing_width,
        })
    }
}

fn translate_rect_position_to_flat_index(
    position: (usize, usize),
    width: usize,
    height: usize,
) -> Option<usize> {
    let offset_from_row = position.1 * width;
    let offset_from_column = position.0;

    if position.0 >= height {
        None
    } else if offset_from_column >= width {
        None
    } else {
        Some(offset_from_row + offset_from_column)
    }
}

impl<'a> IndexableByPosition for RasterWindow<'a> {
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize> {
        if position.0 .0 > self.width || position.0 .1 > self.height {
            None
        } else {
            translate_rect_position_to_flat_index(
                (position + self.top_left).0,
                self.backing_width,
                self.backing_height,
            )
        }
    }

    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start = self.get_index_from_position((0, row_num).into())?;
        let row_end = self.get_index_from_position((self.width - 1, row_num).into())?;

        Some(&self.backing[row_start..row_end + 1])
    }

    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex {
        let bounded_position = self.bound_position(position);

        // Since we bound x and y, this is guaranteed to not panic as long as the total area is
        // not 0.
        let index = translate_rect_position_to_flat_index(
            (bounded_position + self.top_left).0,
            self.backing_width,
            self.backing_height,
        )
        .unwrap();

        BoundedIndex {
            index,
            x_delta: TryInto::<i32>::try_into(bounded_position.0 .0).unwrap() - position.0 .0,
            y_delta: TryInto::<i32>::try_into(bounded_position.0 .1).unwrap() - position.0 .1,
        }
    }

    fn bound_position(&self, position: DrawPosition) -> PixelPosition {
        PixelPosition((
            (TryInto::<usize>::try_into(position.0 .0.max(0)).unwrap()).min(self.width - 1),
            (TryInto::<usize>::try_into(position.0 .1.max(0)).unwrap()).min(self.height - 1),
        ))
    }
}

impl IndexableByPosition for RasterChunk {
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize> {
        translate_rect_position_to_flat_index(position.0, self.size, self.size)
    }

    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start = self.get_index_from_position((0, row_num).into())?;
        let row_end = self.get_index_from_position((self.size - 1, row_num).into())?;

        Some(&self.pixels[row_start..row_end + 1])
    }

    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex {
        let bounded_position = self.bound_position(position);

        let index = translate_rect_position_to_flat_index(bounded_position.0, self.size, self.size)
            .unwrap();

        BoundedIndex {
            index,
            x_delta: TryInto::<i32>::try_into(bounded_position.0 .0).unwrap() - position.0 .0,
            y_delta: TryInto::<i32>::try_into(bounded_position.0 .1).unwrap() - position.0 .1,
        }
    }

    fn bound_position(&self, position: DrawPosition) -> PixelPosition {
        PixelPosition((
            (TryInto::<usize>::try_into(position.0 .0.max(0)).unwrap()).min(self.size - 1),
            (TryInto::<usize>::try_into(position.0 .1.max(0)).unwrap()).min(self.size - 1),
        ))
    }
}

type RowOperation = fn(&mut [Pixel], &[Pixel]) -> ();

impl RasterChunk {
    /// Create a new raster chunk filled in with a pixel value.
    pub fn new_fill(pixel: Pixel, size: usize) -> RasterChunk {
        let pixels = vec![pixel; size * size];

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            size,
        }
    }

    /// Create a new raster chunk where each pixel value is filled in by a closure given the pixel's location.
    pub fn new_fill_dynamic(f: fn(PixelPosition) -> Pixel, size: usize) -> RasterChunk {
        let mut pixels = vec![colors::transparent(); size * size];

        for row in 0..size {
            for column in 0..size {
                pixels[row * size + column] = f(PixelPosition::from((row, column)));
            }
        }

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            size,
        }
    }

    /// Create a new raster chunk that is completely transparent.
    pub fn new(size: usize) -> RasterChunk {
        RasterChunk::new_fill(colors::transparent(), size)
    }

    /// Derive a sub-chunk from a raster chunk. If the sub-chunk positioned at `position` is not fully contained by the source chunk,
    /// any regions outside the source chunk will be filled in as transparent.
    pub fn clone_square(&self, position: (usize, usize), size: usize) -> RasterChunk {
        let mut square = Vec::<Pixel>::with_capacity(size * size);

        for column in 0..size {
            for row in 0..size {
                let source_position = (row + position.0, column + position.1);

                if let Some(source_index) = self.get_index_from_position(source_position.into()) {
                    square.push(self.pixels[source_index]);
                } else {
                    square.push(colors::transparent());
                }
            }
        }

        RasterChunk {
            pixels: square.into_boxed_slice(),
            size,
        }
    }

    /// Shrinks a raster window to the sub-window that is contained within
    /// the current raster chunk. Returns `None` if the resultant window is empty.
    fn shrink_window_to_contain<'a>(
        &self,
        source: &RasterWindow<'a>,
        dest_position: DrawPosition,
    ) -> Option<RasterWindow<'a>> {
        let source_top_left_in_dest = self.get_index_from_bounded_position(dest_position);

        let bottom_right: (i32, i32) = (
            (source.width - 1).try_into().unwrap(),
            (source.height - 1).try_into().unwrap(),
        );
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

        let shrink_top = source_top_left_in_dest.y_delta.try_into().unwrap();
        let shrink_bottom = (-source_bottom_right_in_dest.y_delta).try_into().unwrap();

        let shrink_left = source_top_left_in_dest.x_delta.try_into().unwrap();
        let shrink_right = (-source_bottom_right_in_dest.x_delta).try_into().unwrap();

        source.shrink(shrink_top, shrink_bottom, shrink_left, shrink_right)
    }

    /// Performs an operation on the chunk rows using rows from a window at a specified location.
    fn perform_row_operations(
        &mut self,
        source: &RasterWindow,
        dest_position: DrawPosition,
        operation: RowOperation,
    ) {
        let bounded_top_left = self.bound_position(dest_position);
        if let Some(shrunk_source) = self.shrink_window_to_contain(source, dest_position) {
            for row_num in 0..shrunk_source.height {
                let source_row = shrunk_source.get_row_slice(row_num);

                let start = self
                    .get_index_from_position(bounded_top_left + (0 as usize, row_num))
                    .unwrap();
                let end = self
                    .get_index_from_position(bounded_top_left + (shrunk_source.width - 1, row_num))
                    .unwrap();

                if let Some(source_row) = source_row {
                    let dest_slice = &mut self.pixels[start..end + 1];

                    operation(dest_slice, source_row);
                }
            }
        }
    }

    /// Blits a render window onto the raster chunk at `dest_position`.
    /// If the window at `dest_position` is not contained within the chunk,
    /// the portion of the destination outside the chunk is ignored.
    pub fn blit(&mut self, source: &RasterWindow, dest_position: DrawPosition) {
        // Optimization for blitting something completely over a chunk
        if source.width == self.size
            && source.height == self.size
            && source.backing.len() == self.pixels.len()
        {
            self.pixels.copy_from_slice(source.backing);
            return;
        }

        self.perform_row_operations(source, dest_position, |d, s| d.copy_from_slice(s));
    }

    /// Draws a render window onto the raster chunk at `dest_position` using alpha compositing.
    /// If the window at `dest_position` is not contained within the chunk,
    /// the portion of the destination outside the chunk is ignored.
    pub fn composite_over(&mut self, source: &RasterWindow, dest_position: DrawPosition) {
        self.perform_row_operations(source, dest_position, |d, s| {
            for (pixel_d, pixel_s) in d.iter_mut().zip(s.iter()) {
                pixel_d.composite_over(pixel_s);
            }
        });
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_position_translation() {
        let raster_chunk = RasterChunk::new(256);

        assert_eq!(Some(0), raster_chunk.get_index_from_position((0, 0).into()));
        assert_eq!(
            Some(256),
            raster_chunk.get_index_from_position((0, 1).into())
        );
        assert_eq!(
            Some(256 + 1),
            raster_chunk.get_index_from_position((1, 1).into())
        );
        assert_eq!(
            Some(1024 + 56),
            raster_chunk.get_index_from_position((56, 4).into())
        );

        assert_eq!(
            Some(256 * 256 - 1),
            raster_chunk.get_index_from_position((255, 255).into())
        );

        let raster_window = RasterWindow::new(&raster_chunk, (64, 64).into(), 64, 64).unwrap();

        assert_eq!(
            Some((64 + 32) * 256 + (64 + 32)),
            raster_window.get_index_from_position((32, 32).into())
        );
    }

    #[test]
    fn test_bounded_position_translation() {
        let raster_chunk = RasterChunk::new(256);

        assert_eq!(
            BoundedIndex {
                index: 0,
                x_delta: 0,
                y_delta: 0
            },
            raster_chunk.get_index_from_bounded_position((0, 0).into())
        );

        assert_eq!(
            BoundedIndex {
                index: 0,
                x_delta: 1,
                y_delta: 1
            },
            raster_chunk.get_index_from_bounded_position((-1, -1).into())
        );

        assert_eq!(
            BoundedIndex {
                index: 0,
                x_delta: 4,
                y_delta: 1
            },
            raster_chunk.get_index_from_bounded_position((-4, -1).into())
        );

        assert_eq!(
            BoundedIndex {
                index: 255,
                x_delta: -1,
                y_delta: 1
            },
            raster_chunk.get_index_from_bounded_position((256, -1).into())
        );

        assert_eq!(
            BoundedIndex {
                index: 256 * 256 - 1,
                x_delta: -1,
                y_delta: -1
            },
            raster_chunk.get_index_from_bounded_position((256, 256).into())
        );

        assert_eq!(
            BoundedIndex {
                index: 256 * 256 - 1,
                x_delta: -3,
                y_delta: -2
            },
            raster_chunk.get_index_from_bounded_position((258, 257).into())
        );
    }

    #[test]
    fn test_getting_row_slices() {
        let mut raster_chunk = RasterChunk::new(5);

        raster_chunk.pixels[5 + 1] = colors::blue();
        raster_chunk.pixels[5 + 2] = colors::blue();
        raster_chunk.pixels[5 + 4] = colors::red();

        let chunk_row = raster_chunk.get_row_slice(1).unwrap();
        let mut expected_chunk_row = [colors::transparent(); 5];

        expected_chunk_row[1] = colors::blue();
        expected_chunk_row[2] = colors::blue();
        expected_chunk_row[4] = colors::red();

        assert_eq!(chunk_row, expected_chunk_row);

        let raster_window = RasterWindow::new(&raster_chunk, (1, 1).into(), 3, 3).unwrap();

        let window_row = raster_window.get_row_slice(0).unwrap();

        let mut expected_window_row = [colors::transparent(); 3];

        expected_window_row[0] = colors::blue();
        expected_window_row[1] = colors::blue();

        assert_eq!(window_row, expected_window_row);
    }

    #[test]
    fn test_blitting() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2);

        raster_chunk.blit(&(&blit_source).into(), (2, 2).into());

        let mut expected_raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        expected_raster_chunk.pixels[2 * 8 + 2] = colors::blue();
        expected_raster_chunk.pixels[2 * 8 + 3] = colors::blue();
        expected_raster_chunk.pixels[3 * 8 + 2] = colors::blue();
        expected_raster_chunk.pixels[3 * 8 + 3] = colors::blue();

        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);
    }

    #[test]
    fn test_complete_blit() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 8);

        raster_chunk.blit(&(&blit_source).into(), (2, 2).into());

        assert_eq!(raster_chunk.pixels, blit_source.pixels);
    }

    #[test]
    fn test_blit_into_smaller() {
        let mut raster_chunk = RasterChunk::new(1);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2);

        raster_chunk.blit(&(&blit_source).into(), (0, 0).into());

        assert_eq!(raster_chunk.pixels[0], colors::blue());
    }

    /// Test that blits that are partially/totally outside the chunk work as expected.
    #[test]
    fn test_blit_overflow() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2);

        raster_chunk.blit(&(&blit_source).into(), (7, 7).into());

        let mut expected_raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        expected_raster_chunk.pixels[7 * 8 + 7] = colors::blue();

        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);
    }

    #[test]
    fn test_noop_blit() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        let expected_raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2);

        raster_chunk.blit(&(&blit_source).into(), (-3, -3).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);

        raster_chunk.blit(&(&blit_source).into(), (8, 8).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);

        raster_chunk.blit(&(&blit_source).into(), (-3, 0).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);

        raster_chunk.blit(&(&blit_source).into(), (8, 0).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);
    }

    #[test]
    fn test_window_shrink() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        raster_chunk.pixels[3 * 8 + 4] = colors::blue();

        let raster_window: RasterWindow<'_> = (&raster_chunk).into();

        let shrunk = raster_window.shrink(1, 1, 1, 1).unwrap();
        let expected_shrunk = RasterWindow::new(&raster_chunk, (1, 1).into(), 6, 6).unwrap();

        assert_eq!(shrunk.height, expected_shrunk.height);

        for row in 0..shrunk.height {
            let shrunk_row = shrunk.get_row_slice(row).unwrap();
            let expected_row = expected_shrunk.get_row_slice(row).unwrap();

            assert_eq!(shrunk_row, expected_row);
        }

        assert!(raster_window.shrink(4, 3, 3, 4).is_some());

        assert!(raster_window.shrink(4, 4, 0, 0).is_none());
        assert!(raster_window.shrink(3, 4, 4, 4).is_none());
    }

    #[test]
    fn test_easy_compositing() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8);

        let draw_source = RasterChunk::new_fill(colors::blue(), 8);

        raster_chunk.composite_over(&(&draw_source).into(), (0, 0).into());

        let blended_pixel = Pixel::new_rgb(0, 0, 255);

        for pixel in raster_chunk.pixels.iter() {
            assert!(pixel.is_close(&blended_pixel, 2));
        }
    }

    #[test]
    fn test_medium_compositing() {
        let mut raster_chunk = RasterChunk::new_fill(Pixel::new_rgb(128, 128, 128), 8);

        let draw_source = RasterChunk::new_fill(Pixel::new_rgba(255, 255, 255, 128), 8);

        raster_chunk.composite_over(&(&draw_source).into(), (0, 0).into());

        let blended_pixel = Pixel::new_rgb(191, 191, 191);

        for pixel in raster_chunk.pixels.iter() {
            assert!(pixel.is_close(&blended_pixel, 2));
        }
    }

    #[test]
    fn test_dynamic_fill_checkerboard() {
        let checkerboard_chunk = RasterChunk::new_fill_dynamic(
            |p| {
                let mut is_red = true;
                if p.0 .0 % 2 == 0 {
                    is_red = !is_red;
                }

                if p.0 .1 % 2 == 0 {
                    is_red = !is_red;
                }

                if is_red {
                    colors::red()
                } else {
                    colors::blue()
                }
            },
            4,
        );

        let mut expected_checkerboard_chunk = RasterChunk::new_fill(colors::blue(), 4);

        expected_checkerboard_chunk.pixels[0] = colors::red();
        expected_checkerboard_chunk.pixels[2] = colors::red();

        expected_checkerboard_chunk.pixels[5] = colors::red();
        expected_checkerboard_chunk.pixels[7] = colors::red();

        expected_checkerboard_chunk.pixels[8] = colors::red();
        expected_checkerboard_chunk.pixels[10] = colors::red();

        expected_checkerboard_chunk.pixels[13] = colors::red();
        expected_checkerboard_chunk.pixels[15] = colors::red();

        assert_eq!(
            expected_checkerboard_chunk.pixels,
            checkerboard_chunk.pixels
        );
    }

    #[test]
    fn test_dynamic_fill_gradient() {
        let gradient_chunk = RasterChunk::new_fill_dynamic(
            |p| {
                Pixel::new_rgb_norm(
                    (1.0 + p.0 .1 as f32) / 3.0,
                    0.0,
                    (1.0 + p.0 .0 as f32) / 3.0,
                )
            },
            3,
        );

        let mut expected_gradient_chunk = RasterChunk::new(3);

        expected_gradient_chunk.pixels[8] = Pixel::new_rgb_norm(1.0, 0.0, 1.0);
        expected_gradient_chunk.pixels[7] = Pixel::new_rgb_norm(2.0 / 3.0, 0.0, 1.0);
        expected_gradient_chunk.pixels[6] = Pixel::new_rgb_norm(1.0 / 3.0, 0.0, 1.0);

        expected_gradient_chunk.pixels[5] = Pixel::new_rgb_norm(1.0, 0.0, 2.0 / 3.0);
        expected_gradient_chunk.pixels[4] = Pixel::new_rgb_norm(2.0 / 3.0, 0.0, 2.0 / 3.0);
        expected_gradient_chunk.pixels[3] = Pixel::new_rgb_norm(1.0 / 3.0, 0.0, 2.0 / 3.0);

        expected_gradient_chunk.pixels[2] = Pixel::new_rgb_norm(1.0, 0.0, 1.0 / 3.0);
        expected_gradient_chunk.pixels[1] = Pixel::new_rgb_norm(2.0 / 3.0, 0.0, 1.0 / 3.0);
        expected_gradient_chunk.pixels[0] = Pixel::new_rgb_norm(1.0 / 3.0, 0.0, 1.0 / 3.0);

        for (pixel, expected_pixel) in gradient_chunk
            .pixels
            .iter()
            .zip(expected_gradient_chunk.pixels.iter())
        {
            println!("{:?}, {:?}", pixel.as_rgba(), expected_pixel.as_rgba());
            assert!(pixel.is_close(expected_pixel, 2));
        }
    }
}
