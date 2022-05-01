use std::ops::Div;

use super::{
    chunks::{RasterChunk, RasterWindow},
    pixels::colors,
    position::PixelPosition,
    Pixel,
};

/// A polygon represented as a finite bounding box and
/// a discriminator to check that a pixel within the bounding
/// box is inside.
pub trait Polygon {
    /// The minimum size box to bound this polygon, given in `(width, height)`.
    fn bounding_box(&self) -> (usize, usize);
    /// How much of the pixel at `p` is inside of this polygon.
    /// `0` being completely outside and `255` being completely inside.
    fn inside_proportion(&self, p: &PixelPosition) -> u8;
    /// The color a pixel should be given how `inside` the shape it is.
    fn color_from_inside_proportion(&self, p: u8) -> Pixel {
        greyscale_from_proportion_inside(p)
    }
}

/// A way to rasterize a polygon.
pub trait RasterPolygon {
    /// Rasterization of the polygon as a raster chunk.
    fn rasterize(&self) -> RasterChunk;
}

fn greyscale_from_proportion_inside(proportion_inside: u8) -> Pixel {
    let u = 255 - proportion_inside;

    Pixel::new_rgba(u, u, u, proportion_inside)
}

impl<T: Polygon> RasterPolygon for T {
    fn rasterize(&self) -> RasterChunk {
        let bounding_box = self.bounding_box();

        let (width, height) = bounding_box;
        let mut pixels = vec![colors::transparent(); width * height];

        for y in 0..height {
            for x in 0..width {
                let p = (x, y).into();
                let inside_proportion = self.inside_proportion(&p);
                let color = self.color_from_inside_proportion(inside_proportion);
                pixels[x + y * width] = color;
            }
        }

        RasterWindow::from_slice(pixels.as_slice(), width, height)
            .unwrap()
            .to_chunk()
    }
}

pub struct Circle {
    radius: f32,
    roughness: f32,
}

impl Circle {
    pub fn new(radius: f32) -> Circle {
        Circle {
            radius,
            roughness: 10.0,
        }
    }

    pub fn new_roughness(radius: f32, roughness: f32) -> Circle {
        Circle { radius, roughness }
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn roughness(&self) -> f32 {
        self.roughness
    }
}

const CIRCLE_PADDING: f32 = 2.2;
const HALF_CIRCLE_PADDING: f32 = CIRCLE_PADDING / 2.0;

impl Polygon for Circle {
    fn bounding_box(&self) -> (usize, usize) {
        let d: usize = (self.radius * CIRCLE_PADDING).ceil() as usize + 1;
        (d, d)
    }

    fn inside_proportion(&self, p: &PixelPosition) -> u8 {
        let origin = (
            self.radius * HALF_CIRCLE_PADDING,
            self.radius * HALF_CIRCLE_PADDING,
        );

        let (x, y): (f32, f32) = (p.0 .0 as f32 - origin.0, p.0 .1 as f32 - origin.1);

        let dist = f32::sqrt(x.powi(2) + y.powi(2));

        if dist < self.radius {
            255
        } else {
            ((1.0 - (dist - self.radius).div(self.radius / self.roughness)) * 255.0)
                .clamp(0.0, 255.0) as u8
        }
    }
}

mod tests {
    #[cfg(test)]
    use crate::raster::chunks::IndexableByPosition;

    #[cfg(test)]
    use super::*;

    #[test]
    fn test_circle_rasterization() {
        let radius = 10.0;
        let roughness = 1000.0;
        let circle = Circle::new_roughness(radius, roughness);

        let raster = circle.rasterize();
        let (width, height) = circle.bounding_box();

        for (x, y) in (0..width).zip(0..height) {
            let position = raster.get_index_from_position((x, y).into()).unwrap();

            let (x, y) = (x as f32, y as f32);
            let origin = (width as f32 / 2.0, height as f32 / 2.0);
            let dist = f32::sqrt((x - origin.0).powi(2) + (y - origin.1).powi(2));

            if dist < radius {
                assert!(raster.pixels()[position].is_close(&colors::black(), 2));
            } else if dist > radius * 1.1 {
                assert!(raster.pixels()[position].is_close(&Pixel::new_rgba(255, 255, 255, 0), 10));
            }
        }
    }
}
