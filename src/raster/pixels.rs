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

    pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
        let r = self.0 & 0xFF;
        let g = self.0 & 0xFF00;
        let b = self.0 & 0xFF0000;
        let a = self.0 & 0xFF000000;

        (
            r.try_into().unwrap(),
            g.try_into().unwrap(),
            b.try_into().unwrap(),
            a.try_into().unwrap(),
        )
    }
}

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
