use std::ops::Add;

/// A position within a 2d collection of pixels.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PixelPosition(pub (usize, usize));

/// An position in which to blit something. Does not have to be inside the
/// destination pixel collection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BlitPosition(pub (i32, i32));

impl Add<PixelPosition> for PixelPosition {
    type Output = PixelPosition;

    fn add(self, rhs: Self) -> Self::Output {
        PixelPosition((self.0 .0 + rhs.0 .0, self.0 .1 + rhs.0 .1))
    }
}

impl Add<(usize, usize)> for PixelPosition {
    type Output = PixelPosition;

    fn add(self, rhs: (usize, usize)) -> Self::Output {
        PixelPosition((self.0 .0 + rhs.0, self.0 .1 + rhs.1))
    }
}

impl Add<BlitPosition> for PixelPosition {
    type Output = BlitPosition;

    fn add(self, rhs: BlitPosition) -> Self::Output {
        BlitPosition((self.0 .0 as i32 + rhs.0 .0, self.0 .1 as i32 + rhs.0 .1))
    }
}
impl Add<(i32, i32)> for BlitPosition {
    type Output = BlitPosition;

    fn add(self, rhs: (i32, i32)) -> Self::Output {
        BlitPosition((self.0 .0 + rhs.0, self.0 .1 + rhs.1))
    }
}

impl From<(i32, i32)> for BlitPosition {
    fn from(p: (i32, i32)) -> Self {
        BlitPosition(p)
    }
}

impl From<(usize, usize)> for PixelPosition {
    fn from(p: (usize, usize)) -> Self {
        PixelPosition(p)
    }
}
