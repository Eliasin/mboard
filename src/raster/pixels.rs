//! An RGBA pixel type that supports alpha compositing.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pixel(pub u32);

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

    /// Creates a pixel from normalized RGB values,
    /// inputs will be clamped to [0, 1].
    pub fn new_rgb_norm(r: f32, g: f32, b: f32) -> Pixel {
        let r = (r.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (g.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (b.clamp(0.0, 1.0) * 255.0) as u32;

        Pixel(r + (g << 8) + (b << 16) + (255 << 24))
    }

    /// Creates a pixel from normalized RGBA values,
    /// inputs will be clamped to [0, 1].
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

        (r as u8, g as u8, b as u8, a as u8)
    }

    fn as_rgba_u32(&self) -> (u32, u32, u32, u32) {
        let r = self.0 & 0xFF;
        let g = (self.0 & 0xFF00) >> 8;
        let b = (self.0 & 0xFF0000) >> 16;
        let a = (self.0 & 0xFF000000) >> 24;

        (r, g, b, a)
    }

    /// Get the RGBA values of a pixel as normalized components in
    /// the range \[0,1\].
    pub fn as_norm_rgba(&self) -> (f32, f32, f32, f32) {
        let (r, g, b, a) = self.as_rgba();
        (
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    fn composite_alpha(a1: u32, a2: u32) -> u32 {
        (a1 + a2 - ((a1 * a2) >> 8)).min(255)
    }

    fn composite_component(c1: u32, a1: u32, c2: u32, a2: u32, a_o: u32) -> u32 {
        if a_o == 0 {
            return 255;
        }

        ((c1 * a1 + c2 * a2 - ((c2 * a2 * a1) >> 8)) / a_o).min(255)
    }

    /// Composes another pixel over this one.
    pub fn composite_over(&mut self, over: &Self) {
        let (r1, g1, b1, a1) = over.as_rgba_u32();
        let (r2, g2, b2, a2) = self.as_rgba_u32();

        let a_o = Pixel::composite_alpha(a1, a2);
        let a_o_u32 = a_o as u32;

        let (nr, ng, nb) = (
            Pixel::composite_component(r1, a1, r2, a2, a_o_u32),
            Pixel::composite_component(g1, a1, g2, a2, a_o_u32),
            Pixel::composite_component(b1, a1, b2, a2, a_o_u32),
        );

        self.0 = nr + (ng << 8) + (nb << 16) + (a_o << 24);
    }

    /// Returns whether a pixel is `close` to another. A pixel is `close` to
    /// another if the difference between each pixel's value is lesser than
    /// the provided delta.
    pub fn is_close(&self, other: &Pixel, delta: u8) -> bool {
        let (r, g, b, a) = self.as_rgba();
        let (o_r, o_g, o_b, o_a) = other.as_rgba();

        r.abs_diff(o_r) <= delta
            && g.abs_diff(o_g) <= delta
            && b.abs_diff(o_b) <= delta
            && a.abs_diff(o_a) <= delta
    }

    /// Returns the euclidean distance from one pixel to another.
    pub fn eu_distance(&self, other: &Pixel) -> f32 {
        let (r_a, g_a, b_a, a_a) = self.as_norm_rgba();
        let (r_b, g_b, b_b, a_b) = other.as_norm_rgba();

        let r = (r_a - r_b).powf(2.0);
        let g = (g_a - g_b).powf(2.0);
        let b = (b_a - b_b).powf(2.0);
        let a = (a_a - a_b).powf(2.0);

        f32::sqrt(r + g + b + a)
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

    pub fn black() -> Pixel {
        Pixel::new_rgb(0, 0, 0)
    }

    pub fn white() -> Pixel {
        Pixel::new_rgb(255, 255, 255)
    }

    pub fn grey() -> Pixel {
        Pixel::new_rgb(128, 128, 128)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositing() {
        let mut should_be_blue = colors::red();
        should_be_blue.composite_over(&colors::blue());

        assert!(should_be_blue.is_close(&colors::blue(), 2));

        let mut should_be_grey = Pixel::new_rgba(128, 128, 128, 255);

        should_be_grey.composite_over(&Pixel::new_rgba(255, 255, 255, 128));

        assert!(should_be_grey.is_close(&Pixel::new_rgba(191, 191, 191, 255), 2));
    }

    #[cfg(test)]
    fn float_max_delta(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> f32 {
        (a.0 - b.0)
            .abs()
            .max((a.1 - b.1).abs())
            .max((a.2 - b.2).abs())
            .max((a.3 - b.3).abs())
    }

    #[test]
    fn inverse() {
        assert_eq!(Pixel::new_rgba(2, 4, 8, 16).as_rgba(), (2, 4, 8, 16));
        assert!(
            float_max_delta(
                Pixel::new_rgba_norm(0.1, 0.3, 0.5, 0.8).as_norm_rgba(),
                (0.1, 0.3, 0.5, 0.8)
            ) < 0.01
        );
    }

    #[test]
    fn is_close() {
        assert!(colors::red().is_close(&colors::red(), 0));

        assert!(Pixel::new_rgb(245, 0, 0).is_close(&colors::red(), 10));

        assert!(!Pixel::new_rgb(240, 0, 0).is_close(&colors::red(), 10));

        assert!(!colors::red().is_close(&colors::blue(), 128));
    }

    #[test]
    fn rgb_default() {
        assert_eq!(Pixel::new_rgba(255, 0, 0, 255), Pixel::new_rgb(255, 0, 0));
    }
}
