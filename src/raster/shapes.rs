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
    /// The minimum size box to bound this polygon.
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

    Pixel::new_rgba(u, u, u, u)
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
}

impl Circle {
    pub fn new(radius: f32) -> Circle {
        Circle { radius }
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }
}

impl Polygon for Circle {
    fn bounding_box(&self) -> (usize, usize) {
        let d: usize = (self.radius * 2.1).ceil() as usize;
        (d, d)
    }

    fn inside_proportion(&self, p: &PixelPosition) -> u8 {
        let (x, y): (f32, f32) = (p.0 .0 as f32 - self.radius, p.0 .1 as f32 - self.radius);

        ((f32::sqrt(x.powi(2) + y.powi(2)) - self.radius)
            .abs()
            .div(self.radius)
            .clamp(0.0, 1.0)
            * 255.0) as u8
    }
}

mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn test_circle_rasterization() {
        let circle = Circle::new(5.0);

        let raster = circle.rasterize();

        println!("{}", raster);
        assert!(false);
    }
}
