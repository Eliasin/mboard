use crate::primitives::position::CanvasPosition;

use super::shapes::RasterizablePolygon;

pub struct VectorLayer {
    shapes: Vec<(CanvasPosition, Box<dyn RasterizablePolygon>)>,
}
