use super::{
    chunks::{raster_chunk::BumpRasterChunk, BoxRasterChunk, RasterWindow},
    iter::{RasterChunkIterator, RasterChunkIteratorMut},
    pixels::{colors, Pixel},
    position::{Dimensions, DrawPosition, PixelPosition},
};
use crate::{
    canvas::{CanvasPosition, CanvasRect, CanvasView, Layer, ShapeCache},
    raster::shapes::{Oval, RasterPolygon},
};
use std::{collections::HashMap, convert::TryInto};

/// A layer made of raw pixel data. All layers will eventually
/// be composited onto a raster layer for presentation.
pub struct RasterLayer {
    pub(super) chunk_size: usize,
    pub(super) chunks: HashMap<ChunkPosition, BoxRasterChunk>,
    blank_chunk: BoxRasterChunk,
}

impl RasterLayer {
    pub fn new(chunk_size: usize) -> RasterLayer {
        RasterLayer {
            chunk_size,
            chunks: HashMap::new(),
            blank_chunk: BoxRasterChunk::new_fill(colors::transparent(), chunk_size, chunk_size),
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
pub struct ChunkRectPosition {
    pub top_left_in_chunk: PixelPosition,
    pub width: usize,
    pub height: usize,
    pub x_chunk_offset: usize,
    pub y_chunk_offset: usize,
    pub x_pixel_offset: usize,
    pub y_pixel_offset: usize,
}

/// A poisiton of a chunk within the layer.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ChunkPosition(pub (i64, i64));

impl ChunkPosition {
    /// Get the dimension of chunks spanned between this position and another chunk position.
    pub fn span(&self, other: ChunkPosition) -> Dimensions {
        Dimensions {
            width: self.0 .0.abs_diff(other.0 .0) as usize + 1,
            height: self.0 .1.abs_diff(other.0 .1) as usize + 1,
        }
    }

    pub fn translate(&self, v: (i64, i64)) -> ChunkPosition {
        ChunkPosition((self.0 .0 + v.0, self.0 .1 + v.1))
    }
}

/// A rectangle in chunk-space, also denotes where it starts
/// and ends in the top-left and bottom-right chunks.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ChunkRect {
    pub top_left_chunk: ChunkPosition,
    pub chunk_dimensions: Dimensions,
    pub top_left_in_chunk: PixelPosition,
    pub bottom_right_in_chunk: PixelPosition,
}

impl ChunkRect {
    /// Get the position most top-left within a chunk that is within the chunk rect.
    /// Returns `None` if the requested position is not within this chunk-rect.
    pub fn top_left_in_chunk(&self, chunk_position: ChunkPosition) -> Option<PixelPosition> {
        let bottom_right_chunk = self.top_left_chunk.translate((
            self.chunk_dimensions.width as i64,
            self.chunk_dimensions.height as i64,
        ));

        let position_past_top_left = chunk_position.0 .0 < self.top_left_chunk.0 .0
            || chunk_position.0 .1 < self.top_left_chunk.0 .1;
        let position_past_bottom_right = chunk_position.0 .0 > bottom_right_chunk.0 .0
            || chunk_position.0 .1 > bottom_right_chunk.0 .1;
        if position_past_top_left || position_past_bottom_right {
            None
        } else {
            let left_in_chunk = if chunk_position.0 .0 == self.top_left_chunk.0 .0 {
                self.top_left_in_chunk.0 .0
            } else {
                0
            };

            let top_in_chunk = if chunk_position.0 .1 == self.top_left_chunk.0 .1 {
                self.top_left_in_chunk.0 .1
            } else {
                0
            };

            Some(PixelPosition((left_in_chunk, top_in_chunk)))
        }
    }
}

impl RasterLayer {
    fn find_chunk_rect_in_canvas_rect(&self, canvas_rect: CanvasRect) -> ChunkRect {
        let CanvasRect {
            top_left,
            dimensions,
        } = canvas_rect;

        let top_left_chunk = top_left.containing_chunk(self.chunk_size);
        let top_left_in_chunk = top_left.position_in_containing_chunk(self.chunk_size);

        let bottom_right =
            top_left.translate((dimensions.width as i64 - 1, dimensions.height as i64 - 1));
        let bottom_right_chunk = bottom_right.containing_chunk(self.chunk_size);
        let bottom_right_in_chunk = bottom_right.position_in_containing_chunk(self.chunk_size);

        let chunk_span = top_left_chunk.span(bottom_right_chunk);

        ChunkRect {
            top_left_chunk,
            chunk_dimensions: chunk_span,
            top_left_in_chunk,
            bottom_right_in_chunk,
        }
    }

    fn iter_chunks_in_rect(&self, chunk_rect: ChunkRect) -> RasterChunkIterator {
        RasterChunkIterator::new(self, chunk_rect)
    }

    fn iter_mut_chunks_in_rect(&mut self, chunk_rect: ChunkRect) -> RasterChunkIteratorMut {
        RasterChunkIteratorMut::new(self, chunk_rect)
    }

    /// Composites a `RasterWindow` onto the layer with the top left at the position provided.
    fn composite_over(&mut self, top_left: CanvasPosition, source: &RasterWindow) -> CanvasRect {
        let canvas_rect = CanvasRect {
            top_left,
            dimensions: source.dimensions(),
        };

        let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);
        let mut raster_chunks_need_insert = HashMap::new();
        let chunk_size = self.chunk_size;

        for (raster_chunk, chunk_rect_position) in self.iter_mut_chunks_in_rect(chunk_rect) {
            let ChunkRectPosition {
                top_left_in_chunk,
                width: _,
                height: _,
                x_chunk_offset,
                y_chunk_offset,
                x_pixel_offset,
                y_pixel_offset,
            } = chunk_rect_position;

            let pixel_offset: (i64, i64) = (
                x_pixel_offset.try_into().unwrap(),
                y_pixel_offset.try_into().unwrap(),
            );

            let top_left_in_chunk: (i64, i64) = (
                top_left_in_chunk.0 .0.try_into().unwrap(),
                top_left_in_chunk.0 .1.try_into().unwrap(),
            );

            let top_left_in_chunk: (i64, i64) = (
                top_left_in_chunk.0 - pixel_offset.0,
                top_left_in_chunk.1 - pixel_offset.1,
            );

            if let Some(raster_chunk) = raster_chunk {
                raster_chunk.composite_over(source, top_left_in_chunk.into());
            } else {
                let mut raster_chunk = BoxRasterChunk::new(chunk_size, chunk_size);
                let chunk_position = chunk_rect
                    .top_left_chunk
                    .translate((x_chunk_offset as i64, y_chunk_offset as i64));
                raster_chunk.composite_over(source, top_left_in_chunk.into());
                raster_chunks_need_insert.insert(chunk_position, raster_chunk);
            }
        }

        for (chunk_position, raster_chunk) in raster_chunks_need_insert {
            self.chunks.insert(chunk_position, raster_chunk);
        }

        canvas_rect
    }

    /// Performs a raster canvas action, returning the canvas rect that
    /// has been altered by it.
    pub fn perform_action_with_cache(
        &mut self,
        action: RasterLayerAction,
        shape_cache: &mut ShapeCache,
    ) -> Option<CanvasRect> {
        use RasterLayerAction::*;
        match action {
            FillRect(canvas_rect, pixel) => {
                let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);
                let chunk_size = self.chunk_size;
                let mut raster_chunks_need_insert = HashMap::new();

                for (raster_chunk, chunk_rect_position) in self.iter_mut_chunks_in_rect(chunk_rect)
                {
                    let ChunkRectPosition {
                        top_left_in_chunk,
                        width,
                        height,
                        x_chunk_offset,
                        y_chunk_offset,
                        x_pixel_offset: _,
                        y_pixel_offset: _,
                    } = chunk_rect_position;

                    let draw_chunk = BoxRasterChunk::new_fill(pixel, width, height);
                    if let Some(raster_chunk) = raster_chunk {
                        raster_chunk
                            .composite_over(&draw_chunk.as_window(), top_left_in_chunk.into());
                    } else {
                        let mut raster_chunk = BoxRasterChunk::new(chunk_size, chunk_size);
                        let chunk_position = chunk_rect
                            .top_left_chunk
                            .translate((x_chunk_offset as i64, y_chunk_offset as i64));
                        raster_chunk
                            .composite_over(&draw_chunk.as_window(), top_left_in_chunk.into());
                        raster_chunks_need_insert.insert(chunk_position, raster_chunk);
                    }
                }

                for (chunk_position, raster_chunk) in raster_chunks_need_insert {
                    self.chunks.insert(chunk_position, raster_chunk);
                }

                Some(canvas_rect)
            }
            FillOval(rect, pixel) => {
                let oval = Oval::build_from_bound(
                    rect.dimensions.width as u32,
                    rect.dimensions.height as u32,
                )
                .color(pixel)
                .build();

                let oval_raster = shape_cache.get_oval(oval);

                let canvas_rect = self.composite_over(rect.top_left, &oval_raster.as_window());

                Some(canvas_rect)
            }
        }
    }

    /// Performs a raster canvas action, returning the canvas rect that
    /// has been altered by it.
    pub fn perform_action(&mut self, action: RasterLayerAction) -> Option<CanvasRect> {
        use RasterLayerAction::*;
        match action {
            FillRect(canvas_rect, pixel) => {
                let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);
                let mut raster_chunks_need_insert = HashMap::new();
                let chunk_size = self.chunk_size;

                for (raster_chunk, chunk_rect_position) in self.iter_mut_chunks_in_rect(chunk_rect)
                {
                    let ChunkRectPosition {
                        top_left_in_chunk,
                        width,
                        height,
                        x_chunk_offset,
                        y_chunk_offset,
                        x_pixel_offset: _,
                        y_pixel_offset: _,
                    } = chunk_rect_position;

                    let draw_chunk = BoxRasterChunk::new_fill(pixel, width, height);

                    if let Some(raster_chunk) = raster_chunk {
                        raster_chunk
                            .composite_over(&draw_chunk.as_window(), top_left_in_chunk.into());
                    } else {
                        let mut raster_chunk = BoxRasterChunk::new(chunk_size, chunk_size);
                        let chunk_position = chunk_rect
                            .top_left_chunk
                            .translate((x_chunk_offset as i64, y_chunk_offset as i64));
                        raster_chunk
                            .composite_over(&draw_chunk.as_window(), top_left_in_chunk.into());
                        raster_chunks_need_insert.insert(chunk_position, raster_chunk);
                    }
                }

                for (chunk_position, raster_chunk) in raster_chunks_need_insert {
                    self.chunks.insert(chunk_position, raster_chunk);
                }

                Some(canvas_rect)
            }
            FillOval(rect, pixel) => {
                let oval = Oval::build_from_bound(
                    rect.dimensions.width as u32,
                    rect.dimensions.height as u32,
                )
                .color(pixel)
                .build();

                let canvas_rect = self.composite_over(rect.top_left, &oval.rasterize().as_window());

                Some(canvas_rect)
            }
        }
    }
}

impl Layer for RasterLayer {
    fn rasterize(&mut self, view: &CanvasView) -> BoxRasterChunk {
        let mut raster = self.rasterize_canvas_rect(CanvasRect {
            top_left: view.top_left,
            dimensions: view.canvas_dimensions,
        });

        raster.nn_scale(view.view_dimensions);

        raster
    }

    fn rasterize_canvas_rect(&mut self, canvas_rect: CanvasRect) -> BoxRasterChunk {
        let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);

        let Dimensions {
            width: view_width,
            height: view_height,
        } = canvas_rect.dimensions;
        let mut raster_result = BoxRasterChunk::new(view_width, view_height);

        for (raster_chunk, chunk_rect_position) in self.iter_chunks_in_rect(chunk_rect) {
            let ChunkRectPosition {
                top_left_in_chunk,
                width,
                height,
                x_chunk_offset: _,
                y_chunk_offset: _,
                x_pixel_offset,
                y_pixel_offset,
            } = chunk_rect_position;

            let raster_chunk = raster_chunk.unwrap_or(&self.blank_chunk);

            let raster_window =
                RasterWindow::new(raster_chunk, top_left_in_chunk, width, height).unwrap();

            let x_pixel_offset: i64 = x_pixel_offset.try_into().unwrap();
            let y_pixel_offset: i64 = y_pixel_offset.try_into().unwrap();
            let draw_position_in_result: DrawPosition =
                DrawPosition::from((x_pixel_offset, y_pixel_offset));

            raster_result.blit(&raster_window, draw_position_in_result);
        }

        raster_result
    }

    fn clear(&mut self) {
        self.chunks.clear();
    }

    fn rasterize_into_bump<'bump>(
        &mut self,
        view: &CanvasView,
        bump: &'bump bumpalo::Bump,
    ) -> BumpRasterChunk<'bump> {
        let mut raster = self.rasterize_canvas_rect(CanvasRect {
            top_left: view.top_left,
            dimensions: view.canvas_dimensions,
        });

        raster.nn_scale_into_bump(view.view_dimensions, bump)
    }

    fn rasterize_canvas_rect_into_bump<'bump>(
        &mut self,
        canvas_rect: CanvasRect,
        bump: &'bump bumpalo::Bump,
    ) -> BumpRasterChunk<'bump> {
        let chunk_rect = self.find_chunk_rect_in_canvas_rect(canvas_rect);

        let Dimensions {
            width: view_width,
            height: view_height,
        } = canvas_rect.dimensions;
        let mut raster_result = BumpRasterChunk::new(view_width, view_height, bump);

        for (raster_chunk, chunk_rect_position) in self.iter_chunks_in_rect(chunk_rect) {
            let ChunkRectPosition {
                top_left_in_chunk,
                width,
                height,
                x_chunk_offset: _,
                y_chunk_offset: _,
                x_pixel_offset,
                y_pixel_offset,
            } = chunk_rect_position;

            let raster_chunk = raster_chunk.unwrap_or(&self.blank_chunk);

            let raster_window =
                RasterWindow::new(raster_chunk, top_left_in_chunk, width, height).unwrap();

            let x_pixel_offset: i64 = x_pixel_offset.try_into().unwrap();
            let y_pixel_offset: i64 = y_pixel_offset.try_into().unwrap();
            let draw_position_in_result: DrawPosition =
                DrawPosition::from((x_pixel_offset, y_pixel_offset));

            raster_result.blit(&raster_window, draw_position_in_result);
        }

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

        let mut canvas_rect = CanvasRect::at_origin(10, 10);

        assert_eq!(
            raster_layer.find_chunk_rect_in_canvas_rect(canvas_rect),
            ChunkRect {
                top_left_chunk: ChunkPosition((0, 0)),
                chunk_dimensions: Dimensions {
                    width: 1,
                    height: 1
                },
                top_left_in_chunk: (0, 0).into(),
                bottom_right_in_chunk: (9, 9).into(),
            }
        );

        canvas_rect.top_left = CanvasPosition((-5, -2));

        assert_eq!(
            raster_layer.find_chunk_rect_in_canvas_rect(canvas_rect),
            ChunkRect {
                top_left_chunk: ChunkPosition((-1, -1)),
                chunk_dimensions: Dimensions {
                    width: 2,
                    height: 2
                },
                top_left_in_chunk: (10 - 5, 10 - 2).into(),
                bottom_right_in_chunk: (9 - 5, 9 - 2).into(),
            }
        );
    }

    #[test]
    fn test_chunk_visibility_medium() {
        let raster_layer = RasterLayer::new(1024);

        let mut canvas_rect = CanvasRect::at_origin(2000, 2000);
        canvas_rect.top_left = CanvasPosition((-500, -500));

        assert_eq!(
            raster_layer.find_chunk_rect_in_canvas_rect(canvas_rect),
            ChunkRect {
                top_left_chunk: ChunkPosition((-1, -1)),
                chunk_dimensions: Dimensions {
                    width: 3,
                    height: 3
                },
                top_left_in_chunk: (524, 524).into(),
                bottom_right_in_chunk: (500 - 24 - 1, 500 - 24 - 1).into(),
            }
        );
    }

    #[test]
    fn test_chunk_visibility_hard() {
        let raster_layer = RasterLayer::new(512);

        let mut canvas_rect = CanvasRect::at_origin(2000, 1000);
        canvas_rect.top_left = CanvasPosition((-500, -1000));

        assert_eq!(
            raster_layer.find_chunk_rect_in_canvas_rect(canvas_rect),
            ChunkRect {
                top_left_chunk: ChunkPosition((-1, -2)),
                chunk_dimensions: Dimensions {
                    width: 4,
                    height: 2
                },
                top_left_in_chunk: (12, 24).into(),
                bottom_right_in_chunk: (512 - 36 - 1, 512 - 1).into(),
            }
        );
    }

    #[test]
    fn test_rasterize_offset() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = BoxRasterChunk::new_fill(colors::red(), 10, 10);
        raster_layer
            .chunks
            .insert(ChunkPosition((0, 0)), red_chunk.clone());

        let mut view = CanvasView::new(10, 10);

        view.translate((-5, 0));

        let mut expected_result = BoxRasterChunk::new(10, 10);
        expected_result.fill_rect(colors::red(), DrawPosition::from((5, 0)), 5, 10);

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_rasterization_easy() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = BoxRasterChunk::new_fill(colors::red(), 10, 10);

        raster_layer
            .chunks
            .insert(ChunkPosition((0, 0)), red_chunk.clone());

        let view = CanvasView::new(11, 11);

        let mut expected_result = BoxRasterChunk::new(11, 11);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((0, 0)));

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_rasterization_medium() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = BoxRasterChunk::new_fill(colors::red(), 10, 10);
        let green_chunk = BoxRasterChunk::new_fill(colors::green(), 10, 10);

        raster_layer
            .chunks
            .insert(ChunkPosition((0, 0)), red_chunk.clone());
        raster_layer
            .chunks
            .insert(ChunkPosition((1, 0)), green_chunk.clone());

        let view = CanvasView::new(15, 10);

        let mut expected_result = BoxRasterChunk::new(15, 10);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((0, 0)));
        expected_result.blit(&green_chunk.as_window(), DrawPosition::from((10, 0)));

        let raster = raster_layer.rasterize(&view);

        assert_raster_eq!(raster, expected_result);
    }

    #[test]
    fn test_rasterization_hard() {
        let mut raster_layer = RasterLayer::new(100);

        let red_chunk = BoxRasterChunk::new_fill(colors::red(), 100, 100);
        let green_chunk = BoxRasterChunk::new_fill(colors::green(), 100, 100);

        raster_layer
            .chunks
            .insert(ChunkPosition((0, 0)), red_chunk.clone());
        raster_layer
            .chunks
            .insert(ChunkPosition((-1, -1)), green_chunk.clone());

        let mut view = CanvasView::new(150, 200);
        view.translate((-275, -115));

        let mut expected_result = BoxRasterChunk::new(150, 200);

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
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };
        let red_fill = RasterLayerAction::fill_rect(rect, colors::red());

        raster_layer.perform_action(red_fill);

        let view = CanvasView::new(10, 10);
        let raster = raster_layer.rasterize(&view);

        let expected = BoxRasterChunk::new_fill(colors::red(), 10, 10);

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_rect_medium() {
        let mut raster_layer = RasterLayer::new(10);

        let rect = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };
        let red_fill = RasterLayerAction::fill_rect(rect, colors::red());

        raster_layer.perform_action(red_fill);

        let view = CanvasView::new(10, 10);
        let raster = raster_layer.rasterize(&view);

        let mut expected = BoxRasterChunk::new(10, 10);

        let red_chunk = BoxRasterChunk::new_fill(colors::red(), 5, 5);

        expected.blit(&red_chunk.as_window(), (0, 0).into());

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_rect_action_hard() {
        let mut raster_layer = RasterLayer::new(10);

        let left_rect = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };
        let right_rect = CanvasRect {
            top_left: CanvasPosition((6, 0)),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };
        let red_fill = RasterLayerAction::fill_rect(left_rect, colors::red());
        let blue_fill = RasterLayerAction::fill_rect(right_rect, colors::blue());

        raster_layer.perform_action(red_fill);
        raster_layer.perform_action(blue_fill);

        let view = CanvasView::new(15, 10);
        let raster = raster_layer.rasterize(&view);

        let mut expected = BoxRasterChunk::new(15, 10);

        let red_chunk = BoxRasterChunk::new_fill(colors::red(), 5, 5);
        let blue_chunk = BoxRasterChunk::new_fill(colors::blue(), 5, 5);

        expected.blit(&red_chunk.as_window(), (0, 0).into());
        expected.blit(&blue_chunk.as_window(), (6, 0).into());

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_scaled_rasterization() {
        let mut raster_layer = RasterLayer::new(20);
        let left_rect = CanvasRect {
            top_left: CanvasPosition((9, 9)),
            dimensions: Dimensions {
                width: 2,
                height: 2,
            },
        };
        let red_fill = RasterLayerAction::fill_rect(left_rect, colors::red());
        raster_layer.perform_action(red_fill);

        let mut view = CanvasView::new(20, 20);
        view.pin_resize_canvas(Dimensions {
            width: 10,
            height: 10,
        });

        let raster = raster_layer.rasterize(&view);

        let mut expected = BoxRasterChunk::new(10, 10);
        expected.fill_rect(colors::red(), (4, 4).into(), 2, 2);

        expected.nn_scale(Dimensions {
            width: 20,
            height: 20,
        });

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_oval_easy() {
        let mut raster_layer = RasterLayer::new(30);
        let view = CanvasView::new(30, 30);

        let rect = CanvasRect {
            top_left: CanvasPosition((10, 10)),
            dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };

        let red_oval = RasterLayerAction::fill_oval(rect, colors::red());
        raster_layer.perform_action(red_oval);

        let raster = raster_layer.rasterize(&view);

        let mut expected = BoxRasterChunk::new(30, 30);
        let oval = Oval::build_from_bound(10, 10).color(colors::red()).build();
        expected.composite_over(&oval.rasterize().as_window(), DrawPosition::from((10, 10)));

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_oval_medium() {
        let mut raster_layer = RasterLayer::new(30);
        let view = CanvasView::new(30, 30);

        let rect = CanvasRect {
            top_left: CanvasPosition((10, 15)),
            dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };

        let red_oval = RasterLayerAction::fill_oval(rect, colors::red());
        raster_layer.perform_action(red_oval);

        let raster = raster_layer.rasterize(&view);

        let mut expected = BoxRasterChunk::new(30, 30);
        let oval = Oval::build_from_bound(10, 10).color(colors::red()).build();
        expected.composite_over(&oval.rasterize().as_window(), DrawPosition::from((10, 15)));

        assert_raster_eq!(raster, expected);
    }

    #[test]
    fn test_fill_oval_border() {
        let mut raster_layer = RasterLayer::new(30);
        let view = CanvasView::new(60, 60);

        let rect = CanvasRect {
            top_left: CanvasPosition((25, 10)),
            dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };

        let red_oval = RasterLayerAction::fill_oval(rect, colors::red());
        raster_layer.perform_action(red_oval);

        let raster = raster_layer.rasterize(&view);

        let mut expected = BoxRasterChunk::new(60, 60);
        let oval = Oval::build_from_bound(10, 10).color(colors::red()).build();
        expected.composite_over(&oval.rasterize().as_window(), DrawPosition::from((25, 10)));

        assert_raster_eq!(raster, expected);
    }
}
