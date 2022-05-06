use crate::raster::{
    chunks::RasterChunk,
    layer::ChunkPosition,
    pixels::colors,
    position::{Dimensions, PixelPosition, Scale},
    RasterLayer, RasterLayerAction,
};
use enum_dispatch::enum_dispatch;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

    // Change the canvas source of the view while preserving the middle of the view.
    pub fn pin_resize_canvas(&mut self, d: Dimensions) {
        let difference = self.canvas_dimensions.difference(d);

        self.top_left = self.top_left.translate_scaled(difference, 2);
        self.canvas_dimensions = d;
    }

    // Scale the canvas source of the view while preserving the middle of the view.
    // Negative or factors that scale the view too small are ignored.
    pub fn pin_scale_canvas(&mut self, factor: Scale) {
        let new_dimensions = self.canvas_dimensions.scale(factor);

        if new_dimensions.width < 1 || new_dimensions.height < 1 {
            return;
        }

        self.pin_resize_canvas(new_dimensions);
    }

    /// Transforms a point from view space to canvas space.
    pub fn transform_view_to_canvas(&self, p: PixelPosition) -> CanvasPosition {
        let scaled_point = self
            .canvas_dimensions
            .transform_point(p, self.view_dimensions);

        CanvasPosition::from_view_position(*self, scaled_point)
    }
}

/// A rectangle in canvas-space that can be used for operations
/// on layers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CanvasRect {
    pub top_left: CanvasPosition,
    pub dimensions: Dimensions,
}

/// A logical layer in the canvas. Layers can be composited ontop of eachother.
#[enum_dispatch]
pub enum LayerImplementation {
    RasterLayer,
}

#[enum_dispatch(LayerImplementation)]
pub trait Layer {
    fn rasterize(&mut self, view: &CanvasView) -> RasterChunk;
}

/// A collection of layers that can be rendered.
#[derive(Default)]
pub struct Canvas {
    layers: Vec<LayerImplementation>,
}

impl Canvas {
    pub fn render(&mut self, view: &CanvasView) -> RasterChunk {
        let Dimensions {
            width: view_width,
            height: view_height,
        } = view.view_dimensions;
        let mut base = RasterChunk::new_fill(colors::white(), view_width, view_height);

        for layer in &mut self.layers {
            base.composite_over(&layer.rasterize(view).as_window(), (0, 0).into());
        }

        base
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
                RasterLayer(raster_layer) => raster_layer.perform_action(action),
            }
        } else {
            None
        }
    }
}

mod tests {
    #[cfg(test)]
    use crate::raster::{chunks::IndexableByPosition, Pixel, RasterLayerAction};

    #[cfg(test)]
    use super::*;

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
}
