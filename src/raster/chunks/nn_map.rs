use bumpalo::Bump;
use std::{mem::MaybeUninit, ops::DerefMut};
use thiserror::Error;

use crate::{primitives::dimensions::Dimensions, raster::Pixel};

use super::{
    raster_chunk::{BumpRasterChunk, RasterChunk},
    translate_rect_position_to_flat_index,
};

#[derive(Error, Debug)]
pub enum InvalidScaleError {
    #[error(
        "source dimensions {dimensions_given:?} \
             given do not match dimensions of \
             'NearestNeighbourMap' {expected:?}"
    )]
    InvalidSourceDimensions {
        dimensions_given: Dimensions,
        expected: Dimensions,
    },
    #[error(
        "destination dimensions {dimensions_given:?} \
             given do not match dimensions of \
             'NearestNeighbourMap' {expected:?}"
    )]
    InvalidDestinationDimensions {
        dimensions_given: Dimensions,
        expected: Dimensions,
    },
}

/// A mapping from source pixels to destination pixels for the
/// nearest neighbour resampling algorithm.
pub struct NearestNeighbourMap {
    source_dimensions: Dimensions,
    destination_dimensions: Dimensions,
    map: Box<[usize]>,
}

impl NearestNeighbourMap {
    pub fn new(
        source_dimensions: Dimensions,
        destination_dimensions: Dimensions,
    ) -> NearestNeighbourMap {
        let mut index_mappings =
            Vec::with_capacity(destination_dimensions.width * destination_dimensions.height);

        for row in 0..destination_dimensions.height {
            for column in 0..destination_dimensions.width {
                let nearest =
                    source_dimensions.transform_point((column, row).into(), destination_dimensions);

                let source_index = translate_rect_position_to_flat_index(
                    nearest.into(),
                    source_dimensions.width,
                    source_dimensions.height,
                )
                .expect("transformation should provide position bounded inside source");
                index_mappings.push(source_index);
            }
        }

        NearestNeighbourMap {
            source_dimensions,
            destination_dimensions,
            map: index_mappings.into_boxed_slice(),
        }
    }

    pub fn scale_using_map<S: DerefMut<Target = [Pixel]>, D: DerefMut<Target = [Pixel]>>(
        &self,
        source_chunk: &RasterChunk<S>,
        destination_chunk: &mut RasterChunk<D>,
    ) -> Result<(), InvalidScaleError> {
        if source_chunk.dimensions() != self.source_dimensions {
            return Err(InvalidScaleError::InvalidSourceDimensions {
                dimensions_given: source_chunk.dimensions(),
                expected: self.source_dimensions,
            });
        } else if destination_chunk.dimensions() != self.destination_dimensions {
            return Err(InvalidScaleError::InvalidDestinationDimensions {
                dimensions_given: destination_chunk.dimensions(),
                expected: self.source_dimensions,
            });
        }

        for row in 0..self.destination_dimensions.height {
            for column in 0..self.destination_dimensions.width {
                let destination_index = translate_rect_position_to_flat_index(
                    (column, row),
                    self.destination_dimensions.width,
                    self.destination_dimensions.height,
                )
                .expect("position is bounded");
                let source_index = self.map[destination_index];
                destination_chunk.pixels[destination_index] = source_chunk.pixels[source_index];
            }
        }

        Ok(())
    }

    pub fn scale_using_map_into_bump<'bump, S: DerefMut<Target = [Pixel]>>(
        &self,
        source_chunk: &RasterChunk<S>,
        bump: &'bump Bump,
    ) -> Result<BumpRasterChunk<'bump>, InvalidScaleError> {
        if source_chunk.dimensions() != self.source_dimensions {
            return Err(InvalidScaleError::InvalidSourceDimensions {
                dimensions_given: source_chunk.dimensions(),
                expected: self.source_dimensions,
            });
        }

        let chunk_pixels: &'bump mut [MaybeUninit<Pixel>] = bump.alloc_slice_fill_copy(
            self.destination_dimensions.width * self.destination_dimensions.height,
            MaybeUninit::uninit(),
        );

        for row in 0..self.destination_dimensions.height {
            for column in 0..self.destination_dimensions.width {
                let destination_index = translate_rect_position_to_flat_index(
                    (column, row),
                    self.destination_dimensions.width,
                    self.destination_dimensions.height,
                )
                .expect("position is bounded");
                let source_index = self.map[destination_index];

                chunk_pixels[destination_index].write(source_chunk.pixels[source_index]);
            }
        }

        // Technically we could transmute `chunk_pixels` into `bumpalo::boxed::Box` because
        // of how it's `#[repr(transparent)]` but the documentation reccomends doing
        // it this way instead
        let chunk_pixels = unsafe {
            let initialized_pixels = std::mem::transmute::<_, &'bump mut [Pixel]>(chunk_pixels);
            bumpalo::boxed::Box::from_raw(initialized_pixels)
        };

        Ok(BumpRasterChunk {
            pixels: chunk_pixels,
            dimensions: self.destination_dimensions,
        })
    }

    pub fn destination_dimensions(&self) -> Dimensions {
        self.destination_dimensions
    }

    pub fn source_dimensions(&self) -> Dimensions {
        self.source_dimensions
    }
}

#[cfg(test)]
mod test {
    use crate::{
        assert_raster_eq,
        primitives::dimensions::Dimensions,
        raster::{chunks::BoxRasterChunk, Pixel},
    };

    use super::NearestNeighbourMap;

    #[test]
    fn scaling_using_map_is_same_as_without() {
        let gradient_chunk = BoxRasterChunk::new_fill_dynamic(
            &mut |p| Pixel::new_rgb_norm((1.0 + p.1 as f32) / 3.0, 0.0, (1.0 + p.0 as f32) / 3.0),
            3,
            3,
        );

        let source_dimensions = Dimensions {
            width: 3,
            height: 3,
        };

        let new_dimensions = Dimensions {
            width: 6,
            height: 6,
        };

        let nn_map = NearestNeighbourMap::new(source_dimensions, new_dimensions);

        let mut scaled = gradient_chunk.clone();
        scaled.nn_scale(new_dimensions);

        let expected_scaled = gradient_chunk.clone();
        let expected_scaled = expected_scaled.nn_scaled_with_map(&nn_map).unwrap();

        assert_raster_eq!(scaled, expected_scaled);
    }
}
