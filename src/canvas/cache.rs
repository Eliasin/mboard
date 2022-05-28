use lru::LruCache;

use crate::raster::{
    chunks::{RasterChunk, RasterWindow},
    shapes::{Oval, RasterPolygon},
};

use super::{CanvasPosition, CanvasRect};

pub struct ShapeCache {
    oval_cache: LruCache<Oval, RasterChunk>,
}

impl ShapeCache {
    pub fn new() -> ShapeCache {
        ShapeCache {
            oval_cache: LruCache::new(32),
        }
    }

    pub fn get_oval(&mut self, oval: Oval) -> &RasterChunk {
        self.oval_cache
            .get_or_insert(oval, || oval.rasterize())
            .unwrap()
    }
}

impl Default for ShapeCache {
    fn default() -> Self {
        ShapeCache::new()
    }
}

#[derive(Default)]
pub struct CanvasRasterizationCache(Option<CachedCanvasRaster>);

impl CanvasRasterizationCache {
    fn get_chunk_from_cache<'a, R>(
        cached_canvas_raster: &'a mut CachedCanvasRaster,
        canvas_rect: &CanvasRect,
        rasterizer: &mut R,
    ) -> RasterWindow<'a>
    where
        R: FnMut(&CanvasRect) -> RasterChunk,
    {
        // We don't use an if-let here due to some lifetime issues
        // it causes, primarily, this one https://github.com/rust-lang/rust/issues/54663
        if cached_canvas_raster.has_rect_cached(canvas_rect) {
            cached_canvas_raster.get_window(canvas_rect).unwrap()
        } else {
            // Pre-render surrounding area
            let expanded_canvas_rect =
                canvas_rect.expand(canvas_rect.dimensions.largest_dimension());
            let raster_chunk = rasterizer(canvas_rect);
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
        R: FnMut(&CanvasRect) -> RasterChunk,
    {
        let cached_canvas_raster = self.0.get_or_insert_with(|| {
            // Pre-render surrounding area
            let expanded_canvas_rect =
                canvas_rect.expand(canvas_rect.dimensions.largest_dimension());
            let raster_chunk = rasterizer(canvas_rect);
            CachedCanvasRaster {
                cached_chunk_position: expanded_canvas_rect.top_left,
                cached_chunk: raster_chunk,
            }
        });

        CanvasRasterizationCache::get_chunk_from_cache(
            cached_canvas_raster,
            canvas_rect,
            rasterizer,
        )
    }
}

struct CachedCanvasRaster {
    cached_chunk_position: CanvasPosition,
    cached_chunk: RasterChunk,
}

impl CachedCanvasRaster {
    fn cached_canvas_rect(&self) -> CanvasRect {
        CanvasRect {
            top_left: self.cached_chunk_position,
            dimensions: self.cached_chunk.dimensions(),
        }
    }

    pub fn get_window(&self, canvas_rect: &CanvasRect) -> Option<RasterWindow> {
        if let Some(canvas_rect_offset_from_cached) =
            self.cached_canvas_rect().contains_with_offset(canvas_rect)
        {
            Some(
                RasterWindow::new(
                    &self.cached_chunk,
                    canvas_rect_offset_from_cached,
                    canvas_rect.dimensions.width,
                    canvas_rect.dimensions.height,
                )
                .unwrap(),
            )
        } else {
            None
        }
    }

    pub fn has_rect_cached(&self, canvas_rect: &CanvasRect) -> bool {
        self.get_window(canvas_rect).is_some()
    }
}

mod tests {
    #[test]
    fn test_canvas_rect_rasterization_cache() {}
}
