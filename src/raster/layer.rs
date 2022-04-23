use super::{
    chunks::{RasterChunk, RasterWindow},
    pixels::Pixel,
    position::{DrawPosition, PixelPosition},
};
use crate::canvas::{Camera, CanvasRect};
use std::{collections::HashMap, convert::TryInto, ops::Rem};

/// A layer made of raw pixel data. All layers will eventually
/// be composited onto a raster layer for presentation.
pub struct RasterLayer {
    chunk_size: usize,
    chunks: HashMap<(i64, i64), RasterChunk>,
}

impl RasterLayer {
    pub fn new(chunk_size: usize) -> RasterLayer {
        RasterLayer {
            chunk_size,
            chunks: HashMap::new(),
        }
    }
}

/// An editing action that can be applied to a raster canvas.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RasterCanvasAction {
    FillRect(CanvasRect, Pixel),
}

/// A rectangle in chunk-space, also denotes where it starts
/// and ends in the top-left and bottom-right chunks.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct ChunkRect {
    top_left_chunk: (i64, i64),
    chunk_width: u64,
    chunk_height: u64,
    top_left_in_chunk: PixelPosition,
    bottom_right_in_chunk: PixelPosition,
}

impl RasterLayer {
    fn find_chunk_rect_in_camera_view(&self, camera: &Camera) -> ChunkRect {
        let origin = (
            camera.top_left.0 + TryInto::<i64>::try_into(camera.width / 2).unwrap(),
            camera.top_left.1 + TryInto::<i64>::try_into(camera.height / 2).unwrap(),
        );

        let scaled_width = (camera.width as f32 * camera.scale()) as i64;
        let scaled_height = (camera.height as f32 * camera.scale()) as i64;
        let top_left_scaled = (origin.0 - scaled_width / 2, origin.1 - scaled_height / 2);

        let chunk_size: i64 = self.chunk_size.try_into().unwrap();

        let top_left_chunk = (
            top_left_scaled.0.div_floor(chunk_size),
            top_left_scaled.1.div_floor(chunk_size),
        );

        let top_left_in_chunk: (i64, i64) = (
            top_left_scaled.0 - top_left_chunk.0 * chunk_size,
            top_left_scaled.1 - top_left_chunk.1 * chunk_size,
        );

        let bottom_right_in_chunk: (i64, i64) = (
            (top_left_in_chunk.0 + scaled_width).rem(chunk_size),
            (top_left_in_chunk.1 + scaled_height).rem(chunk_size),
        );

        let top_left_in_chunk: PixelPosition = PixelPosition::from((
            top_left_in_chunk.0.try_into().unwrap(),
            top_left_in_chunk.1.try_into().unwrap(),
        ));

        let bottom_right_in_chunk: PixelPosition = PixelPosition::from((
            bottom_right_in_chunk.0.try_into().unwrap(),
            bottom_right_in_chunk.1.try_into().unwrap(),
        ));

        let chunk_width: u64 = scaled_width
            .div_ceil(self.chunk_size.try_into().unwrap())
            .try_into()
            .unwrap();
        let chunk_height: u64 = scaled_height
            .div_ceil(self.chunk_size.try_into().unwrap())
            .try_into()
            .unwrap();

        ChunkRect {
            top_left_chunk,
            chunk_height,
            chunk_width,
            top_left_in_chunk,
            bottom_right_in_chunk,
        }
    }

    pub fn rasterize(&mut self, camera: &Camera) -> RasterChunk {
        let chunk_rect = self.find_chunk_rect_in_camera_view(&camera);

        let mut raster_result = RasterChunk::new(camera.width, camera.height);

        for delta_y in 0..chunk_rect.chunk_height {
            for delta_x in 0..chunk_rect.chunk_width {
                let width_rasterized = if delta_x == 0 {
                    self.chunk_size - chunk_rect.top_left_in_chunk.0 .0
                } else if delta_x == chunk_rect.chunk_height - 1 {
                    chunk_rect.bottom_right_in_chunk.0 .0
                } else {
                    self.chunk_size
                };

                let height_rasterized = if delta_y == 0 {
                    self.chunk_size - chunk_rect.top_left_in_chunk.0 .1
                } else if delta_y == chunk_rect.chunk_height - 1 {
                    chunk_rect.bottom_right_in_chunk.0 .1
                } else {
                    self.chunk_size
                };

                let delta_x: i64 = delta_x.try_into().unwrap();
                let delta_y: i64 = delta_y.try_into().unwrap();

                let chunk_position = (
                    chunk_rect.top_left_chunk.0 + delta_x,
                    chunk_rect.top_left_chunk.1 + delta_y,
                );

                let left_in_chunk = if delta_x == 0 {
                    chunk_rect.top_left_in_chunk.0 .0
                } else {
                    0
                };

                let top_in_chunk = if delta_y == 0 {
                    chunk_rect.top_left_in_chunk.0 .1
                } else {
                    0
                };

                let raster_chunk = match self.chunks.get(&chunk_position) {
                    Some(raster_chunk) => raster_chunk,
                    None => {
                        self.chunks.insert(
                            chunk_position,
                            RasterChunk::new(self.chunk_size, self.chunk_size),
                        );
                        self.chunks.get(&chunk_position).unwrap()
                    }
                };

                let top_left_in_chunk = PixelPosition::from((left_in_chunk, top_in_chunk));

                let raster_window = RasterWindow::new(
                    &raster_chunk,
                    top_left_in_chunk,
                    width_rasterized,
                    height_rasterized,
                )
                .unwrap();

                let chunk_size: i64 = self.chunk_size.try_into().unwrap();

                let draw_position_in_result: DrawPosition =
                    DrawPosition::from((delta_x * chunk_size, delta_y * chunk_size));

                raster_result.blit(&raster_window, draw_position_in_result);
            }
        }

        raster_result
    }

    pub fn perform_action(&mut self, action: &RasterCanvasAction) -> CanvasRect {
        todo!()
    }
}

mod tests {
    #[cfg(test)]
    use crate::raster::pixels::colors;

    #[cfg(test)]
    use super::*;

    #[test]
    fn test_chunk_visibility_easy() {
        let raster_layer = RasterLayer::new(1024);

        let mut camera = Camera::new(2000, 2000);
        camera.translate((-500, -500));

        assert_eq!(
            raster_layer.find_chunk_rect_in_camera_view(&camera),
            ChunkRect {
                top_left_chunk: (-1, -1),
                chunk_width: 2,
                chunk_height: 2,
                top_left_in_chunk: (524, 524).into(),
                bottom_right_in_chunk: (500 - 24, 500 - 24).into(),
            }
        );
    }

    #[test]
    fn test_chunk_visibility_hard() {
        let raster_layer = RasterLayer::new(512);

        let mut camera = Camera::new(2000, 1000);
        camera.translate((-500, -1000));

        assert_eq!(
            raster_layer.find_chunk_rect_in_camera_view(&camera),
            ChunkRect {
                top_left_chunk: (-1, -2),
                chunk_width: 4,
                chunk_height: 2,
                top_left_in_chunk: (12, 24).into(),
                bottom_right_in_chunk: (512 - 36, 0).into(),
            }
        );
    }

    #[test]
    fn test_rasterization_easy() {
        let mut raster_layer = RasterLayer::new(10);

        let red_chunk = RasterChunk::new_fill(colors::red(), 10, 10);
        let green_chunk = RasterChunk::new_fill(colors::green(), 10, 10);

        raster_layer.chunks.insert((0, 0), red_chunk.clone());
        raster_layer.chunks.insert((1, 0), green_chunk.clone());

        let camera = Camera::new(15, 10);

        let mut expected_result = RasterChunk::new(15, 10);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((0, 0)));
        expected_result.blit(&green_chunk.as_window(), DrawPosition::from((10, 0)));

        assert_eq!(raster_layer.rasterize(&camera), expected_result);
    }

    #[test]
    fn test_rasterization_hard() {
        let mut raster_layer = RasterLayer::new(100);

        let red_chunk = RasterChunk::new_fill(colors::red(), 100, 100);
        let green_chunk = RasterChunk::new_fill(colors::green(), 100, 100);

        raster_layer.chunks.insert((0, 0), red_chunk.clone());
        raster_layer.chunks.insert((-1, -1), green_chunk.clone());

        let mut camera = Camera::new(150, 200);
        camera.translate((-275, -115));

        let mut expected_result = RasterChunk::new(150, 200);

        expected_result.blit(&red_chunk.as_window(), DrawPosition::from((250, 100)));
        expected_result.blit(
            &green_chunk.as_window(),
            DrawPosition::from((100 - 275, 100 - 115)),
        );

        assert!(raster_layer.rasterize(&camera) == expected_result);
    }
}
