//! Manipulation of raster data in the form of discretized chunks.

pub mod chunks;
pub mod layer;
pub mod pixels;
pub mod position;
pub mod shapes;

pub use layer::{RasterCanvasAction, RasterLayer};
pub use pixels::Pixel;
