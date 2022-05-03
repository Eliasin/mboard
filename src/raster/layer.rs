use super::{
    chunks::{RasterChunk, RasterWindow},
    pixels::Pixel,
    position::{DrawPosition, PixelPosition},
};
use crate::{
    canvas::{CanvasRect, CanvasView, Layer},
    raster::shapes::{Oval, RasterPolygon},
};
use std::{
    collections::{hash_map::Entry, HashMap},
    convert::TryInto,
    ops::Rem,
};

/// A layer made of raw pixel data. All layers will eventually
/// be composited onto a raster layer for presentation.
pub struct RasterLayer {
    chunk_size: usize,
    chunks: HashMap<(i64, i64), RasterChunk>,
}

impl RasterLayer {
    pub fn new(chunk_size: usize) -> RasterLayer {
        RasterLayer {
            chunk_size,
            chunks: HashMap::new(),
        }
    }
}

/// An editing action that can be applied to a raster canvas.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RasterLayerAction {
    /// Fills a rect with `pixel`.
    FillRect(CanvasRect, Pixel),
    /// Draws an oval bounded by a canvas rect, filled with `pixel`.
    FillOval(CanvasRect, Pixel),
}

impl RasterLayerAction {
    pub fn fill_rect(canvas_rect: CanvasRect, pixel: Pixel) -> RasterLayerAction {
        RasterLayerAction::FillRect(canvas_rect, pixel)
    }

    pub fn fill_oval(canvas_rect: CanvasRect, pixel: Pixel) -> RasterLayerAction {
        RasterLayerAction::FillOval(canvas_rect, pixel)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct ChunkRectPosition {
    top_left_in_chunk: PixelPosition,
    width: usize,
    height: usize,
    x_chunk_offset: i64,
    y_chunk_offset: i64,
    x_pixel_offset: usize,
    y_pixel_offset: usize,
}

/// A rectangle in chunk-space, also denotes where it starts
/// and ends in the top-left and bottom-right chunks.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct ChunkRect {
    top_left_chunk: (i64, i64),
    chunk_width: u64,
    chunk_height: u64,
    top_left_in_chunk: PixelPosition,
    bottom_right_in_chunk: PixelPosition,
}

impl RasterLayer {
    fn find_chunk_rect_in_canvas_rect(&self, canvas_rect: CanvasRect) -> ChunkRect {
        let CanvasRect {
            top_left,
            width,
            height,
        } = canvas_rect;

        let width: i64 = width.try_into().unwrap();
        let height: i64 = height.try_into().unwrap();

        let chunk_size: i64 = self.chunk_size.try_into().unwrap();

        let top_left_chunk = (
            top_left.0.div_floor(chunk_size),
            top_left.1.div_floor(chunk_size),
        );

        let top_left_in_chunk: (i64, i64) = (
            top_left.0 - top_left_chunk.0 * chunk_size,
            top_left.1 - top_left_chunk.1 * chunk_size,
        );

        let bottom_right_in_chunk: (i64, i64) = (
            (top_left_in_chunk.0 + width - 1).rem(chunk_size),
            (top_left_in_chunk.1 + height - 1).rem(chunk_size),
        );

        let chunk_width: u64 = (width + top_left_in_chunk.0)
            .div_ceil(self.chunk_size.try_into().unwrap())
            .try_into()
            .unwrap();
        let chunk_height: u64 = (height + top_left_in_chunk.1)
            .div_ceil(self.chunk_size.try_into().unwrap())
            .try_into()
            .unwrap();

        let top_left_in_chunk: PixelPosition = PixelPosition::from((
            top_left_in_chunk.0.try_into().unwrap(),
            top_left_in_chunk.1.try_into().unwrap(),
        ));

        let bottom_right_in_chunk: PixelPosition = PixelPosition::from((
            bottom_right_in_chunk.0.try_into().unwrap(),
            bottom_right_in_chunk.1.try_into().unwrap(),
        ));

        ChunkRect {
            top_left_chunk,
            chunk_height,
            chunk_width,
            top_left_in_chunk,
            bottom_right_in_chunk,
        }
    }

    fn find_chunk_rect_in_view(&self, view: &CanvasView) -> ChunkRect {
        let (view_width, view_height) = view.canvas_dimensions();
        let view_anchor = view.anchor();

        let canvas_rect = CanvasRect {
            top_left: view_anchor,
            width: view_width as u32,
            height: view_height as u32,
        };

        self.find_chunk_rect_in_canvas_rect(canvas_rect)
    }

    fn for_chunk_rect<F>(&mut self, r: &mut F, chunk_rect: ChunkRect)
    where
        F: FnMut(&mut RasterChunk, ChunkRectPosition),
    {
        for y_offset in 0..chunk_rect.chunk_height {
            for x_offset in 0..chunk_rect.chunk_width {
                let width = if chunk_rect.chunk_width == 1 {
                    chunk_rect.bottom_right_in_chunk.0 .0 - chunk_rect.top_left_in_chunk.0 .0 + 1
                } else if x_offset == 0 {
                    self.chunk_size - chunk_rect.top_left_in_chunk.0 .0
                } else if x_offset == chunk_rect.chunk_width - 1 {
                    chunk_rect.bottom_right_in_chunk.0 .0 + 1
                } else {
                    self.chunk_size
                };

                let height = if chunk_rect.chunk_height == 1 {
                    chunk_rect.bottom_right_in_chunk.0 .1 - chunk_rect.top_left_in_chunk.0 .1 + 1
                } else if y_offset == 0 {
                    self.chunk_size - chunk_rect.top_left_in_chunk.0 .1
                } else if y_offset == chunk_rect.chunk_height - 1 {
                    chunk_rect.bottom_right_in_chunk.0 .1 + 1
                } else {
                    self.chunk_size
                };

                let x_offset_usize: usize = x_offset.try_into().unwrap();
                let y_offset_usize: usize = y_offset.try_into().unwrap();

                let x_pixel_offset: usize = if x_offset == 0 {
                    0
                } else {
                    self.chunk_size - chunk_rect.top_left_in_chunk.0 .0
                        + (self.chunk_size * (x_offset_usize - 1))
                };

                let y_pixel_offset: usize = if y_offset == 0 {
                    0
                } else {
                    self.chunk_size - chunk_rect.top_left_in_chunk.0 .1
                        + (self.chunk_size * (y_offset_usize - 1))
                };

                let x_offset: i64 = x_offset.try_into().unwrap();
                let y_offset: i64 = y_offset.try_into().unwrap();

                let chunk_position = (
                    chunk_rect.top_left_chunk.0 + x_offset,
                    chunk_rect.top_left_chunk.1 + y_offset,
                );

                let left_in_chunk = if x_offset == 0 {
                    chunk_rect.top_left_in_chunk.0 .0
                } else {
                    0
                };

                let top_in_chunk = if y_offset == 0 {
                    chunk_rect.top_left_in_chunk.0 .1
                } else {
                    0
                };

                let raster_chunk = match self.chunks.entry(chunk_position) {
                    Entry::Occupied(o) => o.into_mut(),
                    Entry::Vacant(v) => {
                        v.insert(RasterChunk::new(self.chunk_size, self.chunk_size))
                    }
                };

                let top_left_in_chunk = PixelPosition::from((left_in_chunk, top_in_chunk));

                let chunk_rect_position = ChunkRectPosition {
                    top_left_in_chunk,
                    width,
                    height,
                    x_chunk_offset: x_offset,
                    y_chunk_offset: y_offset,
                    x_pixel_offset,
                    y_pixel_offset,
                };

                r(raster_chunk, chunk_rect_position);
            }
        }
    }

    /// Draws a `RasterWindow` onto the layer with the top left at the position provided.
    fn blit(&mut self, top_left: (i64, i64), source: &RasterWindow) -> CanvasRect {
        let canvas_rect = CanvasRect {
            top_left,
            width: source.width().try_into().unwrap(),
            height: source.height().try_into().unwrap(),
        };

        let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);
        let chunk_size: i64 = self.chunk_size.try_into().unwrap();

        let mut writer = |raster_chunk: &mut RasterChunk,
                          chunk_rect_position: ChunkRectPosition| {
            let ChunkRectPosition {
                top_left_in_chunk,
                width: _,
                height: _,
                x_chunk_offset,
                y_chunk_offset,
                x_pixel_offset: _,
                y_pixel_offset: _,
            } = chunk_rect_position;

            let pixel_offset: (i64, i64) =
                (x_chunk_offset * chunk_size, y_chunk_offset * chunk_size);

            let top_left_in_chunk: (i64, i64) = (
                top_left_in_chunk.0 .0.try_into().unwrap(),
                top_left_in_chunk.0 .1.try_into().unwrap(),
            );
            let top_left_in_chunk: (i64, i64) = (
                top_left_in_chunk.0 - pixel_offset.0,
                top_left_in_chunk.0 - pixel_offset.1,
            );

            raster_chunk.blit(source, top_left_in_chunk.into());
        };

        self.for_chunk_rect(&mut writer, chunk_rect);

        canvas_rect
    }

    /// Performs a raster canvas action, returning the canvas rect that
    /// has been altered by it.
    pub fn perform_action(&mut self, action: RasterLayerAction) -> Option<CanvasRect> {
        use RasterLayerAction::*;
        match action {
            FillRect(canvas_rect, pixel) => {
                let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);

                let mut writer =
                    |raster_chunk: &mut RasterChunk, chunk_rect_position: ChunkRectPosition| {
                        let ChunkRectPosition {
                            top_left_in_chunk,
                            width,
                            height,
                            x_chunk_offset: _,
                            y_chunk_offset: _,
                            x_pixel_offset: _,
                            y_pixel_offset: _,
                        } = chunk_rect_position;

                        let draw_chunk = RasterChunk::new_fill(pixel, width, height);

                        raster_chunk.blit(&draw_chunk.as_window(), top_left_in_chunk.into());
                    };

                self.for_chunk_rect(&mut writer, chunk_rect);

                Some(canvas_rect)
            }
            FillOval(rect, pixel) => {
                let oval = Oval::build_from_bound(rect.width, rect.height)
                    .color(pixel)
                    .build();

                Some(self.blit(rect.top_left, &oval.rasterize().as_window()))
            }
        }
    }
}

impl Layer for RasterLayer {
    fn rasterize(&mut self, view: &CanvasView) -> RasterChunk {
        let chunk_rect = self.find_chunk_rect_in_view(view);

        let (view_width, view_height) = view.canvas_dimensions();
        let mut raster_result = RasterChunk::new(view_width, view_height);

        let mut rasterizer = |raster_chunk: &mut RasterChunk,
                              chunk_rect_position: ChunkRectPosition| {
            let ChunkRectPosition {
                top_left_in_chunk,
                width,
                height,
                x_chunk_offset: _,
                y_chunk_offset: _,
                x_pixel_offset,
                y_pixel_offset,
            } = chunk_rect_position;

            let raster_window =
                RasterWindow::new(raster_chunk, top_left_in_chunk, width, height).unwrap();

            let x_pixel_offset: i64 = x_pixel_offset.try_into().unwrap();
            let y_pixel_offset: i64 = y_pixel_offset.try_into().unwrap();
            let draw_position_in_result: DrawPosition =
                DrawPosition::from((x_pixel_offset, y_pixel_offset));

            raster_result.blit(&raster_window, draw_position_in_result);
        };

        self.for_chunk_rect(&mut rasterizer, chunk_rect);

        raster_result.nn_scale(view.view_dimensions());

        raster_result
    }
}

mod tests {
    #[cfg(test)]
    use crate::assert_raster_eq;
    #[cfg(test)]
    use crate::raster::pixels::colors;

    #[cfg(test)]
    use super::*;

    #[test]
    fn test_chunk_visibility_easy() {
        let raster_layer = RasterLayer::new(10);

        let mut view = CanvasView::new(10, 10);

        assert_eq!(
            raster_layer.find_chunk_rect_in_view(&view),
            ChunkRect {
                top_left_chunk: (0, 0),
                chunk_width: 1,
                chunk_height: 1,
                top_left_in_chunk: (0, 0).into(),
                bottom_right_in_chunk: (9, 9).into(),
            }
        );

        view.translate((-5, -2));

        assert_eq!(
            raster_layer.find_chunk_rect_in_view(&view),
            ChunkRect {
                top_left_chunk: (-1, -1),
                chunk_width: 2,
                chunk_height: 2,
                top_left_in_chunk: (10 - 5, 10 - 2).into(),
                bottom_right_in_chunk: (9 - 5, 9 - 2).into(),
            }
        );
    }

    #[test]
    fn test_chunk_visibility_medium() {
        let raster_layer = RasterLayer::new(1024);

        let mut view = CanvasView::new(2000, 2000);
        view.translate((-500, -500));

        assert_eq!(
            raster_layer.find_chunk_rect_in_view(&view),
            ChunkRect {
                top_left_chunk: (-1, -1),
                chunk_width: 3,
                chunk_height: 3,
                top_left_in_chunk: (524, 524).into(),
                bottom_right_in_chunk: (500 - 24 - 1, 500 - 24 - 1).into(),
            }
        );
    }

    #[test]
    fn test_chunk_visibility_hard() {
        let raster_layer = RasterLayer::new(512);

        let mut view = CanvasView::new(2000, 1000);
        view.translate((-500, -1000));

        assert_eq!(
            raster_layer.find_chunk_rect_in_view(&view),
            ChunkRect {
                top_left_chunk: (-1, -2),
                chunk_width: 4,
                chunk_height: 2,
                top_left_in_chunk: (12, 24).into(),
                bottom_right_in_chunk: (512 - 36 - 1, 512 - 1).into(),
            }
        );
    }

    #[test]
    fn test_rasterize_offset() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = RasterChunk::new_fill(colors::red(), 10, 10);
        raster_layer.chunks.insert((0, 0), red_chunk.clone());

        let mut view = CanvasView::new(10, 10);

        view.translate((-5, 0));

        let mut expected_result = RasterChunk::new(10, 10);
        expected_result.fill_rect(colors::red(), DrawPosition::from((5, 0)), 5, 10);

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_rasterization_easy() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = RasterChunk::new_fill(colors::red(), 10, 10);

        raster_layer.chunks.insert((0, 0), red_chunk.clone());

        let view = CanvasView::new(11, 11);

        let mut expected_result = RasterChunk::new(11, 11);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((0, 0)));

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_rasterization_medium() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = RasterChunk::new_fill(colors::red(), 10, 10);
        let green_chunk = RasterChunk::new_fill(colors::green(), 10, 10);

        raster_layer.chunks.insert((0, 0), red_chunk.clone());
        raster_layer.chunks.insert((1, 0), green_chunk.clone());

        let view = CanvasView::new(15, 10);

        let mut expected_result = RasterChunk::new(15, 10);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((0, 0)));
        expected_result.blit(&green_chunk.as_window(), DrawPosition::from((10, 0)));

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_rasterization_hard() {
        let mut raster_layer = RasterLayer::new(100);

        let red_chunk = RasterChunk::new_fill(colors::red(), 100, 100);
        let green_chunk = RasterChunk::new_fill(colors::green(), 100, 100);

        raster_layer.chunks.insert((0, 0), red_chunk.clone());
        raster_layer.chunks.insert((-1, -1), green_chunk.clone());

        let mut view = CanvasView::new(150, 200);
        view.translate((-275, -115));

        let mut expected_result = RasterChunk::new(150, 200);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((250, 100)));
        expected_result.blit(
            &green_chunk.as_window(),
            DrawPosition::from((100 - 275, 100 - 115)),
        );

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_fill_rect_easy() {
        let mut raster_layer = RasterLayer::new(10);

        let rect = CanvasRect {
            top_left: (0, 0).into(),
            width: 10,
            height: 10,
        };
        let red_fill = RasterLayerAction::fill_rect(rect, colors::red());

        raster_layer.perform_action(red_fill);

        let view = CanvasView::new(10, 10);
        let raster = raster_layer.rasterize(&view);

        let expected = RasterChunk::new_fill(colors::red(), 10, 10);

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_rect_medium() {
        let mut raster_layer = RasterLayer::new(10);

        let rect = CanvasRect {
            top_left: (0, 0).into(),
            width: 5,
            height: 5,
        };
        let red_fill = RasterLayerAction::fill_rect(rect, colors::red());

        raster_layer.perform_action(red_fill);

        let view = CanvasView::new(10, 10);
        let raster = raster_layer.rasterize(&view);

        let mut expected = RasterChunk::new(10, 10);

        let red_chunk = RasterChunk::new_fill(colors::red(), 5, 5);

        expected.blit(&red_chunk.as_window(), (0, 0).into());

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_rect_action_hard() {
        let mut raster_layer = RasterLayer::new(10);

        let left_rect = CanvasRect {
            top_left: (0, 0).into(),
            width: 5,
            height: 5,
        };
        let right_rect = CanvasRect {
            top_left: (6, 0).into(),
            width: 5,
            height: 5,
        };
        let red_fill = RasterLayerAction::fill_rect(left_rect, colors::red());
        let blue_fill = RasterLayerAction::fill_rect(right_rect, colors::blue());

        raster_layer.perform_action(red_fill);
        raster_layer.perform_action(blue_fill);

        let view = CanvasView::new(15, 10);
        let raster = raster_layer.rasterize(&view);

        let mut expected = RasterChunk::new(15, 10);

        let red_chunk = RasterChunk::new_fill(colors::red(), 5, 5);
        let blue_chunk = RasterChunk::new_fill(colors::blue(), 5, 5);

        expected.blit(&red_chunk.as_window(), (0, 0).into());
        expected.blit(&blue_chunk.as_window(), (6, 0).into());

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_scaled_rasterization() {
        let mut raster_layer = RasterLayer::new(20);
        let left_rect = CanvasRect {
            top_left: (9, 9).into(),
            width: 2,
            height: 2,
        };
        let red_fill = RasterLayerAction::fill_rect(left_rect, colors::red());
        raster_layer.perform_action(red_fill);

        let mut view = CanvasView::new(20, 20);
        view.pin_resize_canvas((10, 10));

        let raster = raster_layer.rasterize(&view);

        let mut expected = RasterChunk::new(10, 10);
        expected.fill_rect(colors::red(), (4, 4).into(), 2, 2);

        expected.nn_scale((20, 20));

        assert_raster_eq!(raster, expected);
    }
}
