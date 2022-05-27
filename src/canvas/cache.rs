use lru::LruCache;

use crate::raster::{
    chunks::{RasterChunk, RasterWindow},
    shapes::{Oval, RasterPolygon},
};

use super::CanvasRect;

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

pub struct CanvasCacheNeedsPartialRender<'a> {
    canvas_rasterization_cache: &'a mut CanvasRasterizationCache,
    requested_render_rect: CanvasRect,
}

impl<'a> CanvasCacheNeedsPartialRender<'a> {
    fn get_rects_need_rendering(&self) -> Vec<CanvasRect> {
        todo!()
    }

    fn update_canvas_rect_in_cache<R>(&mut self, canvas_rect: CanvasRect, rasterizer: &mut R)
    where
        R: FnMut(CanvasRect) -> RasterChunk,
    {
        todo!()
    }

    pub fn resolve_with_rasterizer<R>(mut self, rasterizer: &mut R) -> RasterWindow
    where
        R: FnMut(CanvasRect) -> RasterChunk,
    {
        for canvas_rect in self.get_rects_need_rendering() {
            self.update_canvas_rect_in_cache(canvas_rect, rasterizer)
        }

        todo!();
    }
}

pub enum CanvasRasterizationCacheResult<'a> {
    Cached(RasterWindow<'a>),
    NeedsPartialRender(CanvasCacheNeedsPartialRender<'a>),
}

pub struct CanvasRasterizationCache {
    cached_rect: CanvasRect,
    cached_chunk: RasterChunk,
}

impl CanvasRasterizationCache {
    pub fn new(cached_rect: CanvasRect, cached_chunk: RasterChunk) -> CanvasRasterizationCache {
        CanvasRasterizationCache {
            cached_rect,
            cached_chunk,
        }
    }

    pub fn get_chunk(&mut self, canvas_rect: &CanvasRect) -> CanvasRasterizationCacheResult {
        if let Some(canvas_rect_offset_from_cached) =
            self.cached_rect.contains_with_offset(canvas_rect)
        {
            CanvasRasterizationCacheResult::Cached(
                RasterWindow::new(
                    &self.cached_chunk,
                    canvas_rect_offset_from_cached,
                    canvas_rect.dimensions.width,
                    canvas_rect.dimensions.height,
                )
                .unwrap(),
            )
        } else {
            CanvasRasterizationCacheResult::NeedsPartialRender(CanvasCacheNeedsPartialRender {
                canvas_rasterization_cache: self,
                requested_render_rect: *canvas_rect,
            })
        }
    }
}
