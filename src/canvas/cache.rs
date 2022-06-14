use lru::LruCache;

use crate::{
    raster::{
        chunks::{
            nn_map::{InvalidScaleError, NearestNeighbourMap},
            BoxRasterChunk, RasterWindow,
        },
        position::{Dimensions, DrawPosition},
    },
    vector::shapes::{Oval, RasterizablePolygon},
};

use super::{CanvasPosition, CanvasRect, CanvasView};

pub struct ShapeCache {
    oval_cache: LruCache<Oval, BoxRasterChunk>,
}

impl ShapeCache {
    pub fn new() -> ShapeCache {
        ShapeCache {
            oval_cache: LruCache::new(32),
        }
    }

    pub fn get_oval(&mut self, oval: Oval) -> &BoxRasterChunk {
        self.oval_cache
            .get_or_insert(oval, || oval.rasterize())
            .expect("this should never happen, as it only occurs with cache size 0")
    }
}

impl Default for ShapeCache {
    fn default() -> Self {
        ShapeCache::new()
    }
}

#[derive(Default)]
pub struct CanvasViewRasterCache {
    cached_raster: Option<CachedScaledCanvasRaster>,
    nn_map_cache: NearestNeighbourMapCache,
}

struct CachedScaledCanvasRaster {
    cached_chunk_position: CanvasPosition,
    view_dimensions: Dimensions,
    cached_chunk: BoxRasterChunk,
}

impl CachedScaledCanvasRaster {
    fn try_new_cached_from_scaling_canvas_raster(
        cached_canvas_raster: CachedCanvasRaster,
        nn_map: NearestNeighbourMap,
    ) -> Result<CachedScaledCanvasRaster, InvalidScaleError> {
        let scaled = cached_canvas_raster
            .cached_chunk
            .nn_scaled_with_map(&nn_map)?;

        Ok(CachedScaledCanvasRaster {
            cached_chunk_position: cached_canvas_raster.cached_chunk_position,
            view_dimensions: nn_map.destination_dimensions(),
            cached_chunk: cached_canvas_raster.cached_chunk,
        })
    }
}

#[derive(Default)]
pub struct CanvasRectRasterCache(Option<CachedCanvasRaster>);

impl CanvasRectRasterCache {
    pub fn rerender_canvas_rect<R>(&mut self, canvas_rect: &CanvasRect, rasterizer: &mut R)
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        if let Some(cached_canvas_raster) = &mut self.0 {
            if let Some(rect_offset) = cached_canvas_raster
                .cached_canvas_rect()
                .contains_with_offset(canvas_rect)
            {
                let new_chunk = rasterizer(canvas_rect);
                let draw_position: DrawPosition = rect_offset.into();

                cached_canvas_raster
                    .cached_chunk
                    .blit(&new_chunk.as_window(), draw_position);
            }
        }
    }

    fn get_chunk_from_cache<'a, R>(
        cached_canvas_raster: &'a mut CachedCanvasRaster,
        canvas_rect: &CanvasRect,
        rasterizer: &mut R,
    ) -> RasterWindow<'a>
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        // We don't use an if-let here due to some lifetime issues
        // it causes, primarily, this one https://github.com/rust-lang/rust/issues/54663
        if cached_canvas_raster.has_rect_cached(canvas_rect) {
            cached_canvas_raster.get_window(canvas_rect).unwrap()
        } else {
            // Pre-render surrounding area
            let expanded_canvas_rect =
                canvas_rect.expand(canvas_rect.dimensions.largest_dimension());
            let raster_chunk = rasterizer(&expanded_canvas_rect);
            *cached_canvas_raster = CachedCanvasRaster {
                cached_chunk_position: expanded_canvas_rect.top_left,
                cached_chunk: raster_chunk,
            };

            cached_canvas_raster.get_window(canvas_rect).unwrap()
        }
    }

    pub fn get_chunk_or_rasterize<R>(
        &mut self,
        canvas_rect: &CanvasRect,
        rasterizer: &mut R,
    ) -> RasterWindow
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        let cached_canvas_raster = self.0.get_or_insert_with(|| {
            // Pre-render surrounding area
            let expanded_canvas_rect =
                canvas_rect.expand(canvas_rect.dimensions.largest_dimension());
            let raster_chunk = rasterizer(&expanded_canvas_rect);
            CachedCanvasRaster {
                cached_chunk_position: expanded_canvas_rect.top_left,
                cached_chunk: raster_chunk,
            }
        });

        CanvasRectRasterCache::get_chunk_from_cache(cached_canvas_raster, canvas_rect, rasterizer)
    }
}

struct CachedCanvasRaster {
    cached_chunk_position: CanvasPosition,
    cached_chunk: BoxRasterChunk,
}

impl CachedCanvasRaster {
    fn cached_canvas_rect(&self) -> CanvasRect {
        CanvasRect {
            top_left: self.cached_chunk_position,
            dimensions: self.cached_chunk.dimensions(),
        }
    }

    pub fn get_window(&self, canvas_rect: &CanvasRect) -> Option<RasterWindow> {
        self.cached_canvas_rect()
            .contains_with_offset(canvas_rect)
            .map(|canvas_rect_offset_from_cached| {
                RasterWindow::new(
                    &self.cached_chunk,
                    canvas_rect_offset_from_cached,
                    canvas_rect.dimensions.width,
                    canvas_rect.dimensions.height,
                )
                .unwrap()
            })
    }

    pub fn has_rect_cached(&self, canvas_rect: &CanvasRect) -> bool {
        self.get_window(canvas_rect).is_some()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct ViewDimensions {
    canvas_dimensions: Dimensions,
    view_dimensions: Dimensions,
}

impl ViewDimensions {
    pub fn from_view(view: &CanvasView) -> ViewDimensions {
        ViewDimensions {
            canvas_dimensions: view.canvas_dimensions,
            view_dimensions: view.view_dimensions,
        }
    }
}

pub struct NearestNeighbourMapCache(LruCache<ViewDimensions, NearestNeighbourMap>);

impl NearestNeighbourMapCache {
    pub fn get_nn_map_for_view(&mut self, view: &CanvasView) -> &NearestNeighbourMap {
        self.0
            .get_or_insert(ViewDimensions::from_view(view), || {
                view.create_nn_map_to_view_dimensions()
            })
            .expect("this should never happen, as it only occurs with cache size 0")
    }
}

impl Default for NearestNeighbourMapCache {
    fn default() -> Self {
        NearestNeighbourMapCache(LruCache::new(128))
    }
}

#[cfg(test)]
mod tests {
    use super::{CachedCanvasRaster, CanvasRectRasterCache};
    use crate::{
        assert_raster_eq,
        canvas::{CanvasPosition, CanvasRect},
        raster::{
            chunks::BoxRasterChunk, pixels::colors, position::Dimensions, position::PixelPosition,
        },
    };

    #[test]
    fn test_canvas_rect_rasterization_cache_caches_renders() {
        let mut cache = CanvasRectRasterCache::default();

        let render_chunk = BoxRasterChunk::new_fill(colors::green(), 512, 512);

        let canvas_rect = CanvasRect {
            top_left: CanvasPosition((256, 256)),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };

        cache
            .get_chunk_or_rasterize(&canvas_rect, &mut |rect: &CanvasRect| -> BoxRasterChunk {
                let position =
                    PixelPosition((rect.top_left.0 .0 as usize, rect.top_left.0 .1 as usize));

                render_chunk.clone_square(position, rect.dimensions.width, rect.dimensions.height)
            })
            .to_chunk();

        let expected_cached_chunk = BoxRasterChunk::new_fill(colors::green(), 64 * 3, 64 * 3);

        let cached_canvas_raster = cache.0.unwrap();
        let cached_chunk = cached_canvas_raster.cached_chunk;

        assert_eq!(
            cached_canvas_raster.cached_chunk_position,
            CanvasPosition((256 - 64, 256 - 64))
        );

        assert_raster_eq!(expected_cached_chunk, cached_chunk);
    }

    #[test]
    fn test_canvas_rect_rasterization_cache_doesnt_rerender() {
        // Ensure that the cache does not re-render unnecessarily

        let render_chunk = BoxRasterChunk::new_fill(colors::green(), 64, 64);
        let cached_chunk = BoxRasterChunk::new_fill(colors::red(), 64, 64);

        let mut cache = CanvasRectRasterCache(Some(CachedCanvasRaster {
            cached_chunk_position: CanvasPosition((0, 0)),
            cached_chunk: cached_chunk.clone(),
        }));

        let canvas_rect = CanvasRect {
            top_left: CanvasPosition((0, 0)),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };

        let cache_result = cache
            .get_chunk_or_rasterize(&canvas_rect, &mut |rect: &CanvasRect| -> BoxRasterChunk {
                let position =
                    PixelPosition((rect.top_left.0 .0 as usize, rect.top_left.0 .1 as usize));

                render_chunk.clone_square(position, rect.dimensions.width, rect.dimensions.height)
            })
            .to_chunk();

        assert_raster_eq!(cache_result, cached_chunk);
    }
}
