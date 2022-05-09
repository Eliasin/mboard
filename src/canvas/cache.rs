use lru::LruCache;

use crate::raster::{
    chunks::RasterChunk,
    shapes::{Oval, RasterPolygon},
};

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
