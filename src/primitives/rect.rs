use std::ops::Neg;

use num::{cast::AsPrimitive, PrimInt, Signed};

use super::{dimensions::Dimensions, position::Position};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Rect<T> {
    pub top_left: Position<T>,
    pub dimensions: Dimensions,
}

impl<T: PrimInt + AsPrimitive<usize>> Rect<T>
where
    usize: AsPrimitive<T>,
{
    pub fn translate(&self, offset: Position<T>) -> Rect<T> {
        Rect {
            top_left: self.top_left.translate(offset),
            ..*self
        }
    }

    pub fn from_points(a: Position<T>, b: Position<T>) -> Rect<T> {
        let top_left = (a.0.min(b.0), a.1.min(b.1));
        let bottom_right = (a.0.max(b.0), a.1.max(b.1));

        Rect {
            top_left: top_left.into(),
            dimensions: Dimensions {
                width: (bottom_right.0 - top_left.0).as_(),
                height: (bottom_right.1 - top_left.1).as_(),
            },
        }
    }

    pub fn at_origin(dimensions: Dimensions) -> Rect<T> {
        Rect {
            top_left: (T::zero(), T::zero()).into(),
            dimensions,
        }
    }

    /// The bottom right of a canvas rect.
    pub fn bottom_right(&self) -> Position<T> {
        self.top_left
            .translate((self.dimensions.width.as_(), self.dimensions.height.as_()).into())
    }

    pub fn spanning_rect(&self, other: &Rect<T>) -> Rect<T> {
        let top = self.top_left.1.min(other.top_left.1);
        let left = self.top_left.0.min(other.top_left.0);

        let bottom_right = self.bottom_right();
        let other_bottom_right = other.bottom_right();

        let bottom = bottom_right.1.max(other_bottom_right.1);
        let right = bottom_right.0.max(other_bottom_right.0);

        Rect {
            top_left: (left, top).into(),
            dimensions: Dimensions {
                width: (right - left).as_(),
                height: (bottom - top).as_(),
            },
        }
    }
}

impl<T: PrimInt + AsPrimitive<usize> + Neg<Output = T>> Rect<T>
where
    usize: AsPrimitive<T>,
{
    /// Expands `self` in all directions by `margin`.
    pub fn expand(&self, margin: usize) -> Rect<T> {
        let mut new_rect = *self;
        new_rect.top_left = new_rect
            .top_left
            .translate((-margin.as_(), -margin.as_()).into());
        new_rect.dimensions = Dimensions {
            width: self.dimensions.width + margin * 2,
            height: self.dimensions.height + margin * 2,
        };

        new_rect
    }
}

impl<T: PrimInt + AsPrimitive<usize> + Signed> Rect<T>
where
    usize: AsPrimitive<T>,
{
    /// The offset of a contained rect to this rect.
    pub fn contains_with_offset(&self, other: &Rect<T>) -> Option<Position<usize>> {
        if self.top_left.0 > other.top_left.0 || self.top_left.1 > other.top_left.1 {
            None
        } else {
            let bottom_right = self.bottom_right();
            let other_bottom_right = other.bottom_right();

            if bottom_right.0 < other_bottom_right.0 || bottom_right.1 < other_bottom_right.1 {
                None
            } else {
                Some(
                    (
                        other.top_left.0.abs_sub(&self.top_left.0).as_(),
                        other.top_left.1.abs_sub(&self.top_left.1).as_(),
                    )
                        .into(),
                )
            }
        }
    }
}

impl Rect<usize> {
    /// The offset of a contained rect to this rect.
    pub fn usize_contains_with_offset(&self, other: &Rect<usize>) -> Option<Position<usize>> {
        if self.top_left.0 > other.top_left.0 || self.top_left.1 > other.top_left.1 {
            None
        } else {
            let bottom_right = self.bottom_right();
            let other_bottom_right = other.bottom_right();

            if bottom_right.0 < other_bottom_right.0 || bottom_right.1 < other_bottom_right.1 {
                None
            } else {
                Some(
                    (
                        other.top_left.0.abs_diff(self.top_left.0).as_(),
                        other.top_left.1.abs_diff(self.top_left.1).as_(),
                    )
                        .into(),
                )
            }
        }
    }
}

pub type CanvasRect = Rect<i32>;
pub type ViewRect = Rect<usize>;
