use crate::{
    primitives::position::{DrawPosition, PixelPosition},
    raster::{pixels::colors, Pixel},
};

#[macro_export]
macro_rules! assert_raster_eq {
    ($a:ident, $b:ident) => {
        assert!($a == $b, "\n{}\n{}", $a, $b)
    };
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BoundedIndex {
    pub index: usize,
    pub x_delta: i32,
    pub y_delta: i32,
}

/// A value that can be indexed by `PixelPosition`, providing pixels. It must make sense to get slices representing rows from the value.
pub trait IndexableByPosition {
    /// Returns an index to the backing collection that corresponds to the position supplied.
    fn get_index_from_position(&self, position: PixelPosition) -> Option<usize>;
    /// Returns a bounded index to the backing collection along with the shift applied to bound the
    /// position within the collection.
    fn get_index_from_bounded_position(&self, position: DrawPosition) -> BoundedIndex;
    /// Returns a bit position bounded into the underlying collection.
    fn bound_position(&self, position: DrawPosition) -> PixelPosition;
    /// Returns a slice representing a row of pixels.
    fn get_row_slice(&self, row_num: usize) -> Option<&[Pixel]>;
}

/// Failure to create a `RasterWindow` from a slice due to incompatible sizing.
#[derive(Debug)]
pub struct InvalidPixelSliceSize {
    pub desired_width: usize,
    pub desired_height: usize,
    pub buffer_size: usize,
}

impl std::fmt::Display for InvalidPixelSliceSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot make ({}, {}) from buffer of size {}",
            self.desired_width, self.desired_height, self.buffer_size
        )
    }
}

pub fn translate_rect_position_to_flat_index(
    position: (usize, usize),
    width: usize,
    height: usize,
) -> Option<usize> {
    let offset_from_row = position.1 * width;
    let offset_from_column = position.0;

    let over_width = position.0 >= width;
    let over_height = position.1 >= height;

    if over_width || over_height {
        None
    } else {
        Some(offset_from_row + offset_from_column)
    }
}

pub fn get_color_character_for_pixel(p: &Pixel) -> &'static str {
    let mut color_characters = vec![
        (colors::red(), "r"),
        (colors::blue(), "b"),
        (colors::green(), "g"),
        (colors::black(), "B"),
        (colors::white(), "w"),
        (colors::transparent(), " "),
    ];

    color_characters.sort_by(|(a, _), (b, _)| {
        let d_a = p.eu_distance(a);
        let d_b = p.eu_distance(b);

        d_a.partial_cmp(&d_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    color_characters
        .get(0)
        .expect("color character array should never be empty")
        .1
}

pub fn display_raster_row(row: &[Pixel]) -> String {
    let mut s = String::new();

    for p in row {
        s += get_color_character_for_pixel(p);
    }

    s
}
