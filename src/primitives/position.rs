use std::ops::Add;

use num::cast::AsPrimitive;

use super::dimensions::Dimensions;

/// Generic position with underlying storage type for coordindates. Implements
/// basic operations like converting between different position types and translation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Position<T>(pub T, pub T);

impl<T: Add<Output = T> + Copy> Position<T> {
    /// Translates a position by another, same as the `Add` impl
    /// but more explicit.
    pub fn translate(&self, v: Position<T>) -> Position<T> {
        *self + v
    }
}

impl<T> From<Position<T>> for (T, T) {
    fn from(p: Position<T>) -> Self {
        (p.0, p.1)
    }
}

pub trait IntoPosition<T> {
    fn into_position(self) -> Position<T>;
}

pub trait TryIntoPosition<T> {
    fn try_into_position(self) -> Option<Position<T>>;
}

pub trait UncheckedIntoPosition<T> {
    fn unchecked_into_position(self) -> Position<T>;
}

impl<T, U: Into<T>> IntoPosition<T> for Position<U> {
    fn into_position(self) -> Position<T> {
        Position::<T>(self.0.into(), self.1.into())
    }
}

impl<T, U: TryInto<T>> TryIntoPosition<T> for Position<U> {
    fn try_into_position(self) -> Option<Position<T>> {
        Some(Position::<T>(
            self.0.try_into().ok()?,
            self.1.try_into().ok()?,
        ))
    }
}

impl<T: Copy + 'static, U: AsPrimitive<T>> UncheckedIntoPosition<T> for Position<U> {
    fn unchecked_into_position(self) -> Position<T> {
        Position::<T>(self.0.as_(), self.1.as_())
    }
}

impl<T, U: TryInto<T>> TryIntoPosition<T> for (U, U) {
    fn try_into_position(self) -> Option<Position<T>> {
        Some(Position::<T>(
            self.0.try_into().ok()?,
            self.1.try_into().ok()?,
        ))
    }
}

impl<T: Copy + 'static, U: AsPrimitive<T>> UncheckedIntoPosition<T> for (U, U) {
    fn unchecked_into_position(self) -> Position<T> {
        Position::<T>(self.0.as_(), self.1.as_())
    }
}

impl<T: Add<Output = T> + Copy> Add for Position<T> {
    type Output = Position<T>;

    fn add(self, rhs: Self) -> Self::Output {
        Position(self.0 + rhs.0, self.1 + rhs.1)
    }
}

impl<T> From<(T, T)> for Position<T> {
    fn from(t: (T, T)) -> Self {
        Self(t.0, t.1)
    }
}

pub type PixelPosition = Position<usize>;
pub type CanvasPosition = Position<i32>;
pub type DrawPosition = Position<i32>;
pub type LayerPosition = Position<i32>;
pub type ChunkPosition = Position<i32>;

impl CanvasPosition {
    /// Translate a canvas position by some portion of an offset.
    pub fn translate_scaled(&self, offset: CanvasPosition, divisor: i32) -> CanvasPosition {
        self.translate((offset.0 / divisor, offset.1 / divisor).into())
    }

    /// The chunk containing a canvas position.
    pub fn containing_chunk(&self, chunk_size: usize) -> ChunkPosition {
        (
            self.0.div_floor(chunk_size as i32),
            self.1.div_floor(chunk_size as i32),
        )
            .into()
    }

    /// Where the `CanvasPosition` relative to the containing chunk.
    pub fn position_in_containing_chunk(&self, chunk_size: usize) -> PixelPosition {
        let containing_chunk = self.containing_chunk(chunk_size);
        PixelPosition::from((
            (self.0 - containing_chunk.0 * chunk_size as i32) as usize,
            (self.1 - containing_chunk.1 * chunk_size as i32) as usize,
        ))
    }
}

impl ChunkPosition {
    /// Get the dimension of chunks spanned between this position and another chunk position.
    pub fn span(&self, other: ChunkPosition) -> Dimensions {
        Dimensions {
            width: self.0.abs_diff(other.0) as usize + 1,
            height: self.1.abs_diff(other.1) as usize + 1,
        }
    }
}
