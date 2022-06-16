use crate::raster::{
    chunks::{nn_map::NearestNeighbourMap, raster_chunk::BumpRasterChunk, BoxRasterChunk},
    layer::ChunkPosition,
    pixels::colors,
    position::{Dimensions, PixelPosition, Scale},
    RasterLayer, RasterLayerAction,
};
use bumpalo::Bump;
use enum_dispatch::enum_dispatch;

mod cache;
pub use cache::ShapeCache;

use self::cache::{CanvasRectRasterCache, CanvasViewRasterCache};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CanvasPosition(pub (i64, i64));

impl CanvasPosition {
    /// Get a point in the canvas from a view and an offset to the view.
    pub fn from_view_position(view: CanvasView, p: PixelPosition) -> CanvasPosition {
        CanvasPosition((
            view.top_left.0 .0 + p.0 .0 as i64,
            view.top_left.0 .1 + p.0 .1 as i64,
        ))
    }

    /// Translate a canvas position by an offset.
    pub fn translate(&self, offset: (i64, i64)) -> CanvasPosition {
        CanvasPosition((self.0 .0 + offset.0, self.0 .1 + offset.1))
    }

    /// Translate a canvas position by some portion of an offset.
    pub fn translate_scaled(&self, offset: (i64, i64), divisor: i64) -> CanvasPosition {
        self.translate((offset.0 / divisor, offset.1 / divisor))
    }

    /// The chunk containing a canvas position.
    pub fn containing_chunk(&self, chunk_size: usize) -> ChunkPosition {
        ChunkPosition((
            self.0 .0.div_floor(chunk_size as i64),
            self.0 .1.div_floor(chunk_size as i64),
        ))
    }

    /// Where the `CanvasPosition` relative to the containing chunk.
    pub fn position_in_containing_chunk(&self, chunk_size: usize) -> PixelPosition {
        let containing_chunk = self.containing_chunk(chunk_size);
        PixelPosition((
            (self.0 .0 - containing_chunk.0 .0 * chunk_size as i64) as usize,
            (self.0 .1 - containing_chunk.0 .1 * chunk_size as i64) as usize,
        ))
    }
}

/// A view positioned relative to a set of layers.
/// The view has a scale and a width and height, the width and height are in pixel units.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CanvasView {
    pub top_left: CanvasPosition,
    pub view_dimensions: Dimensions,
    pub canvas_dimensions: Dimensions,
}

impl CanvasView {
    /// Create a new view with a specified width and height. The default placement
    /// is at the origin with an effective scale of 1.
    pub fn new(width: usize, height: usize) -> CanvasView {
        CanvasView {
            top_left: CanvasPosition((0, 0)),
            view_dimensions: Dimensions { width, height },
            canvas_dimensions: Dimensions { width, height },
        }
    }

    /// Translate a view by an offset.
    pub fn translate(&mut self, d: (i64, i64)) {
        self.top_left = self.top_left.translate(d);
    }

    /// Change the canvas dimensions of the view while preserving the middle of the view.
    pub fn pin_resize_canvas(&mut self, d: Dimensions) {
        let difference = self.canvas_dimensions.difference(d);

        self.top_left = self.top_left.translate_scaled(difference, 2);
        self.canvas_dimensions = d;
    }

    /// Scale the canvas source of the view while preserving the middle of the view.
    /// Negative or factors that scale the view too small are ignored.
    pub fn pin_scale_canvas(&mut self, factor: Scale) {
        let new_dimensions = self.canvas_dimensions.scale(factor);

        if new_dimensions.width < 1 || new_dimensions.height < 1 {
            return;
        }

        self.pin_resize_canvas(new_dimensions);
    }

    /// Scale the canvas source and view dimensions of the view while preserving
    /// the middle of the view. Negatives or factors that scale the view too small are ignored.
    pub fn pin_scale(&mut self, factor: Scale) {
        let new_canvas_dimensions = self.canvas_dimensions.scale(factor);
        let new_view_dimensions = self.view_dimensions.scale(factor);

        if new_canvas_dimensions.width < 1
            || new_canvas_dimensions.height < 1
            || new_view_dimensions.width < 1
            || new_view_dimensions.height < 1
        {
            return;
        }

        let difference = self.canvas_dimensions.difference(new_canvas_dimensions);

        self.top_left = self.top_left.translate_scaled(difference, 2);
        self.canvas_dimensions = new_canvas_dimensions;
        self.view_dimensions = new_view_dimensions;
    }

    /// Transforms a point from view space to canvas space.
    pub fn transform_view_to_canvas(&self, p: PixelPosition) -> CanvasPosition {
        let scaled_point = self
            .canvas_dimensions
            .transform_point(p, self.view_dimensions);

        CanvasPosition::from_view_position(*self, scaled_point)
    }

    /// Attempt to transform a position in canvas space to a position
    /// in view space. Canvas positions not in view will map to `None`;
    pub fn transform_canvas_to_view(&self, p: CanvasPosition) -> Option<PixelPosition> {
        let translated_point = p.translate((-self.top_left.0 .0, -self.top_left.0 .1));

        let point_past_top_left = translated_point.0 .0 < 0 || translated_point.0 .1 < 0;
        let point_past_bottom_right = translated_point.0 .0 > self.canvas_dimensions.width as i64
            || translated_point.0 .1 > self.canvas_dimensions.height as i64;
        if point_past_top_left || point_past_bottom_right {
            None
        } else {
            Some(self.view_dimensions.transform_point(
                PixelPosition((
                    translated_point.0 .0 as usize,
                    translated_point.0 .1 as usize,
                )),
                self.canvas_dimensions,
            ))
        }
    }

    /// Attempt to transform a rect in canvas space to a rect
    /// in view space. Canvas rects not fully in view will map to `None`;
    pub fn transform_canvas_rect_to_view(&self, r: &CanvasRect) -> Option<ViewRect> {
        let top_left = self.transform_canvas_to_view(r.top_left)?;
        let bottom_right = self.transform_canvas_to_view(r.bottom_right())?;

        Some(ViewRect::new_points(top_left, bottom_right))
    }

    /// Transform a rect in view space to a rect in canvas space.
    pub fn transform_view_rect_to_canvas(&self, r: &ViewRect) -> CanvasRect {
        let top_left = self.transform_view_to_canvas(r.top_left);
        let bottom_right = self.transform_view_to_canvas(r.bottom_right());

        CanvasRect::new_points(top_left, bottom_right)
    }

    /// Create a `NearestNeighbourMap` for the transformation from the canvas
    /// dimensions to the view dimensions of this `CanvasView`.
    pub fn create_nn_map_to_view_dimensions(&self) -> NearestNeighbourMap {
        NearestNeighbourMap::new(self.canvas_dimensions, self.view_dimensions)
    }

    pub fn canvas_rect(&self) -> CanvasRect {
        CanvasRect {
            top_left: self.top_left,
            dimensions: self.canvas_dimensions,
        }
    }

    pub fn scale_eq(&self, other: &CanvasView) -> bool {
        let scale = self.canvas_dimensions.relative_scale(self.view_dimensions);

        let other_scale = other
            .canvas_dimensions
            .relative_scale(other.view_dimensions);

        scale.similar_to(other_scale)
    }
}

/// A rectangle within a view configuration.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ViewRect {
    pub top_left: PixelPosition,
    pub dimensions: Dimensions,
}

impl ViewRect {
    /// A view rect defined by two points.
    pub fn new_points(a: PixelPosition, b: PixelPosition) -> ViewRect {
        let top_left = (a.0 .0.min(b.0 .0), a.0 .1.min(b.0 .1));
        let bottom_right = (a.0 .0.max(b.0 .0), a.0 .1.max(b.0 .1));

        ViewRect {
            top_left: PixelPosition(top_left),
            dimensions: Dimensions {
                width: bottom_right.0 - top_left.0,
                height: bottom_right.1 - top_left.1,
            },
        }
    }

    /// The bottom right of a view rect.
    pub fn bottom_right(&self) -> PixelPosition {
        self.top_left
            .translate((self.dimensions.width, self.dimensions.height))
    }
}

/// A rectangle in canvas-space that can be used for operations
/// on layers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CanvasRect {
    pub top_left: CanvasPosition,
    pub dimensions: Dimensions,
}

impl CanvasRect {
    pub fn new_points(a: CanvasPosition, b: CanvasPosition) -> CanvasRect {
        let top_left = (a.0 .0.min(b.0 .0), a.0 .1.min(b.0 .1));
        let bottom_right = (a.0 .0.max(b.0 .0), a.0 .1.max(b.0 .1));

        CanvasRect {
            top_left: CanvasPosition(top_left),
            dimensions: Dimensions {
                width: (bottom_right.0 - top_left.0) as usize,
                height: (bottom_right.1 - top_left.1) as usize,
            },
        }
    }

    pub fn at_origin(width: usize, height: usize) -> CanvasRect {
        CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions { width, height },
        }
    }

    /// A rect that contains both this `CanvasRect` and `other`.
    pub fn spanning_rect(&self, other: &CanvasRect) -> CanvasRect {
        let top = self.top_left.0 .1.min(other.top_left.0 .1);
        let left = self.top_left.0 .0.min(other.top_left.0 .0);

        let bottom_right = self.bottom_right();
        let other_bottom_right = other.bottom_right();

        let bottom = bottom_right.0 .1.max(other_bottom_right.0 .1);
        let right = bottom_right.0 .0.max(other_bottom_right.0 .0);

        CanvasRect {
            top_left: CanvasPosition((left, top)),
            dimensions: Dimensions {
                width: (right - left) as usize,
                height: (bottom - top) as usize,
            },
        }
    }

    /// The bottom right of a canvas rect.
    pub fn bottom_right(&self) -> CanvasPosition {
        self.top_left
            .translate((self.dimensions.width as i64, self.dimensions.height as i64))
    }

    /// Whether or not this canvas rect fully contains another.
    pub fn contains(&self, other: &CanvasRect) -> bool {
        self.contains_with_offset(other).is_some()
    }

    /// The offset of a contained rect to this rect.
    pub fn contains_with_offset(&self, other: &CanvasRect) -> Option<PixelPosition> {
        if self.top_left.0 .0 > other.top_left.0 .0 || self.top_left.0 .1 > other.top_left.0 .1 {
            None
        } else {
            let bottom_right = self.bottom_right();
            let other_bottom_right = other.bottom_right();

            if bottom_right.0 .0 < other_bottom_right.0 .0
                || bottom_right.0 .1 < other_bottom_right.0 .1
            {
                None
            } else {
                Some(PixelPosition::from((
                    other.top_left.0 .0.abs_diff(self.top_left.0 .0) as usize,
                    other.top_left.0 .1.abs_diff(self.top_left.0 .1) as usize,
                )))
            }
        }
    }

    /// Expands `self` in all directions by `margin`.
    pub fn expand(&self, margin: usize) -> CanvasRect {
        let margin_i64 = margin as i64;

        let mut new_rect = *self;
        new_rect.top_left = new_rect.top_left.translate((-margin_i64, -margin_i64));
        new_rect.dimensions = Dimensions {
            width: self.dimensions.width + margin * 2,
            height: self.dimensions.height + margin * 2,
        };

        new_rect
    }
}

/// A logical layer in the canvas. Layers can be composited ontop of eachother.
#[enum_dispatch]
pub enum LayerImplementation {
    RasterLayer,
}

#[enum_dispatch(LayerImplementation)]
pub trait Layer {
    fn rasterize(&mut self, view: &CanvasView) -> BoxRasterChunk;
    fn rasterize_canvas_rect(&mut self, canvas_rect: CanvasRect) -> BoxRasterChunk;
    fn rasterize_into_bump<'bump>(
        &mut self,
        view: &CanvasView,
        bump: &'bump Bump,
    ) -> BumpRasterChunk<'bump>;
    fn rasterize_canvas_rect_into_bump<'bump>(
        &mut self,
        canvas_rect: CanvasRect,
        bump: &'bump Bump,
    ) -> BumpRasterChunk<'bump>;
    fn clear(&mut self);
}

/// A collection of layers that can be rendered.
#[derive(Default)]
pub struct Canvas {
    layers: Vec<LayerImplementation>,
    shape_cache: ShapeCache,
    rect_raster_cache: CanvasRectRasterCache,
    view_raster_cache: CanvasViewRasterCache,
}

impl Canvas {
    pub fn render(&mut self, view: &CanvasView) -> BoxRasterChunk {
        let layers = &mut self.layers;
        let raster = self
            .view_raster_cache
            .get_chunk_or_rasterize(view, &mut |c| Canvas::rasterize_canvas_rect(layers, *c));

        raster.to_chunk()
    }

    pub fn render_into_bump<'bump>(
        &mut self,
        view: &CanvasView,
        bump: &'bump Bump,
    ) -> BumpRasterChunk<'bump> {
        let layers = &mut self.layers;
        let raster = self
            .view_raster_cache
            .get_chunk_or_rasterize(view, &mut |c| Canvas::rasterize_canvas_rect(layers, *c));

        raster.to_chunk_into_bump(bump)
    }

    fn rasterize_canvas_rect(
        layers: &mut Vec<LayerImplementation>,
        canvas_rect: CanvasRect,
    ) -> BoxRasterChunk {
        let Dimensions { width, height } = canvas_rect.dimensions;
        let mut base = BoxRasterChunk::new_fill(colors::white(), width, height);

        let layer_bump = Bump::new();
        for layer in layers {
            base.composite_over(
                &layer
                    .rasterize_canvas_rect_into_bump(canvas_rect, &layer_bump)
                    .as_window(),
                (0, 0).into(),
            );
        }

        base
    }

    pub fn render_canvas_rect(&mut self, canvas_rect: CanvasRect) -> BoxRasterChunk {
        let layers = &mut self.layers;
        self.rect_raster_cache
            .get_chunk_or_rasterize(&canvas_rect, &mut |c| {
                Canvas::rasterize_canvas_rect(layers, *c)
            })
            .to_chunk()
    }

    pub fn render_canvas_rect_into_bump<'bump>(
        &mut self,
        canvas_rect: CanvasRect,
        bump: &'bump Bump,
    ) -> BumpRasterChunk<'bump> {
        let layers = &mut self.layers;
        self.rect_raster_cache
            .get_chunk_or_rasterize(&canvas_rect, &mut |c| {
                Canvas::rasterize_canvas_rect(layers, *c)
            })
            .to_chunk_into_bump(bump)
    }

    pub fn add_layer(&mut self, layer: LayerImplementation) {
        self.layers.push(layer);
    }

    pub fn perform_raster_action(
        &mut self,
        layer_num: usize,
        action: RasterLayerAction,
    ) -> Option<CanvasRect> {
        use LayerImplementation::*;
        if let Some(layer) = self.layers.get_mut(layer_num) {
            match layer {
                RasterLayer(raster_layer) => {
                    let changed_canvas_rect =
                        raster_layer.perform_action_with_cache(action, &mut self.shape_cache);

                    let layers = &mut self.layers;
                    if let Some(changed_canvas_rect) = changed_canvas_rect {
                        self.rect_raster_cache
                            .rerender_canvas_rect(&changed_canvas_rect, &mut |c| {
                                Canvas::rasterize_canvas_rect(layers, *c)
                            });
                        self.view_raster_cache
                            .rerender_canvas_rect(&changed_canvas_rect, &mut |c| {
                                Canvas::rasterize_canvas_rect(layers, *c)
                            });
                    }

                    changed_canvas_rect
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::{chunks::IndexableByPosition, Pixel, RasterLayerAction};

    #[test]
    fn test_transform_view_to_canvas() {
        let mut view = CanvasView::new(10, 10);

        view.translate((-5, -5));
        assert_eq!(
            view.transform_view_to_canvas(PixelPosition((5, 5))),
            CanvasPosition((0, 0))
        );
        assert_eq!(
            view.transform_view_to_canvas(PixelPosition((0, 5))),
            CanvasPosition((-5, 0))
        );

        view.translate((5, 5));
        view.canvas_dimensions = Dimensions {
            width: 20,
            height: 20,
        };
        assert_eq!(
            view.transform_view_to_canvas(PixelPosition((0, 1))),
            CanvasPosition((0, 2))
        );
        assert_eq!(
            view.transform_view_to_canvas(PixelPosition((5, 5))),
            CanvasPosition((10, 10))
        );
        assert_eq!(
            view.transform_view_to_canvas(PixelPosition((5, 1))),
            CanvasPosition((10, 2))
        );
    }

    #[test]
    fn test_compositing_rasters() {
        let mut canvas = Canvas::default();
        let mut red_layer = RasterLayer::new(128);
        let mut blue_layer = RasterLayer::new(128);

        let quarter = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };
        let rect = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 128,
                height: 128,
            },
        };

        red_layer.perform_action(RasterLayerAction::fill_rect(
            quarter,
            Pixel::new_rgba(255, 0, 0, 128),
        ));
        blue_layer.perform_action(RasterLayerAction::fill_rect(rect, colors::blue()));

        canvas.add_layer(blue_layer.into());
        canvas.add_layer(red_layer.into());

        let raster = canvas.render(&CanvasView::new(128, 128));

        let composited_color = Pixel::new_rgba(127, 0, 127, 255);

        for (x, y) in (0..128).zip(0..128) {
            let position = raster.get_index_from_position((x, y).into()).unwrap();
            let pixel = raster.pixels()[position];

            if x < 64 && y < 64 {
                assert!(composited_color.is_close(&pixel, 10));
            } else {
                assert!(colors::blue().is_close(&pixel, 10));
            }
        }
    }

    #[test]
    fn test_view_rect_conversion_easy() {
        let mut view = CanvasView::new(10, 15);
        view.translate((5, 5));

        let canvas_rect = CanvasRect {
            top_left: CanvasPosition((10, 10)),
            dimensions: Dimensions {
                width: 5,
                height: 10,
            },
        };

        assert_eq!(
            view.transform_canvas_rect_to_view(&canvas_rect),
            Some(ViewRect {
                top_left: PixelPosition((5, 5)),
                dimensions: Dimensions {
                    width: 5,
                    height: 10
                }
            })
        );
    }

    #[test]
    fn test_view_rect_conversion_medium() {
        let mut view = CanvasView::new(10, 20);
        view.canvas_dimensions = Dimensions {
            width: 20,
            height: 40,
        };

        let canvas_rect = CanvasRect {
            top_left: CanvasPosition((12, 10)),
            dimensions: Dimensions {
                width: 8,
                height: 10,
            },
        };

        assert_eq!(
            view.transform_canvas_rect_to_view(&canvas_rect),
            Some(ViewRect {
                top_left: PixelPosition((6, 5)),
                dimensions: Dimensions {
                    width: 4,
                    height: 5
                }
            })
        );
    }

    #[test]
    fn test_spanning_canvas_rect() {
        let rect_a = CanvasRect {
            top_left: CanvasPosition((3, 4)),
            dimensions: Dimensions {
                width: 2,
                height: 6,
            },
        };

        let rect_b = CanvasRect {
            top_left: CanvasPosition((5, 8)),
            dimensions: Dimensions {
                width: 1,
                height: 2,
            },
        };

        assert_eq!(
            rect_a.spanning_rect(&rect_b),
            CanvasRect {
                top_left: CanvasPosition((3, 4)),
                dimensions: Dimensions {
                    width: 3,
                    height: 6
                }
            }
        );

        let rect_c = CanvasRect {
            top_left: CanvasPosition((9, 2)),
            dimensions: Dimensions {
                width: 3,
                height: 5,
            },
        };

        let rect_d = CanvasRect {
            top_left: CanvasPosition((10, 1)),
            dimensions: Dimensions {
                width: 3,
                height: 7,
            },
        };

        assert_eq!(
            rect_c.spanning_rect(&rect_d),
            CanvasRect {
                top_left: CanvasPosition((9, 1)),
                dimensions: Dimensions {
                    width: 4,
                    height: 7
                }
            }
        );
    }

    #[test]
    fn test_canvas_rect_containment() {
        let rect_a = CanvasRect {
            top_left: CanvasPosition((-5, -10)),
            dimensions: Dimensions {
                width: 10,
                height: 20,
            },
        };

        assert_eq!(
            rect_a.contains_with_offset(&rect_a),
            Some(PixelPosition::from((0, 0)))
        );

        let rect_b = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };

        assert_eq!(
            rect_a.contains_with_offset(&rect_b),
            Some(PixelPosition::from((5, 10)))
        );

        let rect_c = CanvasRect {
            top_left: CanvasPosition((4, 9)),
            dimensions: Dimensions {
                width: 1,
                height: 1,
            },
        };

        assert_eq!(
            rect_a.contains_with_offset(&rect_c),
            Some(PixelPosition::from((9, 19)))
        );

        let rect_d = CanvasRect {
            top_left: CanvasPosition((5, 10)),
            dimensions: Dimensions {
                width: 1,
                height: 1,
            },
        };

        assert_eq!(rect_a.contains_with_offset(&rect_d), None);
    }

    #[test]
    fn test_canvas_rect_expansion() {
        let canvas_rect = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };

        let expanded_a = canvas_rect.expand(canvas_rect.dimensions.largest_dimension());

        let expected_a = CanvasRect {
            top_left: CanvasPosition((-64, -64)),
            dimensions: Dimensions {
                width: 64 * 3,
                height: (64 * 3),
            },
        };

        assert_eq!(expanded_a, expected_a);
    }

    #[test]
    fn view_transform_rect() {
        let canvas_view = CanvasView {
            top_left: CanvasPosition((-5, -5)),
            view_dimensions: Dimensions {
                width: 10,
                height: 10,
            },
            canvas_dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };

        let canvas_rect_a = CanvasRect {
            top_left: CanvasPosition((-5, -5)),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };

        assert_eq!(
            canvas_view.transform_canvas_rect_to_view(&canvas_rect_a),
            Some(ViewRect {
                top_left: PixelPosition((0, 0)),
                dimensions: Dimensions {
                    width: 10,
                    height: 10
                }
            })
        );

        let canvas_view = CanvasView {
            top_left: CanvasPosition((-10, -10)),
            view_dimensions: Dimensions {
                width: 10,
                height: 10,
            },
            canvas_dimensions: Dimensions {
                width: 20,
                height: 20,
            },
        };

        let canvas_rect_b = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };

        assert_eq!(
            canvas_view.transform_canvas_rect_to_view(&canvas_rect_b),
            Some(ViewRect {
                top_left: PixelPosition((5, 5)),
                dimensions: Dimensions {
                    width: 5,
                    height: 5
                }
            })
        );
    }

    #[test]
    fn canvas_view_scaling() {
        let canvas_view = CanvasView {
            top_left: CanvasPosition((10, 10)),
            view_dimensions: Dimensions {
                width: 10,
                height: 10,
            },
            canvas_dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };

        {
            let mut canvas_view = canvas_view;

            canvas_view.pin_resize_canvas(Dimensions {
                width: 20,
                height: 20,
            });

            assert_eq!(
                canvas_view,
                CanvasView {
                    top_left: CanvasPosition((5, 5)),
                    view_dimensions: Dimensions {
                        width: 10,
                        height: 10
                    },
                    canvas_dimensions: Dimensions {
                        width: 20,
                        height: 20
                    }
                }
            );
        }

        {
            let mut canvas_view = canvas_view;

            canvas_view.pin_resize_canvas(Dimensions {
                width: 5,
                height: 5,
            });

            assert_eq!(
                canvas_view,
                CanvasView {
                    top_left: CanvasPosition((12, 12)),
                    view_dimensions: Dimensions {
                        width: 10,
                        height: 10
                    },
                    canvas_dimensions: Dimensions {
                        width: 5,
                        height: 5
                    }
                }
            );
        }

        {
            let mut canvas_view = canvas_view;

            canvas_view.pin_scale_canvas(Scale {
                width_factor: 2.0,
                height_factor: 2.0,
            });

            assert_eq!(
                canvas_view,
                CanvasView {
                    top_left: CanvasPosition((5, 5)),
                    view_dimensions: Dimensions {
                        width: 10,
                        height: 10
                    },
                    canvas_dimensions: Dimensions {
                        width: 20,
                        height: 20
                    }
                }
            );
        }

        {
            let mut canvas_view = canvas_view;

            canvas_view.pin_scale(Scale {
                width_factor: 2.0,
                height_factor: 2.0,
            });

            assert_eq!(
                canvas_view,
                CanvasView {
                    top_left: CanvasPosition((5, 5)),
                    view_dimensions: Dimensions {
                        width: 20,
                        height: 20
                    },
                    canvas_dimensions: Dimensions {
                        width: 20,
                        height: 20
                    }
                }
            );
        }
    }
}
