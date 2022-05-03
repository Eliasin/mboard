use crate::raster::{chunks::RasterChunk, pixels::colors, RasterLayer, RasterLayerAction};
use enum_dispatch::enum_dispatch;

/// A view positioned relative to a set of layers.
/// The view has a scale and a width and height, the width and height are in pixel units.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CanvasView {
    top_left: (i64, i64),
    view_width: usize,
    view_height: usize,
    canvas_width: usize,
    canvas_height: usize,
}

impl CanvasView {
    /// Create a new view with a specified width and height. The default placement
    /// is at the origin with an effective scale of 1.
    pub fn new(width: usize, height: usize) -> CanvasView {
        CanvasView {
            top_left: (0, 0),
            view_width: width,
            view_height: height,
            canvas_width: width,
            canvas_height: height,
        }
    }

    /// Translate a view by `d`.
    pub fn translate(&mut self, d: (i64, i64)) {
        self.top_left = (self.top_left.0 + d.0, self.top_left.1 + d.1);
    }

    // Resizes the view to a different `(width, height)`.
    pub fn resize_view(&mut self, d: (usize, usize)) {
        self.view_width = d.0;
        self.view_height = d.1;
    }

    // Resizes the area of the canvas the view renders to a different `(width, height)`.
    pub fn resize_canvas_source(&mut self, d: (usize, usize)) {
        self.canvas_width = d.0;
        self.canvas_height = d.1;
    }

    // The dimensions of the view in `(width, height)`.
    pub fn view_dimensions(&self) -> (usize, usize) {
        (self.view_width, self.view_height)
    }

    // The dimensions of canvas area spanned by the view in `(width, height)`.
    pub fn canvas_dimensions(&self) -> (usize, usize) {
        (self.canvas_width, self.canvas_height)
    }

    // Change the canvas source of the view while preserving the middle of the view.
    pub fn pin_resize_canvas(&mut self, d: (usize, usize)) {
        let (canvas_width, canvas_height): (u32, u32) = (
            self.canvas_width.try_into().unwrap(),
            self.canvas_height.try_into().unwrap(),
        );

        let d_u32: (u32, u32) = (d.0.try_into().unwrap(), d.1.try_into().unwrap());
        let difference: (u32, u32) = (canvas_width - d_u32.0, canvas_height - d_u32.1);

        self.translate((
            (difference.0 / 2).try_into().unwrap(),
            (difference.1 / 2).try_into().unwrap(),
        ));
        self.resize_canvas_source(d);
    }

    // Scale the canvas source of the view while preserving the middle of the view.
    // Negative or factors that scale the view too small are ignored.
    pub fn pin_scale_canvas(&mut self, factor: (f32, f32)) {
        if factor.0 < 0.1 || factor.1 < 0.1 {
            return;
        }

        let new_dimensions = (
            (self.canvas_width as f32 * factor.0) as usize,
            (self.canvas_height as f32 * factor.1) as usize,
        );

        if new_dimensions.0 < 1 || new_dimensions.1 < 1 {
            return;
        }

        self.pin_resize_canvas(new_dimensions);
    }

    // The top left of a view in canvas-space.
    pub fn anchor(&self) -> (i64, i64) {
        self.top_left
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
#[derive(Default)]
pub struct Canvas {
    layers: Vec<LayerImplementation>,
}

impl Canvas {
    pub fn render(&mut self, view: &CanvasView) -> RasterChunk {
        let (view_width, view_height) = view.view_dimensions();
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
    fn test_compositing_rasters() {
        let mut canvas = Canvas::default();
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
