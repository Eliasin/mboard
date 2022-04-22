pub mod pixels {
    use std::convert::TryInto;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Pixel(u32);

    impl Pixel {
        pub fn new_rgb(r: u8, g: u8, b: u8) -> Pixel {
            Pixel::new_rgba(r, g, b, 255)
        }

        pub fn new_rgba(r: u8, g: u8, b: u8, a: u8) -> Pixel {
            let r = r as u32;
            let g = g as u32;
            let b = b as u32;
            let a = a as u32;
            Pixel(r + (g << 8) + (b << 16) + (a << 24))
        }

        pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
            let r = self.0 & 0xFF;
            let g = self.0 & 0xFF00;
            let b = self.0 & 0xFF0000;
            let a = self.0 & 0xFF000000;

            (
                r.try_into().unwrap(),
                g.try_into().unwrap(),
                b.try_into().unwrap(),
                a.try_into().unwrap(),
            )
        }
    }

    pub mod colors {
        use super::Pixel;

        pub fn red() -> Pixel {
            Pixel::new_rgb(255, 0, 0)
        }

        pub fn green() -> Pixel {
            Pixel::new_rgb(0, 255, 0)
        }

        pub fn blue() -> Pixel {
            Pixel::new_rgb(0, 0, 255)
        }

        pub fn transparent() -> Pixel {
            Pixel::new_rgba(0, 0, 0, 0)
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PixelPosition((usize, usize));

impl Add for PixelPosition {
    type Output = PixelPosition;

    fn add(self, rhs: Self) -> Self::Output {
        PixelPosition((self.0 .0 + rhs.0 .0, self.0 .1 + rhs.0 .1))
    }
}

impl From<(usize, usize)> for PixelPosition {
    fn from(p: (usize, usize)) -> Self {
        PixelPosition(p)
    }
}

use std::ops::Add;

use pixels::{colors, Pixel};

pub struct RasterChunk {
    pixels: Box<[Pixel]>,
    size: usize,
}

/// A reference to a square subsection of a raster chunk.
pub struct RasterChunkWindow<'a> {
    backing: &'a [Pixel],
    top_left: PixelPosition,
    width: usize,
    height: usize,
    backing_width: usize,
    backing_height: usize,
}

trait IndexableByPosition {
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize>;
    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]>;
}

impl<'a> Into<RasterChunkWindow<'a>> for &'a RasterChunk {
    fn into(self) -> RasterChunkWindow<'a> {
        RasterChunkWindow {
            backing: self.pixels.as_ref(),
            top_left: (0, 0).into(),
            width: self.size,
            height: self.size,
            backing_height: self.size,
            backing_width: self.size,
        }
    }
}

impl<'a> RasterChunkWindow<'a> {
    pub fn new(
        chunk: &'a RasterChunk,
        top_left: PixelPosition,
        width: usize,
        height: usize,
    ) -> Option<RasterChunkWindow<'a>> {
        if top_left.0 .0 + width > chunk.size {
            None
        } else if top_left.0 .1 + height > chunk.size {
            None
        } else {
            Some(RasterChunkWindow {
                backing: chunk.pixels.as_ref(),
                backing_height: chunk.size,
                backing_width: chunk.size,
                top_left,
                width,
                height,
            })
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

    if position.0 >= height {
        None
    } else if offset_from_column >= width {
        None
    } else {
        Some(offset_from_row + offset_from_column)
    }
}

impl<'a> IndexableByPosition for RasterChunkWindow<'a> {
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
}

impl RasterChunk {
    pub fn new_fill(pixel: Pixel, size: usize) -> RasterChunk {
        let pixels = vec![pixel; size * size];

        RasterChunk {
            pixels: pixels.into_boxed_slice(),
            size,
        }
    }

    pub fn new(size: usize) -> RasterChunk {
        RasterChunk::new_fill(pixels::colors::transparent(), size)
    }

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

    pub fn blit(&mut self, source: &RasterChunkWindow, dest_position: PixelPosition) {
        // Optimization for blitting something completely over a chunk
        if source.width == self.size
            && source.height == self.size
            && source.backing.len() == self.pixels.len()
        {
            self.pixels.copy_from_slice(source.backing);
            return;
        }

        for row_num in 0..source.height {
            let source_row = source.get_row_slice(row_num);

            let dest_slice_start =
                self.get_index_from_position(dest_position + (0, row_num).into());
            let dest_slice_end =
                self.get_index_from_position(dest_position + (source.width, row_num).into());

            if let Some(source_row) = source_row && let Some(start) = dest_slice_start && let Some(end) = dest_slice_end {
                let dest_slice = &mut self.pixels[start..end];
                dest_slice.copy_from_slice(source_row);
            }
        }
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

        let raster_window = RasterChunkWindow::new(&raster_chunk, (64, 64).into(), 64, 64).unwrap();

        assert_eq!(
            Some((64 + 32) * 256 + (64 + 32)),
            raster_window.get_index_from_position((32, 32).into())
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

        let raster_window = RasterChunkWindow::new(&raster_chunk, (1, 1).into(), 3, 3).unwrap();

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
}
