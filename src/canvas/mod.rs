use crate::{
    primitives::{
        dimensions::{Dimensions, Scale},
        position::{CanvasPosition, PixelPosition, UncheckedIntoPosition},
        rect::{CanvasRect, ViewRect},
    },
    raster::{
        chunks::{nn_map::NearestNeighbourMap, raster_chunk::BumpRasterChunk, BoxRasterChunk},
        pixels::colors,
        RasterLayer, RasterLayerAction,
    },
};
use bumpalo::Bump;
use enum_dispatch::enum_dispatch;

mod cache;
pub use cache::ShapeCache;

use self::cache::{CanvasRectRasterCache, CanvasViewRasterCache};

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
            top_left: (0, 0).into(),
            view_dimensions: Dimensions { width, height },
            canvas_dimensions: Dimensions { width, height },
        }
    }

    /// Translate a view by an offset.
    pub fn translate(&mut self, d: CanvasPosition) {
        self.top_left = self.top_left.translate(d);
    }

    /// Change the canvas dimensions of the view while preserving the middle of the view.
    pub fn pin_resize_canvas(&mut self, d: Dimensions) {
        let difference = self.canvas_dimensions.difference(d);

        self.top_left = self.top_left.translate_scaled(difference.into(), 2);
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

        self.top_left = self.top_left.translate_scaled(difference.into(), 2);
        self.canvas_dimensions = new_canvas_dimensions;
        self.view_dimensions = new_view_dimensions;
    }

    /// Transforms a point from view space to canvas space.
    pub fn transform_view_to_canvas(&self, p: PixelPosition) -> CanvasPosition {
        let scaled_point = self
            .canvas_dimensions
            .transform_point(p, self.view_dimensions);

        (self.top_left + scaled_point.unchecked_into_position()).into()
    }

    /// Attempt to transform a position in canvas space to a position
    /// in view space. Canvas positions not in view will map to `None`;
    pub fn transform_canvas_to_view(&self, p: CanvasPosition) -> Option<PixelPosition> {
        let translated_point = p.translate((-self.top_left.0, -self.top_left.1).into());

        let point_past_top_left = translated_point.0 < 0 || translated_point.1 < 0;
        let point_past_bottom_right = translated_point.0 > self.canvas_dimensions.width as i32
            || translated_point.1 > self.canvas_dimensions.height as i32;
        if point_past_top_left || point_past_bottom_right {
            None
        } else {
            Some(self.view_dimensions.transform_point(
                translated_point.unchecked_into_position(),
                self.canvas_dimensions,
            ))
        }
    }

    /// Attempt to transform a rect in canvas space to a rect
    /// in view space. Canvas rects not fully in view will map to `None`;
    pub fn transform_canvas_rect_to_view(&self, r: &CanvasRect) -> Option<ViewRect> {
        let top_left = self.transform_canvas_to_view(r.top_left)?;
        let bottom_right = self.transform_canvas_to_view(r.bottom_right())?;

        Some(ViewRect::from_points(top_left, bottom_right))
    }

    /// Transform a rect in view space to a rect in canvas space.
    pub fn transform_view_rect_to_canvas(&self, r: &ViewRect) -> CanvasRect {
        let top_left = self.transform_view_to_canvas(r.top_left);
        let bottom_right = self.transform_view_to_canvas(r.bottom_right());

        CanvasRect::from_points(top_left, bottom_right)
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

    /// Compares equality of scales for two canvas views. Since scales can have some
    /// rounding, this equality evaluates as true for scales that are "close enough".
    pub fn scale_eq(&self, other: &CanvasView) -> bool {
        let scale = self.canvas_dimensions.relative_scale(self.view_dimensions);

        let other_scale = other
            .canvas_dimensions
            .relative_scale(other.view_dimensions);

        scale.similar_to(other_scale)
    }

    /// A subview of this view that contains a given canvas rect. The scale of the subview
    /// is derived from this view.
    pub fn canvas_rect_subview(&self, canvas_rect: &CanvasRect) -> Option<CanvasView> {
        let view_rect = self.transform_canvas_rect_to_view(canvas_rect)?;

        Some(CanvasView {
            top_left: canvas_rect.top_left,
            canvas_dimensions: canvas_rect.dimensions,
            view_dimensions: view_rect.dimensions,
        })
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
            .get_chunk_or_rasterize(view, &mut |c| {
                Canvas::rasterize_canvas_rect_uncached(layers, *c)
            });

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
            .get_chunk_or_rasterize(view, &mut |c| {
                Canvas::rasterize_canvas_rect_uncached(layers, *c)
            });

        raster.to_chunk_into_bump(bump)
    }

    fn rasterize_canvas_rect_uncached(
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

    pub fn rasterize_canvas_rect(&mut self, canvas_rect: CanvasRect) -> BoxRasterChunk {
        let layers = &mut self.layers;
        self.rect_raster_cache
            .get_chunk_or_rasterize(&canvas_rect, &mut |c| {
                Canvas::rasterize_canvas_rect_uncached(layers, *c)
            })
            .to_chunk()
    }

    pub fn rasterize_canvas_rect_into_bump<'bump>(
        &mut self,
        canvas_rect: CanvasRect,
        bump: &'bump Bump,
    ) -> BumpRasterChunk<'bump> {
        let layers = &mut self.layers;
        self.rect_raster_cache
            .get_chunk_or_rasterize(&canvas_rect, &mut |c| {
                Canvas::rasterize_canvas_rect_uncached(layers, *c)
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
                                Canvas::rasterize_canvas_rect_uncached(layers, *c)
                            });
                        self.view_raster_cache
                            .rerender_canvas_rect(&changed_canvas_rect, &mut |c| {
                                Canvas::rasterize_canvas_rect_uncached(layers, *c)
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
    use crate::{
        primitives::rect::ViewRect,
        raster::{chunks::IndexableByPosition, Pixel, RasterLayerAction},
    };

    #[test]
    fn transform_view_to_canvas() {
        let mut view = CanvasView::new(10, 10);

        view.translate((-5, -5).into());
        assert_eq!(view.transform_view_to_canvas((5, 5).into()), (0, 0).into());
        assert_eq!(view.transform_view_to_canvas((0, 5).into()), (-5, 0).into());

        view.translate((5, 5).into());
        view.canvas_dimensions = Dimensions {
            width: 20,
            height: 20,
        };
        assert_eq!(view.transform_view_to_canvas((0, 1).into()), (0, 2).into());
        assert_eq!(
            view.transform_view_to_canvas((5, 5).into()),
            (10, 10).into()
        );
        assert_eq!(view.transform_view_to_canvas((5, 1).into()), (10, 2).into());
    }

    #[test]
    fn compositing_rasters() {
        let mut canvas = Canvas::default();
        let mut red_layer = RasterLayer::new(128);
        let mut blue_layer = RasterLayer::new(128);

        let quarter = CanvasRect {
            top_left: (0, 0).into(),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };
        let rect = CanvasRect {
            top_left: (0, 0).into(),
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
    fn view_rect_conversion_easy() {
        let mut view = CanvasView::new(10, 15);
        view.translate((5, 5).into());

        let canvas_rect = CanvasRect {
            top_left: (10, 10).into(),
            dimensions: Dimensions {
                width: 5,
                height: 10,
            },
        };

        assert_eq!(
            view.transform_canvas_rect_to_view(&canvas_rect),
            Some(ViewRect {
                top_left: (5, 5).into(),
                dimensions: Dimensions {
                    width: 5,
                    height: 10
                }
            })
        );
    }

    #[test]
    fn view_rect_conversion_medium() {
        let mut view = CanvasView::new(10, 20);
        view.canvas_dimensions = Dimensions {
            width: 20,
            height: 40,
        };

        let canvas_rect = CanvasRect {
            top_left: (12, 10).into(),
            dimensions: Dimensions {
                width: 8,
                height: 10,
            },
        };

        assert_eq!(
            view.transform_canvas_rect_to_view(&canvas_rect),
            Some(ViewRect {
                top_left: (6, 5).into(),
                dimensions: Dimensions {
                    width: 4,
                    height: 5
                }
            })
        );
    }

    #[test]
    fn spanning_canvas_rect() {
        let rect_a = CanvasRect {
            top_left: (3, 4).into(),
            dimensions: Dimensions {
                width: 2,
                height: 6,
            },
        };

        let rect_b = CanvasRect {
            top_left: (5, 8).into(),
            dimensions: Dimensions {
                width: 1,
                height: 2,
            },
        };

        assert_eq!(
            rect_a.spanning_rect(&rect_b),
            CanvasRect {
                top_left: (3, 4).into(),
                dimensions: Dimensions {
                    width: 3,
                    height: 6
                }
            }
        );

        let rect_c = CanvasRect {
            top_left: (9, 2).into(),
            dimensions: Dimensions {
                width: 3,
                height: 5,
            },
        };

        let rect_d = CanvasRect {
            top_left: (10, 1).into(),
            dimensions: Dimensions {
                width: 3,
                height: 7,
            },
        };

        assert_eq!(
            rect_c.spanning_rect(&rect_d),
            CanvasRect {
                top_left: (9, 1).into(),
                dimensions: Dimensions {
                    width: 4,
                    height: 7
                }
            }
        );
    }

    #[test]
    fn canvas_rect_containment() {
        let rect_a = CanvasRect {
            top_left: (-5, -1).into(),
            dimensions: Dimensions {
                width: 10,
                height: 20,
            },
        };

        assert_eq!(rect_a.contains_with_offset(&rect_a), Some((0, 0).into()));

        let rect_b = CanvasRect {
            top_left: (0, 0).into(),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };

        assert_eq!(rect_a.contains_with_offset(&rect_b), Some((5, 1).into()));

        let rect_c = CanvasRect {
            top_left: (4, 9).into(),
            dimensions: Dimensions {
                width: 1,
                height: 1,
            },
        };

        assert_eq!(
            rect_a.contains_with_offset(&rect_c),
            Some(PixelPosition::from((9, 10)))
        );

        let rect_d = CanvasRect {
            top_left: (5, 10).into(),
            dimensions: Dimensions {
                width: 1,
                height: 1,
            },
        };

        assert_eq!(rect_a.contains_with_offset(&rect_d), None);
    }

    #[test]
    fn canvas_rect_expansion() {
        let canvas_rect = CanvasRect {
            top_left: (0, 0).into(),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };

        let expanded_a = canvas_rect.expand(canvas_rect.dimensions.largest_dimension());

        let expected_a = CanvasRect {
            top_left: (-64, -64).into(),
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
            top_left: (-5, -5).into(),
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
            top_left: (-5, -5).into(),
            dimensions: Dimensions {
                width: 5,
                height: 5,
            },
        };

        assert_eq!(
            canvas_view.transform_canvas_rect_to_view(&canvas_rect_a),
            Some(ViewRect {
                top_left: (0, 0).into(),
                dimensions: Dimensions {
                    width: 10,
                    height: 10
                }
            })
        );

        let canvas_view = CanvasView {
            top_left: (-10, -10).into(),
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
            top_left: (0, 0).into(),
            dimensions: Dimensions {
                width: 10,
                height: 10,
            },
        };

        assert_eq!(
            canvas_view.transform_canvas_rect_to_view(&canvas_rect_b),
            Some(ViewRect {
                top_left: (5, 5).into(),
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
            top_left: (10, 10).into(),
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
                    top_left: (5, 5).into(),
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
                    top_left: (12, 12).into(),
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
                    top_left: (5, 5).into(),
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
                    top_left: (5, 5).into(),
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
