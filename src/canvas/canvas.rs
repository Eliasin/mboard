use crate::raster::{chunks::RasterChunk, pixels::colors, RasterLayer};

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
pub enum Layer {
    RasterLayer(RasterLayer),
}

impl Layer {
    /// Render the portion of the layer visible to the view to a `RenderBuffer`.
    pub fn rasterize(&mut self, camera: &CanvasView) -> RasterChunk {
        use Layer::*;
        match self {
            RasterLayer(raster) => raster.rasterize(camera),
        }
    }
}

impl From<RasterLayer> for Layer {
    fn from(l: RasterLayer) -> Self {
        Layer::RasterLayer(l)
    }
}

/// A collection of layers that can be rendered.
pub struct Canvas {
    layers: Vec<Layer>,
}

impl Canvas {
    pub fn render(&mut self, camera: &CanvasView) -> RasterChunk {
        let mut base = RasterChunk::new_fill(colors::white(), camera.width, camera.height);

        for layer in &mut self.layers {
            base.composite_over(&layer.rasterize(camera).as_window(), (0, 0).into());
        }

        base
    }
}

mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn test_compositing_rasters() {
        let mut red_layer = RasterLayer::new(128);
        let mut blue_layer = RasterLayer::new(128);
    }
}
