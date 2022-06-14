use crate::canvas::CanvasPosition;

use super::shapes::RasterizablePolygon;

pub struct VectorLayer {
    shapes: Vec<(CanvasPosition, Box<dyn RasterizablePolygon>)>,
}
