use std::ops::Neg;

use num::{cast::AsPrimitive, PrimInt, Signed};

use super::{
    dimensions::Dimensions,
    position::{Position, UncheckedIntoPosition},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Rect<T> {
    pub top_left: Position<T>,
    pub dimensions: Dimensions,
}

impl<T: PrimInt + 'static> Rect<T>
where
    usize: AsPrimitive<T>,
{
    /// The bottom right of a canvas rect.
    pub fn bottom_right(&self) -> Position<T> {
        self.top_left.translate(
            (
                (self.dimensions.width - 1).as_(),
                (self.dimensions.height - 1).as_(),
            )
                .into(),
        )
    }
}

impl<T: PrimInt + AsPrimitive<usize>> Rect<T>
where
    usize: AsPrimitive<T>,
{
    pub fn is_degenerate(&self) -> bool {
        self.dimensions.is_degenerate()
    }

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
                width: (bottom_right.0 - top_left.0).as_() + 1,
                height: (bottom_right.1 - top_left.1).as_() + 1,
            },
        }
    }

    pub fn at_origin(dimensions: Dimensions) -> Rect<T> {
        Rect {
            top_left: (T::zero(), T::zero()).into(),
            dimensions,
        }
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
                width: (right - left).as_() + 1,
                height: (bottom - top).as_() + 1,
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

impl Rect<i32> {
    pub fn subrect_contained_in(&self, dimensions: Dimensions) -> Option<Rect<usize>> {
        let bound_top_left = dimensions.bound_position(self.top_left.into());
        let bound_bottom_right = dimensions.bound_position(self.bottom_right().into());

        let self_top_left_past_other_bottom_right =
            bound_top_left.delta.0 < 0 || bound_top_left.delta.1 < 0;
        let self_bottom_right_past_other_top_left =
            bound_bottom_right.delta.0 > 0 || bound_bottom_right.delta.1 > 0;

        if self_top_left_past_other_bottom_right || self_bottom_right_past_other_top_left {
            return None;
        }

        let top_left_relative_to_self =
            bound_top_left.position.unchecked_into_position() + self.top_left.mul(-1);

        let bottom_right_relative_to_self =
            bound_bottom_right.position.unchecked_into_position() + self.top_left.mul(-1);

        println!(
            "{:?} {:?} {:?} {:?}",
            bound_top_left,
            bound_bottom_right,
            top_left_relative_to_self,
            bottom_right_relative_to_self
        );

        Some(Rect::<usize>::from_points(
            top_left_relative_to_self.unchecked_into_position(),
            bottom_right_relative_to_self.unchecked_into_position(),
        ))
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
pub type DrawRect = Rect<i32>;
pub type RasterRect = Rect<usize>;
