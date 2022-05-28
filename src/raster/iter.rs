use super::{
    chunks::RasterChunk,
    layer::{ChunkPosition, ChunkRect, ChunkRectPosition},
    RasterLayer,
};
use std::collections::HashMap;

pub type RasterChunkIterator<'a> = GenericRasterChunkIterator<&'a RasterLayer>;
pub type RasterChunkIteratorMut<'a> = GenericRasterChunkIterator<&'a mut RasterLayer>;

pub trait RasterLayerReference {}

impl<'a> RasterLayerReference for &'a RasterLayer {}
impl<'a> RasterLayerReference for &'a mut RasterLayer {}

pub struct GenericRasterChunkIterator<T: RasterLayerReference> {
    pub(super) raster_layer: T,
    pub(super) chunk_rect: ChunkRect,
    pub(super) delta: (usize, usize),
}

impl<'a> Iterator for GenericRasterChunkIterator<&'a RasterLayer> {
    type Item = (Option<&'a RasterChunk>, ChunkRectPosition);

    fn next(&mut self) -> Option<Self::Item> {
        let chunk_rect = self.chunk_rect;
        let chunk_size = self.raster_layer.chunk_size;
        let chunks = &self.raster_layer.chunks;

        if self.delta.0 >= chunk_rect.chunk_dimensions.width {
            self.delta.0 = 0;
            self.delta.1 += 1;
        }

        if self.delta.1 >= chunk_rect.chunk_dimensions.height {
            return None;
        }

        let (x_offset, y_offset) = self.delta;

        let width = if chunk_rect.chunk_dimensions.width == 1 {
            chunk_rect.bottom_right_in_chunk.0 .0 - chunk_rect.top_left_in_chunk.0 .0 + 1
        } else if x_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.0 .0
        } else if x_offset == chunk_rect.chunk_dimensions.width - 1 {
            chunk_rect.bottom_right_in_chunk.0 .0 + 1
        } else {
            chunk_size
        };

        let height = if chunk_rect.chunk_dimensions.height == 1 {
            chunk_rect.bottom_right_in_chunk.0 .1 - chunk_rect.top_left_in_chunk.0 .1 + 1
        } else if y_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.0 .1
        } else if y_offset == chunk_rect.chunk_dimensions.height - 1 {
            chunk_rect.bottom_right_in_chunk.0 .1 + 1
        } else {
            chunk_size
        };

        let x_pixel_offset: usize = if x_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.0 .0 + (chunk_size * (x_offset - 1))
        };

        let y_pixel_offset: usize = if y_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.0 .1 + (chunk_size * (y_offset - 1))
        };

        let chunk_position = chunk_rect
            .top_left_chunk
            .translate((x_offset as i64, y_offset as i64));

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
    type Item = (Option<&'a mut RasterChunk>, ChunkRectPosition);

    fn next<'b>(&'b mut self) -> Option<Self::Item> {
        let chunk_rect = self.chunk_rect;
        let chunk_size = self.raster_layer.chunk_size;
        let chunks = unsafe {
            std::mem::transmute::<
                &'b mut HashMap<ChunkPosition, RasterChunk>,
                &'a mut HashMap<ChunkPosition, RasterChunk>,
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
            chunk_rect.bottom_right_in_chunk.0 .0 - chunk_rect.top_left_in_chunk.0 .0 + 1
        } else if x_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.0 .0
        } else if x_offset == chunk_rect.chunk_dimensions.width - 1 {
            chunk_rect.bottom_right_in_chunk.0 .0 + 1
        } else {
            chunk_size
        };

        let height = if chunk_rect.chunk_dimensions.height == 1 {
            chunk_rect.bottom_right_in_chunk.0 .1 - chunk_rect.top_left_in_chunk.0 .1 + 1
        } else if y_offset == 0 {
            chunk_size - chunk_rect.top_left_in_chunk.0 .1
        } else if y_offset == chunk_rect.chunk_dimensions.height - 1 {
            chunk_rect.bottom_right_in_chunk.0 .1 + 1
        } else {
            chunk_size
        };

        let x_pixel_offset: usize = if x_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.0 .0 + (chunk_size * (x_offset - 1))
        };

        let y_pixel_offset: usize = if y_offset == 0 {
            0
        } else {
            chunk_size - chunk_rect.top_left_in_chunk.0 .1 + (chunk_size * (y_offset - 1))
        };

        let chunk_position = chunk_rect
            .top_left_chunk
            .translate((x_offset as i64, y_offset as i64));

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
