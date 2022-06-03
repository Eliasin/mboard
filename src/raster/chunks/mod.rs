//! Collections of raster data and ways to draw and manipulate them.
//!
//! `BoxRasterChunk` is a square-sized chunk of owned raster data that
//! can be blitted and alpha composited onto.
//!
//! `RasterWindow` is a borrow of some raster data, this can be a full
//! chunk or part of a `Pixel` slice.

pub mod nn_map;
pub mod raster_chunk;
pub mod raster_window;
mod util;

pub use raster_chunk::BoxRasterChunk;
pub use raster_window::RasterWindow;
pub use util::translate_rect_position_to_flat_index;
pub use util::IndexableByPosition;

#[cfg(test)]
mod tests {
    use super::{raster_chunk::BoxRasterChunk, raster_window::*, util::*};
    use crate::{
        assert_raster_eq,
        raster::{
            pixels::{colors, Pixel},
            position::{Dimensions, DrawPosition, PixelPosition},
        },
    };

    #[test]
    fn test_position_translation() {
        let raster_chunk = BoxRasterChunk::new(256, 256);

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
        let raster_chunk = BoxRasterChunk::new(256, 256);

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
        let mut pixels = vec![colors::transparent(); 5 * 5];

        pixels[5 + 1] = colors::blue();
        pixels[5 + 2] = colors::blue();
        pixels[5 + 4] = colors::red();

        let raster_chunk = BoxRasterChunk::from_vec(pixels, 5, 5).unwrap();

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
        let mut raster_chunk = BoxRasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = BoxRasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (2, 2).into());

        let mut pixels = vec![colors::red(); 8 * 8];

        pixels[2 * 8 + 2] = colors::blue();
        pixels[2 * 8 + 3] = colors::blue();
        pixels[3 * 8 + 2] = colors::blue();
        pixels[3 * 8 + 3] = colors::blue();

        let expected_raster_chunk = BoxRasterChunk::from_vec(pixels, 8, 8).unwrap();

        assert_eq!(expected_raster_chunk.pixels(), raster_chunk.pixels());
    }

    #[test]
    fn test_complete_blit() {
        let mut raster_chunk = BoxRasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = BoxRasterChunk::new_fill(colors::blue(), 8, 8);

        raster_chunk.blit(&blit_source.as_window(), (0, 0).into());

        assert_eq!(raster_chunk.pixels(), blit_source.pixels());
    }

    #[test]
    fn test_blit_into_smaller() {
        let mut raster_chunk = BoxRasterChunk::new(1, 1);

        let blit_source = BoxRasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (0, 0).into());

        assert_eq!(raster_chunk.pixels()[0], colors::blue());
    }

    /// Test that blits that are partially/totally outside the chunk work as expected.
    #[test]
    fn test_blit_overflow() {
        let mut raster_chunk = BoxRasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = BoxRasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (7, 7).into());

        let mut pixels = vec![colors::red(); 8 * 8];

        pixels[7 * 8 + 7] = colors::blue();
        let expected_raster_chunk = BoxRasterChunk::from_vec(pixels, 8, 8).unwrap();

        assert_raster_eq!(expected_raster_chunk, raster_chunk);
    }

    #[test]
    fn test_noop_blit() {
        let mut raster_chunk = BoxRasterChunk::new_fill(colors::red(), 8, 8);

        let expected_raster_chunk = BoxRasterChunk::new_fill(colors::red(), 8, 8);

        let blit_source = BoxRasterChunk::new_fill(colors::blue(), 2, 2);

        raster_chunk.blit(&blit_source.as_window(), (-3, -3).into());
        assert_raster_eq!(expected_raster_chunk, raster_chunk);

        raster_chunk.blit(&blit_source.as_window(), (8, 8).into());
        assert_raster_eq!(expected_raster_chunk, raster_chunk);

        raster_chunk.blit(&blit_source.as_window(), (-3, 0).into());
        assert_raster_eq!(expected_raster_chunk, raster_chunk);

        raster_chunk.blit(&blit_source.as_window(), (8, 0).into());
        assert_raster_eq!(expected_raster_chunk, raster_chunk);
    }

    #[test]
    fn test_window_shrink() {
        let mut pixels = vec![colors::red(); 8 * 8];

        pixels[3 * 8 + 4] = colors::blue();

        let raster_chunk = BoxRasterChunk::from_vec(pixels, 8, 8).unwrap();

        let raster_window: RasterWindow<'_> = raster_chunk.as_window();

        let shrunk = raster_window.shrink(1, 1, 1, 1).unwrap();
        let expected_shrunk = RasterWindow::new(&raster_chunk, (1, 1).into(), 6, 6).unwrap();

        assert_eq!(
            shrunk.dimensions().height,
            expected_shrunk.dimensions().height
        );

        for row in 0..shrunk.dimensions().height {
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
        let mut raster_chunk = BoxRasterChunk::new_fill(colors::red(), 8, 8);

        let draw_source = BoxRasterChunk::new_fill(colors::blue(), 8, 8);

        raster_chunk.composite_over(&draw_source.as_window(), (0, 0).into());

        let blended_pixel = Pixel::new_rgb(0, 0, 255);

        for pixel in raster_chunk.pixels().iter() {
            assert!(pixel.is_close(&blended_pixel, 2));
        }
    }

    #[test]
    fn test_medium_compositing() {
        let mut raster_chunk = BoxRasterChunk::new_fill(Pixel::new_rgb(128, 128, 128), 8, 8);

        let draw_source = BoxRasterChunk::new_fill(Pixel::new_rgba(255, 255, 255, 128), 8, 8);

        raster_chunk.composite_over(&draw_source.as_window(), (0, 0).into());

        let blended_pixel = Pixel::new_rgb(191, 191, 191);

        for pixel in raster_chunk.pixels().iter() {
            assert!(pixel.is_close(&blended_pixel, 2));
        }
    }

    #[test]
    fn test_dynamic_fill_checkerboard() {
        let checkerboard_chunk = BoxRasterChunk::new_fill_dynamic(
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

        let mut checkerboard_pixels = vec![colors::blue(); 4 * 4];

        checkerboard_pixels[0] = colors::red();
        checkerboard_pixels[2] = colors::red();

        checkerboard_pixels[5] = colors::red();
        checkerboard_pixels[7] = colors::red();

        checkerboard_pixels[8] = colors::red();
        checkerboard_pixels[10] = colors::red();

        checkerboard_pixels[13] = colors::red();
        checkerboard_pixels[15] = colors::red();

        let expected_checkerboard_chunk =
            BoxRasterChunk::from_vec(checkerboard_pixels, 4, 4).unwrap();

        assert_raster_eq!(expected_checkerboard_chunk, checkerboard_chunk);
    }

    #[test]
    fn test_dynamic_fill_gradient() {
        let gradient_chunk = BoxRasterChunk::new_fill_dynamic(
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

        let mut gradient_pixels = vec![colors::transparent(); 3 * 3];

        gradient_pixels[8] = Pixel::new_rgb_norm(1.0, 0.0, 1.0);
        gradient_pixels[7] = Pixel::new_rgb_norm(2.0 / 3.0, 0.0, 1.0);
        gradient_pixels[6] = Pixel::new_rgb_norm(1.0 / 3.0, 0.0, 1.0);

        gradient_pixels[5] = Pixel::new_rgb_norm(1.0, 0.0, 2.0 / 3.0);
        gradient_pixels[4] = Pixel::new_rgb_norm(2.0 / 3.0, 0.0, 2.0 / 3.0);
        gradient_pixels[3] = Pixel::new_rgb_norm(1.0 / 3.0, 0.0, 2.0 / 3.0);

        gradient_pixels[2] = Pixel::new_rgb_norm(1.0, 0.0, 1.0 / 3.0);
        gradient_pixels[1] = Pixel::new_rgb_norm(2.0 / 3.0, 0.0, 1.0 / 3.0);
        gradient_pixels[0] = Pixel::new_rgb_norm(1.0 / 3.0, 0.0, 1.0 / 3.0);

        let expected_gradient_chunk = BoxRasterChunk::from_vec(gradient_pixels, 3, 3).unwrap();

        for (pixel, expected_pixel) in gradient_chunk
            .pixels()
            .iter()
            .zip(expected_gradient_chunk.pixels().iter())
        {
            assert!(pixel.is_close(expected_pixel, 2));
        }
    }

    #[test]
    fn test_window_to_chunk() {
        let mut pixels = vec![colors::red(); 3 * 4];

        pixels[3 + 2] = colors::blue();

        let raster_chunk = BoxRasterChunk::from_vec(pixels, 3, 4).unwrap();

        let raster_window = RasterWindow::new(&raster_chunk, (1, 1).into(), 2, 2).unwrap();

        let new_chunk = raster_window.to_chunk();

        let mut expected_pixels = vec![colors::red(); 2 * 2];
        expected_pixels[1] = colors::blue();

        let expected_chunk = BoxRasterChunk::from_vec(expected_pixels, 2, 2).unwrap();

        assert_raster_eq!(new_chunk, expected_chunk);
    }

    #[test]
    fn test_new_window_edge_cases() {
        let raster_chunk = BoxRasterChunk::new(10, 10);

        let raster_window_close = RasterWindow::new(&raster_chunk, (1, 1).into(), 9, 9);

        assert!(raster_window_close.is_some());

        let raster_window_over = RasterWindow::new(&raster_chunk, (1, 1).into(), 9, 10);

        assert!(raster_window_over.is_none());

        let raster_window_over_both = RasterWindow::new(&raster_chunk, (1, 1).into(), 11, 11);

        assert!(raster_window_over_both.is_none());
    }

    #[test]
    fn test_scale_up() {
        let mut raster_chunk = BoxRasterChunk::new(10, 10);
        raster_chunk.fill_rect(colors::red(), DrawPosition::from((0, 0)), 5, 5);

        raster_chunk.nn_scale(Dimensions {
            width: 20,
            height: 20,
        });

        let mut expected = BoxRasterChunk::new(20, 20);
        expected.fill_rect(colors::red(), DrawPosition::from((0, 0)), 10, 10);

        assert_raster_eq!(raster_chunk, expected);
    }

    #[test]
    fn test_scale_down() {
        let mut raster_chunk = BoxRasterChunk::new(20, 20);
        raster_chunk.fill_rect(colors::red(), DrawPosition::from((0, 0)), 10, 10);

        raster_chunk.nn_scale(Dimensions {
            width: 10,
            height: 10,
        });

        let mut expected = BoxRasterChunk::new(10, 10);
        expected.fill_rect(colors::red(), DrawPosition::from((0, 0)), 5, 5);

        assert_raster_eq!(raster_chunk, expected);
    }

    #[test]
    fn test_raster_chunk_shift() {
        let mut raster_a = BoxRasterChunk::new(10, 10);
        raster_a.fill_rect(colors::red(), DrawPosition((4, 2)), 2, 3);

        raster_a.horizontal_shift_left(2);

        let shifted_a = RasterWindow::new(&raster_a, PixelPosition((2, 2)), 2, 3)
            .unwrap()
            .to_chunk();

        let expected_a = BoxRasterChunk::new_fill(colors::red(), 2, 3);
        assert_raster_eq!(shifted_a, expected_a);

        let mut raster_b = BoxRasterChunk::new(10, 10);
        raster_b.fill_rect(colors::blue(), DrawPosition((3, 4)), 1, 4);

        raster_b.horizontal_shift_right(2);

        let shifted_b = RasterWindow::new(&raster_b, PixelPosition((5, 4)), 1, 4)
            .unwrap()
            .to_chunk();

        let expected_b = BoxRasterChunk::new_fill(colors::blue(), 1, 4);
        assert_raster_eq!(shifted_b, expected_b);

        let mut raster_c = BoxRasterChunk::new(10, 10);
        raster_c.fill_rect(colors::green(), DrawPosition((1, 2)), 2, 4);

        raster_c.vertical_shift_down(3);

        let shifted_c = RasterWindow::new(&raster_c, PixelPosition((1, 5)), 2, 4)
            .unwrap()
            .to_chunk();

        let expected_c = BoxRasterChunk::new_fill(colors::green(), 2, 4);
        assert_raster_eq!(shifted_c, expected_c);

        let mut raster_d = BoxRasterChunk::new(10, 10);
        raster_d.fill_rect(colors::white(), DrawPosition((6, 8)), 3, 1);

        raster_d.vertical_shift_up(3);

        let shifted_d = RasterWindow::new(&raster_d, PixelPosition((6, 5)), 3, 1)
            .unwrap()
            .to_chunk();

        let expected_d = BoxRasterChunk::new_fill(colors::white(), 3, 1);
        assert_raster_eq!(shifted_d, expected_d);
    }
}
