use std::ops::Mul;

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

const OVAL_PADDING: f32 = 2.2;
const HALF_OVAL_PADDING: f32 = OVAL_PADDING / 2.0;

pub struct OvalBuilder {
    width: f32,
    height: f32,
    roughness: Option<f32>,
    color: Option<Pixel>,
}

impl OvalBuilder {
    pub fn new(width: f32, height: f32) -> OvalBuilder {
        OvalBuilder {
            width,
            height,
            roughness: None,
            color: None,
        }
    }

    pub fn roughness(&mut self, roughness: f32) -> &mut Self {
        self.roughness = Some(roughness);
        self
    }

    pub fn color(&mut self, color: Pixel) -> &mut Self {
        self.color = Some(color);
        self
    }

    pub fn build(&self) -> Oval {
        Oval {
            half_width: self.width,
            half_height: self.height,
            roughness: self.roughness.unwrap_or(10.0),
            color: self.color.unwrap_or_else(colors::black),
        }
    }
}

pub struct Oval {
    half_width: f32,
    half_height: f32,
    roughness: f32,
    color: Pixel,
}

impl Oval {
    /// Create a new oval with a half width and half height. The rasterization
    /// may exactly reflect this size to account for antialiasing.
    pub fn new(half_width: f32, half_height: f32) -> Oval {
        Oval {
            half_width,
            half_height,
            roughness: 10.0,
            color: colors::black(),
        }
    }

    /// Create a new oval that fits in a bounding box, including any
    /// antialiasing.
    pub fn new_from_bound(width: u32, height: u32) -> Oval {
        let size = Oval::size_from_bound(width, height);

        Oval::new(size.0, size.1)
    }

    /// Create an `Oval` using the builder pattern.
    pub fn build(width: f32, height: f32) -> OvalBuilder {
        OvalBuilder::new(width, height)
    }

    fn size_from_bound(width: u32, height: u32) -> (f32, f32) {
        let real_width = (width as f32) / (HALF_OVAL_PADDING * 2.0);
        let real_height = (height as f32) / (HALF_OVAL_PADDING * 2.0);

        (real_width, real_height)
    }

    /// Create an `Oval` using the builder pattern and a bounding box.
    pub fn build_from_bound(width: u32, height: u32) -> OvalBuilder {
        let size = Oval::size_from_bound(width, height);

        OvalBuilder::new(size.0, size.1)
    }

    pub fn half_width(&self) -> f32 {
        self.half_width
    }

    pub fn half_height(&self) -> f32 {
        self.half_height
    }
}

impl Polygon for Oval {
    fn bounding_box(&self) -> (usize, usize) {
        let width: usize = (self.half_width * OVAL_PADDING).ceil() as usize + 1;
        let height: usize = (self.half_height * OVAL_PADDING).ceil() as usize + 1;

        (width, height)
    }

    fn inside_proportion(&self, p: &PixelPosition) -> u8 {
        let origin = (
            self.half_width * HALF_OVAL_PADDING,
            self.half_height * HALF_OVAL_PADDING,
        );

        let (x, y): (f32, f32) = (p.0 .0 as f32 - origin.0, p.0 .1 as f32 - origin.1);

        let dist =
            f32::sqrt(x.powi(2) / self.half_width.powi(2) + y.powi(2) / self.half_height.powi(2));

        if dist < 1.0 {
            255
        } else {
            ((1.0 - (dist - 1.0).mul(self.roughness)) * 255.0).clamp(0.0, 255.0) as u8
        }
    }

    fn color_from_inside_proportion(&self, p: u8) -> Pixel {
        let u = p as f32 / 255.0;
        let (r, g, b, a) = self.color.as_rgba();

        let (r, g, b, a): (u8, u8, u8, u8) = (
            (r as f32 * u).clamp(0.0, 255.0) as u8,
            (g as f32 * u).clamp(0.0, 255.0) as u8,
            (b as f32 * u).clamp(0.0, 255.0) as u8,
            (a as f32 * u).clamp(0.0, 255.0) as u8,
        );

        Pixel::new_rgba(r, g, b, a)
    }
}

pub struct Circle {
    oval: Oval,
    roughness: f32,
}

impl Circle {
    pub fn new(radius: f32) -> Circle {
        Circle {
            oval: Oval::new(radius, radius),
            roughness: 10.0,
        }
    }

    pub fn new_roughness(radius: f32, roughness: f32) -> Circle {
        Circle {
            oval: Oval::new(radius, radius),
            roughness,
        }
    }

    pub fn radius(&self) -> f32 {
        self.oval.half_width
    }

    pub fn roughness(&self) -> f32 {
        self.roughness
    }
}

impl Polygon for Circle {
    fn bounding_box(&self) -> (usize, usize) {
        self.oval.bounding_box()
    }

    fn inside_proportion(&self, p: &PixelPosition) -> u8 {
        self.oval.inside_proportion(p)
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
