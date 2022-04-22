//! An RGBA pixel type that supports alpha compositing.

use std::convert::TryInto;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pixel(u32);

impl Pixel {
    pub fn new_rgb(r: u8, g: u8, b: u8) -> Pixel {
        Pixel::new_rgba(r, g, b, 255)
    }

    pub fn new_rgba(r: u8, g: u8, b: u8, a: u8) -> Pixel {
        let r = r as u32;
        let g = g as u32;
        let b = b as u32;
        let a = a as u32;
        Pixel(r + (g << 8) + (b << 16) + (a << 24))
    }

    pub fn new_rgb_norm(r: f32, g: f32, b: f32) -> Pixel {
        let r = (r.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (g.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (b.clamp(0.0, 1.0) * 255.0) as u32;

        Pixel(r + (g << 8) + (b << 16) + (255 << 24))
    }

    pub fn new_rgba_norm(r: f32, g: f32, b: f32, a: f32) -> Pixel {
        let r = (r.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (g.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (b.clamp(0.0, 1.0) * 255.0) as u32;
        let a = (a.clamp(0.0, 1.0) * 255.0) as u32;

        Pixel(r + (g << 8) + (b << 16) + (a << 24))
    }

    pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
        let r = self.0 & 0xFF;
        let g = (self.0 & 0xFF00) >> 8;
        let b = (self.0 & 0xFF0000) >> 16;
        let a = (self.0 & 0xFF000000) >> 24;

        (
            r.try_into().unwrap(),
            g.try_into().unwrap(),
            b.try_into().unwrap(),
            a.try_into().unwrap(),
        )
    }

    pub fn as_norm_rgba(&self) -> (f32, f32, f32, f32) {
        let (r, g, b, a) = self.as_rgba();
        (
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    fn composite_norm_component(c_a: f32, a_a: f32, c_b: f32, c: f32, a_o: f32) -> f32 {
        let w = c_a * a_a + c_b * c;

        w / a_o
    }

    pub fn composite_over(&mut self, other: &Self) {
        let (a_r, a_g, a_b, a_a) = other.as_norm_rgba();
        let (b_r, b_g, b_b, b_a) = self.as_norm_rgba();

        let c = b_a * (1.0 - a_a);

        let a_o: f32 = a_a + c;

        let new_pixel = Pixel::new_rgba_norm(
            Pixel::composite_norm_component(a_r, a_a, b_r, c, a_o),
            Pixel::composite_norm_component(a_g, a_a, b_g, c, a_o),
            Pixel::composite_norm_component(a_b, a_a, b_b, c, a_o),
            a_o,
        );

        *self = new_pixel;
    }

    pub fn is_close(&self, other: &Pixel, delta: u8) -> bool {
        let (r, g, b, a) = self.as_rgba();
        let (o_r, o_g, o_b, o_a) = other.as_rgba();

        r.abs_diff(o_r) < delta
            && g.abs_diff(o_g) < delta
            && b.abs_diff(o_b) < delta
            && a.abs_diff(o_a) < delta
    }
}

/// Common color definitions.
pub mod colors {
    use super::Pixel;

    pub fn red() -> Pixel {
        Pixel::new_rgb(255, 0, 0)
    }

    pub fn green() -> Pixel {
        Pixel::new_rgb(0, 255, 0)
    }

    pub fn blue() -> Pixel {
        Pixel::new_rgb(0, 0, 255)
    }

    pub fn transparent() -> Pixel {
        Pixel::new_rgba(0, 0, 0, 0)
    }
}
