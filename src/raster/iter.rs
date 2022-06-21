use super::{
    chunks::BoxRasterChunk,
    layer::{ChunkRect, ChunkRectPosition},
    RasterLayer,
};

use crate::primitives::{
    dimensions::Dimensions,
    position::{ChunkPosition, PixelPosition, UncheckedIntoPosition},
};
use std::collections::HashMap;

/// Iterator over individual `PixelPosition`s in a dimension space.
pub struct PixelPositionIterator {
    dimensions: Dimensions,
    current: PixelPosition,
}

impl PixelPositionIterator {
    pub fn new(dimensions: Dimensions) -> PixelPositionIterator {
        PixelPositionIterator {
            dimensions,
            current: PixelPosition::from((0, 0)),
        }
    }
}

impl Iterator for PixelPositionIterator {
    type Item = PixelPosition;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 >= self.dimensions.width {
            self.current.0 = 0;
            self.current.1 += 1;
        }

        if self.current.1 >= self.dimensions.height {
            None
        } else {
            Some(self.current)
        }
    }
}

impl ExactSizeIterator for PixelPositionIterator {}

pub type RasterChunkIterator<'a> = GenericRasterChunkIterator<&'a RasterLayer>;
pub type RasterChunkIteratorMut<'a> = GenericRasterChunkIterator<&'a mut RasterLayer>;

pub trait RasterLayerReference {}

impl<'a> RasterLayerReference for &'a RasterLayer {}
impl<'a> RasterLayerReference for &'a mut RasterLayer {}

pub struct GenericRasterChunkIterator<T: RasterLayerReference> {
    raster_layer: T,
    chunk_rect: ChunkRect,
    delta: (usize, usize),
}

impl<T: RasterLayerReference> GenericRasterChunkIterator<T> {
    pub fn new(raster_layer_reference: T, chunk_rect: ChunkRect) -> Self {
        Self {
            raster_layer: raster_layer_reference,
            chunk_rect,
            delta: (0, 0),
        }
    }
}

impl<'a> Iterator for GenericRasterChunkIterator<&'a RasterLayer> {
    type Item = (Option<&'a BoxRasterChunk>, ChunkRectPosition);

    fn next(&mut self) -> Option<Self::Item> {
        let chunk_rect = self.chunk_rect;
        let chunk_size = self.raster_layer.chunk_size;
        let chunks = &self.raster_layer.chunks;

        if self.delta.0 >= chunk_rect.chunk_dimensions.width {
            self.delta.0 = 0;
            // We must used `checked_add` to ensure that wrapping never occurs,
            // as that would break the invariant that a `delta` value is never
            // repeated for the lifetime of the iterator, causing
            // undefined behvaiour
            self.delta.1 = self.delta.1.checked_add(1).unwrap();
        }

        if self.delta.1 >= chunk_rect.chunk_dimensions.height {
            return None;
        }

        let (x_offset, y_offset) = self.delta;

        let width = if chunk_rect.chunk_dimensions.width == 1 {
            chunk_rect.bottom_right_in_chunk.0 - chunk_rect.top_left_in_chunk.0 + 1
        } else if x_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.0
        } else if x_offset == chunk_rect.chunk_dimensions.width - 1 {
            chunk_rect.bottom_right_in_chunk.0 + 1
        } else {
            chunk_size
        };

        let height = if chunk_rect.chunk_dimensions.height == 1 {
            chunk_rect.bottom_right_in_chunk.1 - chunk_rect.top_left_in_chunk.1 + 1
        } else if y_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.1
        } else if y_offset == chunk_rect.chunk_dimensions.height - 1 {
            chunk_rect.bottom_right_in_chunk.1 + 1
        } else {
            chunk_size
        };

        let x_pixel_offset: usize = if x_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.0 + (chunk_size * (x_offset - 1))
        };

        let y_pixel_offset: usize = if y_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.1 + (chunk_size * (y_offset - 1))
        };

        let chunk_position = chunk_rect
            .top_left_chunk
            .translate((x_offset, y_offset).unchecked_into_position());

        // `unwrap` is ok because chunk_position is constructed to always be within
        // `chunk_rect`.
        let top_left_in_chunk = chunk_rect.top_left_in_chunk(chunk_position).unwrap();

        let raster_chunk = chunks.get(&chunk_position);

        let chunk_rect_position = ChunkRectPosition {
            top_left_in_chunk,
            width,
            height,
            x_chunk_offset: x_offset,
            y_chunk_offset: y_offset,
            x_pixel_offset,
            y_pixel_offset,
        };

        self.delta.0 += 1;

        Some((raster_chunk, chunk_rect_position))
    }
}

impl<'a> Iterator for GenericRasterChunkIterator<&'a mut RasterLayer> {
    type Item = (Option<&'a mut BoxRasterChunk>, ChunkRectPosition);

    fn next<'b>(&'b mut self) -> Option<Self::Item> {
        let chunk_rect = self.chunk_rect;
        let chunk_size = self.raster_layer.chunk_size;

        // This transmute is needed to convince the borrow checker that
        // the lifetime of `chunks` does NOT depend on the lifetime of
        // the iterator at all, but instead the borrow to `raster_layer`.
        // This is sound because chunks is just a field of the `raster_layer`
        // borrow.
        let chunks = unsafe {
            std::mem::transmute::<
                &'b mut HashMap<ChunkPosition, BoxRasterChunk>,
                &'a mut HashMap<ChunkPosition, BoxRasterChunk>,
            >(&mut self.raster_layer.chunks)
        };

        if self.delta.0 >= chunk_rect.chunk_dimensions.width {
            self.delta.0 = 0;
            self.delta.1 += 1;
        }

        if self.delta.1 >= chunk_rect.chunk_dimensions.height {
            return None;
        }

        let (x_offset, y_offset) = self.delta;

        let width = if chunk_rect.chunk_dimensions.width == 1 {
            chunk_rect.bottom_right_in_chunk.0 - chunk_rect.top_left_in_chunk.0 + 1
        } else if x_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.0
        } else if x_offset == chunk_rect.chunk_dimensions.width - 1 {
            chunk_rect.bottom_right_in_chunk.0 + 1
        } else {
            chunk_size
        };

        let height = if chunk_rect.chunk_dimensions.height == 1 {
            chunk_rect.bottom_right_in_chunk.1 - chunk_rect.top_left_in_chunk.1 + 1
        } else if y_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.1
        } else if y_offset == chunk_rect.chunk_dimensions.height - 1 {
            chunk_rect.bottom_right_in_chunk.1 + 1
        } else {
            chunk_size
        };

        let x_pixel_offset: usize = if x_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.0 + (chunk_size * (x_offset - 1))
        };

        let y_pixel_offset: usize = if y_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.1 + (chunk_size * (y_offset - 1))
        };

        let chunk_position = chunk_rect
            .top_left_chunk
            .translate((x_offset, y_offset).unchecked_into_position());

        // `unwrap` is ok because chunk_position is constructed to always be within
        // `chunk_rect`.
        let top_left_in_chunk = chunk_rect.top_left_in_chunk(chunk_position).unwrap();

        let raster_chunk = chunks.get_mut(&chunk_position);

        let chunk_rect_position = ChunkRectPosition {
            top_left_in_chunk,
            width,
            height,
            x_chunk_offset: x_offset,
            y_chunk_offset: y_offset,
            x_pixel_offset,
            y_pixel_offset,
        };

        self.delta.0 += 1;

        Some((raster_chunk, chunk_rect_position))
    }
}
