use crate::raster::{chunks::RasterChunk, RasterLayer};

/// A view positioned relative to a set of layers.
/// The view has a scale and a width and height, the width and height are in pixel units.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Camera {
    pub top_left: (i64, i64),
    pub width: usize,
    pub height: usize,
    scale: u32,
}

impl Camera {
    /// Create a new camera with a specified width and height. The default placement
    /// is at the origin with an effective scale of 1.
    pub fn new(width: usize, height: usize) -> Camera {
        Camera {
            top_left: (0, 0),
            width,
            height,
            scale: 512,
        }
    }

    /// Translate a camera by `v`.
    pub fn translate(&mut self, v: (i64, i64)) {
        self.top_left = (self.top_left.0 + v.0, self.top_left.1 + v.1);
    }

    /// Get the scale of a camera.
    pub fn scale(&self) -> f32 {
        self.scale as f32 / 512.0
    }
}

/// A rectangle in canvas-space that can be used for operations
/// on layers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CanvasRect {
    top_left: (i64, i64),
    width: u32,
    height: u32,
}

/// A logical layer in the canvas. Layers can be composited ontop of eachother.
pub enum Layer {
    RasterLayer(RasterLayer),
}

impl Layer {
    /// Render the portion of the layer visible to the camera to a `RenderBuffer`.
    pub fn rasterize(&mut self, camera: &Camera) -> RasterChunk {
        use Layer::*;
        match self {
            RasterLayer(raster) => raster.rasterize(camera),
        }
    }
}

/// A collection of layers that can be rendered.
pub struct Canvas {
    layers: Vec<Layer>,
}
