use crate::raster::{chunks::RasterChunk, pixels::colors, RasterLayer};
use enum_dispatch::enum_dispatch;

/// A view positioned relative to a set of layers.
/// The view has a scale and a width and height, the width and height are in pixel units.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CanvasView {
    pub top_left: (i64, i64),
    pub width: usize,
    pub height: usize,
    scale: u32,
}

impl CanvasView {
    /// Create a new view with a specified width and height. The default placement
    /// is at the origin with an effective scale of 1.
    pub fn new(width: usize, height: usize) -> CanvasView {
        CanvasView {
            top_left: (0, 0),
            width,
            height,
            scale: 512,
        }
    }

    /// Translate a view by `v`.
    pub fn translate(&mut self, v: (i64, i64)) {
        self.top_left = (self.top_left.0 + v.0, self.top_left.1 + v.1);
    }

    /// Get the scale of a view.
    pub fn scale(&self) -> f32 {
        self.scale as f32 / 512.0
    }
}

/// A rectangle in canvas-space that can be used for operations
/// on layers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CanvasRect {
    pub top_left: (i64, i64),
    pub width: u32,
    pub height: u32,
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
pub struct Canvas {
    layers: Vec<LayerImplementation>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas { layers: vec![] }
    }

    pub fn render(&mut self, view: &CanvasView) -> RasterChunk {
        let mut base = RasterChunk::new_fill(colors::white(), view.width, view.height);

        for layer in &mut self.layers {
            base.composite_over(&layer.rasterize(view).as_window(), (0, 0).into());
        }

        base
    }

    pub fn add_layer(&mut self, layer: LayerImplementation) {
        self.layers.push(layer);
    }
}

mod tests {
    #[cfg(test)]
    use crate::raster::{chunks::IndexableByPosition, Pixel, RasterCanvasAction};

    #[cfg(test)]
    use super::*;

    #[test]
    fn test_compositing_rasters() {
        let mut canvas = Canvas::new();
        let mut red_layer = RasterLayer::new(128);
        let mut blue_layer = RasterLayer::new(128);

        let quarter = CanvasRect {
            top_left: (0, 0),
            width: 64,
            height: 64,
        };
        let rect = CanvasRect {
            top_left: (0, 0),
            width: 128,
            height: 128,
        };

        red_layer.perform_action(RasterCanvasAction::fill_rect(
            quarter,
            Pixel::new_rgba(255, 0, 0, 128),
        ));
        blue_layer.perform_action(RasterCanvasAction::fill_rect(rect, colors::blue()));

        canvas.add_layer(blue_layer.into());
        canvas.add_layer(red_layer.into());

        let raster = canvas.render(&CanvasView::new(128, 128));

        let composited_color = Pixel::new_rgba(127, 0, 127, 255);

        for (x, y) in (0..128).zip(0..128) {
            let position = raster.get_index_from_position((x, y).into()).unwrap();
            let pixel = raster.pixels()[position];

            println!("{:?}", pixel.as_rgba());

            if x < 64 && y < 64 {
                assert!(composited_color.is_close(&pixel, 10));
            } else {
            }
        }
    }
}
