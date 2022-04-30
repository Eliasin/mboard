//! Collections of raster data and ways to draw and manipulate them.
//!
//! `RasterChunk` is a square-sized chunk of owned raster data that
//! can be blitted and alpha composited onto.
//!
//! `RasterWindow` is a borrow of some raster data, this can be a full
//! chunk or part of a `Pixel` slice.

use std::convert::TryInto;
use std::fmt::Display;

use super::pixels::{colors, Pixel};
use super::position::{DrawPosition, PixelPosition};

/// A square collection of pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterChunk {
    pixels: Box<[Pixel]>,
    width: usize,
    height: usize,
}

/// A reference to a sub-rectangle of a raster chunk.
#[derive(Debug, Clone, Copy)]
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
    x_delta: i64,
    y_delta: i64,
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

/// Failure to create a `RasterWindow` from a slice due to incompatible sizing.
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

fn get_color_character_for_pixel(p: &Pixel) -> &'static str {
    let mut color_characters = vec![
        (colors::red(), "r"),
        (colors::blue(), "b"),
        (colors::green(), "g"),
        (colors::black(), "B"),
        (colors::white(), "w"),
        (colors::transparent(), " "),
    ];

    color_characters.sort_by(|(a, _), (b, _)| {
        let d_a = p.eu_distance(a);
        let d_b = p.eu_distance(b);

        d_a.partial_cmp(&d_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    color_characters.get(0).unwrap().1
}

fn display_raster_row(row: &[Pixel]) -> String {
    let mut s = String::new();

    for p in row {
        s += get_color_character_for_pixel(p);
    }

    s
}

impl<'a> Display for RasterWindow<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for row_num in 0..self.height {
            let row_slice = self.get_row_slice(row_num).unwrap();
            s += "|";
            s += display_raster_row(row_slice).as_str();
            s += "|\n";
        }

        write!(f, "{}", s)
    }
}

impl Display for RasterChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_window().fmt(f)
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
        let over_width = top_left.0 .0 + width > chunk.width;
        let over_height = top_left.0 .1 + height > chunk.height;
        if over_width || over_height {
            None
        } else {
            Some(RasterWindow {
                backing: chunk.pixels.as_ref(),
                backing_height: chunk.height,
                backing_width: chunk.width,
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

    /// Creates a raster chunk by copying the data in a window.
    pub fn to_chunk(&self) -> RasterChunk {
        let mut rect = Vec::<Pixel>::with_capacity(self.width * self.height);

        for row in 0..self.height {
            for column in 0..self.width {
                let source_position = (column, row);

                let source_index = self
                    .get_index_from_position(source_position.into())
                    .unwrap();
                rect.push(self.backing[source_index]);
            }
        }

        RasterChunk {
            pixels: rect.into_boxed_slice(),
            width: self.width,
            height: self.height,
        }
    }
}

fn translate_rect_position_to_flat_index(
    position: (usize, usize),
    width: usize,
    height: usize,
) -> Option<usize> {
    let offset_from_row = position.1 * width;
    let offset_from_column = position.0;

    let over_width = position.0 >= width;
    let over_height = position.1 >= height;

    if over_width || over_height {
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
            x_delta: TryInto::<i64>::try_into(bounded_position.0 .0).unwrap() - position.0 .0,
            y_delta: TryInto::<i64>::try_into(bounded_position.0 .1).unwrap() - position.0 .1,
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
        translate_rect_position_to_flat_index(position.0, self.width, self.height)
    }

    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]> {
        let row_start = self.get_index_from_position((0, row_num).into())?;
        let row_end = self.get_index_from_position((self.width - 1, row_num).into())?;

        Some(&self.pixels[row_start..row_end + 1])
    }

    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex {
        let bounded_position = self.bound_position(position);

        let index =
            translate_rect_position_to_flat_index(bounded_position.0, self.width, self.height)
                .unwrap();

        BoundedIndex {
            index,
            x_delta: TryInto::<i64>::try_into(bounded_position.0 .0).unwrap() - position.0 .0,
            y_delta: TryInto::<i64>::try_into(bounded_position.0 .1).unwrap() - position.0 .1,
        }
    }

    fn bound_position(&self, position: DrawPosition) -> PixelPosition {
        PixelPosition((
            (TryInto::<usize>::try_into(position.0 .0.max(0)).unwrap()).min(self.width - 1),
            (TryInto::<usize>::try_into(position.0 .1.max(0)).unwrap()).min(self.height - 1),
        ))
    }
}

type RowOperation = fn(&mut [Pixel], &[Pixel]) -> ();

impl RasterChunk {
    /// Create a new raster chunk filled in with a pixel value.
    pub fn new_fill(pixel: Pixel, width: usize, height: usize) -> RasterChunk {
        let pixels = vec![pixel; width * height];

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            width,
            height,
        }
    }

    /// Create a new raster chunk where each pixel value is filled in by a closure given the pixel's location.
    pub fn new_fill_dynamic(
        f: fn(PixelPosition) -> Pixel,
        width: usize,
        height: usize,
    ) -> RasterChunk {
        let mut pixels = vec![colors::transparent(); width * height];

        for row in 0..width {
            for column in 0..height {
                pixels[row * width + column] = f(PixelPosition::from((row, column)));
            }
        }

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            width,
            height,
        }
    }

    /// Create a new raster chunk that is completely transparent.
    pub fn new(width: usize, height: usize) -> RasterChunk {
        RasterChunk::new_fill(colors::transparent(), width, height)
    }

    /// Derive a sub-chunk from a raster chunk. If the sub-chunk positioned at `position` is not fully contained by the source chunk,
    /// any regions outside the source chunk will be filled in as transparent.
    pub fn clone_square(
        &self,
        position: (usize, usize),
        width: usize,
        height: usize,
    ) -> RasterChunk {
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
            width,
            height,
        }
    }

    /// Takes the whole chunk as a raster window.
    pub fn as_window(&self) -> RasterWindow {
        RasterWindow {
            backing: self.pixels.as_ref(),
            top_left: (0, 0).into(),
            width: self.width,
            height: self.height,
            backing_height: self.height,
            backing_width: self.width,
        }
    }

    /// Shrinks a raster window to the sub-window that is contained within
    /// the current raster chunk. Returns `None` if the resultant window is empty.
    fn shrink_window_to_contain<'a>(
        &self,
        source: &RasterWindow<'a>,
        dest_position: DrawPosition,
    ) -> Option<RasterWindow<'a>> {
        if source.width == 0 || source.height == 0 {
            return None;
        }

        let source_top_left_in_dest = self.get_index_from_bounded_position(dest_position);

        let bottom_right: (i64, i64) = (
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
                    .get_index_from_position(bounded_top_left + (0_usize, row_num))
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
        // Optimization for blittig something completely over a chunk
        if source.width == self.width
            && source.height == self.height
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

    /// The dimensions of a raster chunk in `(width, height)` format.
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn test_position_translation() {
        let raster_chunk = RasterChunk::new(256, 256);

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
        let raster_chunk = RasterChunk::new(256, 256);

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
        let mut raster_chunk = RasterChunk::new(5, 5);

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
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (2, 2).into());

        let mut expected_raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        expected_raster_chunk.pixels[2 * 8 + 2] = colors::blue();
        expected_raster_chunk.pixels[2 * 8 + 3] = colors::blue();
        expected_raster_chunk.pixels[3 * 8 + 2] = colors::blue();
        expected_raster_chunk.pixels[3 * 8 + 3] = colors::blue();

        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);
    }

    #[test]
    fn test_complete_blit() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 8, 8);

        raster_chunk.blit(&blit_source.as_window(), (2, 2).into());

        assert_eq!(raster_chunk.pixels, blit_source.pixels);
    }

    #[test]
    fn test_blit_into_smaller() {
        let mut raster_chunk = RasterChunk::new(1, 1);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (0, 0).into());

        assert_eq!(raster_chunk.pixels[0], colors::blue());
    }

    /// Test that blits that are partially/totally outside the chunk work as expected.
    #[test]
    fn test_blit_overflow() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (7, 7).into());

        let mut expected_raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        expected_raster_chunk.pixels[7 * 8 + 7] = colors::blue();

        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);
    }

    #[test]
    fn test_noop_blit() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        let expected_raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = RasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (-3, -3).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);

        raster_chunk.blit(&blit_source.as_window(), (8, 8).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);

        raster_chunk.blit(&blit_source.as_window(), (-3, 0).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);

        raster_chunk.blit(&blit_source.as_window(), (8, 0).into());
        assert_eq!(expected_raster_chunk.pixels, raster_chunk.pixels);
    }

    #[test]
    fn test_window_shrink() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        raster_chunk.pixels[3 * 8 + 4] = colors::blue();

        let raster_window: RasterWindow<'_> = raster_chunk.as_window();

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
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 8, 8);

        let draw_source = RasterChunk::new_fill(colors::blue(), 8, 8);

        raster_chunk.composite_over(&draw_source.as_window(), (0, 0).into());

        let blended_pixel = Pixel::new_rgb(0, 0, 255);

        for pixel in raster_chunk.pixels.iter() {
            assert!(pixel.is_close(&blended_pixel, 2));
        }
    }

    #[test]
    fn test_medium_compositing() {
        let mut raster_chunk = RasterChunk::new_fill(Pixel::new_rgb(128, 128, 128), 8, 8);

        let draw_source = RasterChunk::new_fill(Pixel::new_rgba(255, 255, 255, 128), 8, 8);

        raster_chunk.composite_over(&draw_source.as_window(), (0, 0).into());

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
            4,
        );

        let mut expected_checkerboard_chunk = RasterChunk::new_fill(colors::blue(), 4, 4);

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
            3,
        );

        let mut expected_gradient_chunk = RasterChunk::new(3, 3);

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
            assert!(pixel.is_close(expected_pixel, 2));
        }
    }

    #[test]
    fn test_window_to_chunk() {
        let mut raster_chunk = RasterChunk::new_fill(colors::red(), 3, 4);

        raster_chunk.pixels[3 + 2] = colors::blue();

        let raster_window = RasterWindow::new(&raster_chunk, (1, 1).into(), 2, 2).unwrap();

        let new_chunk = raster_window.to_chunk();

        let mut expected_chunk = RasterChunk::new_fill(colors::red(), 2, 2);

        expected_chunk.pixels[1] = colors::blue();

        assert_eq!(new_chunk, expected_chunk);
    }

    #[test]
    fn test_new_window_edge_cases() {
        let raster_chunk = RasterChunk::new(10, 10);

        let raster_window_close = RasterWindow::new(&raster_chunk, (1, 1).into(), 9, 9);

        assert!(raster_window_close.is_some());

        let raster_window_over = RasterWindow::new(&raster_chunk, (1, 1).into(), 9, 10);

        assert!(raster_window_over.is_none());

        let raster_window_over_both = RasterWindow::new(&raster_chunk, (1, 1).into(), 11, 11);

        assert!(raster_window_over_both.is_none());
    }
}
