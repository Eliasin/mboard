use lru::LruCache;

use crate::{
    primitives::{
        dimensions::{Dimensions, Scale},
        position::{DrawPosition, UncheckedIntoPosition},
    },
    raster::chunks::{
        nn_map::NearestNeighbourMap, raster_chunk::RcRasterChunk, BoxRasterChunk, RasterWindow,
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

impl CanvasViewRasterCache {
    fn prerender_view_area<R>(
        view: &CanvasView,
        nn_map_cache: &mut NearestNeighbourMapCache,
        rasterizer: &mut R,
    ) -> CachedScaledCanvasRaster
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        let requested_canvas_rect = view.canvas_rect();
        let expanded_canvas_rect =
            requested_canvas_rect.expand(requested_canvas_rect.dimensions.largest_dimension());

        let expanded_view = {
            let mut t = *view;
            t.pin_scale(
                Scale::new(
                    expanded_canvas_rect.dimensions.width as f32
                        / view.canvas_dimensions.width as f32,
                    expanded_canvas_rect.dimensions.height as f32
                        / view.canvas_dimensions.height as f32,
                )
                .unwrap_or(Scale {
                    width_factor: 1.0,
                    height_factor: 1.0,
                }),
            );
            t
        };

        let nn_map = nn_map_cache.get_nn_map_for_view(&expanded_view);
        let raster_chunk = rasterizer(&expanded_view.canvas_rect())
            .nn_scaled_with_map(nn_map)
            .expect("nn_map should be fetched with size of expanded view");
        CachedScaledCanvasRaster {
            cached_chunk_position: expanded_view.top_left,
            cached_chunk: raster_chunk.into(),
            canvas_dimensions: expanded_view.canvas_dimensions,
        }
    }

    pub fn rerender_canvas_rect<R>(&mut self, canvas_rect: &CanvasRect, rasterizer: &mut R)
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        if let Some(cached_canvas_raster) = &mut self.cached_raster {
            let cached_view = cached_canvas_raster.view();

            if let Some(view_rect_needing_rerender) =
                cached_view.transform_canvas_rect_to_view(canvas_rect)
            {
                let new_chunk =
                    rasterizer(canvas_rect).nn_scaled(view_rect_needing_rerender.dimensions);
                let draw_position: DrawPosition = view_rect_needing_rerender
                    .top_left
                    .unchecked_into_position();

                match cached_canvas_raster.cached_chunk.get_mut() {
                    Some(mut cached_chunk) => {
                        cached_chunk.blit(&new_chunk.as_window(), draw_position);
                    }
                    None => {
                        cached_canvas_raster.cached_chunk =
                            cached_canvas_raster.cached_chunk.diverge();

                        let mut cached_chunk = cached_canvas_raster.cached_chunk.get_mut().expect(
                            "cached chunk should be initialized above as newly constructed resource",
                        );
                        cached_chunk.blit(&new_chunk.as_window(), draw_position);
                    }
                }
            }
        }
    }

    fn get_chunk_from_cache<'a, R>(
        cached_canvas_raster: &'a mut CachedScaledCanvasRaster,
        nn_map_cache: &mut NearestNeighbourMapCache,
        view: &CanvasView,
        rasterizer: &mut R,
    ) -> RasterWindow<'a>
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        // We don't use an if-let here due to some lifetime issues
        // it causes, primarily, this one https://github.com/rust-lang/rust/issues/54663
        if view.scale_eq(&cached_canvas_raster.view()) && cached_canvas_raster.has_view_cached(view)
        {
            cached_canvas_raster
                .get_window(view)
                .expect("cached view is checked to contain request")
        } else {
            *cached_canvas_raster =
                CanvasViewRasterCache::prerender_view_area(view, nn_map_cache, rasterizer);
            cached_canvas_raster
                .get_window(view)
                .expect("newly rendered view should contain request")
        }
    }

    pub fn get_chunk_or_rasterize<R>(
        &mut self,
        view: &CanvasView,
        rasterizer: &mut R,
    ) -> RasterWindow
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        let cached_canvas_raster = self.cached_raster.get_or_insert_with(|| {
            CanvasViewRasterCache::prerender_view_area(view, &mut self.nn_map_cache, rasterizer)
        });

        CanvasViewRasterCache::get_chunk_from_cache(
            cached_canvas_raster,
            &mut self.nn_map_cache,
            view,
            rasterizer,
        )
    }
}

struct CachedScaledCanvasRaster {
    cached_chunk_position: CanvasPosition,
    canvas_dimensions: Dimensions,
    cached_chunk: RcRasterChunk,
}

impl CachedScaledCanvasRaster {
    pub fn get_window(&self, view: &CanvasView) -> Option<RasterWindow> {
        let cached_view = self.view();

        let requested_rect = cached_view.transform_canvas_rect_to_view(&view.canvas_rect())?;

        RasterWindow::new(
            &self.cached_chunk,
            requested_rect.top_left,
            requested_rect.dimensions.width,
            requested_rect.dimensions.height,
        )
    }

    pub fn has_view_cached(&self, view: &CanvasView) -> bool {
        self.get_window(view).is_some()
    }

    pub fn view(&self) -> CanvasView {
        CanvasView {
            top_left: self.cached_chunk_position,
            view_dimensions: self.cached_chunk.dimensions(),
            canvas_dimensions: self.canvas_dimensions,
        }
    }
}

#[derive(Default)]
pub struct CanvasRectRasterCache(Option<CachedCanvasRaster>);

impl CanvasRectRasterCache {
    fn prerender_canvas_rect_area<R>(
        canvas_rect: &CanvasRect,
        rasterizer: &mut R,
    ) -> CachedCanvasRaster
    where
        R: FnMut(&CanvasRect) -> BoxRasterChunk,
    {
        let expanded_canvas_rect = canvas_rect.expand(canvas_rect.dimensions.largest_dimension());
        let raster_chunk = rasterizer(&expanded_canvas_rect);
        CachedCanvasRaster {
            cached_chunk_position: expanded_canvas_rect.top_left,
            cached_chunk: raster_chunk,
        }
    }

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
                let draw_position: DrawPosition = rect_offset.unchecked_into_position();

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
            cached_canvas_raster
                .get_window(canvas_rect)
                .expect("cached canvas rect has been checked to contain request")
        } else {
            *cached_canvas_raster =
                CanvasRectRasterCache::prerender_canvas_rect_area(canvas_rect, rasterizer);

            cached_canvas_raster
                .get_window(canvas_rect)
                .expect("newly rendered canvas rect should contain request")
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
            CanvasRectRasterCache::prerender_canvas_rect_area(canvas_rect, rasterizer)
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
                .expect("raster window is checked to contain canvas_rect")
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

    use super::{CachedCanvasRaster, CanvasRectRasterCache, CanvasViewRasterCache};
    use crate::{
        assert_raster_eq,
        canvas::{CanvasRect, CanvasView},
        primitives::{
            dimensions::Dimensions,
            position::UncheckedIntoPosition,
            rect::{DrawRect, RasterRect},
        },
        raster::{chunks::BoxRasterChunk, pixels::colors, source::Subsource},
    };

    fn rasterizer_from_chunk(
        raster_chunk: &BoxRasterChunk,
    ) -> impl Fn(&CanvasRect) -> BoxRasterChunk + '_ {
        |rect: &CanvasRect| {
            let position = (rect.top_left.0, rect.top_left.1).unchecked_into_position();

            raster_chunk
                .subsource_at(RasterRect {
                    top_left: position,
                    dimensions: rect.dimensions,
                })
                .unwrap()
        }
    }

    #[test]
    fn canvas_rect_rasterization_cache_caches_renders() {
        let mut cache = CanvasRectRasterCache::default();

        let render_chunk = BoxRasterChunk::new_fill(colors::green(), 512, 512);

        let canvas_rect = CanvasRect {
            top_left: (256, 256).into(),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };

        let mut rasterizer = rasterizer_from_chunk(&render_chunk);

        cache
            .get_chunk_or_rasterize(&canvas_rect, &mut rasterizer)
            .to_chunk();

        let expected_cached_chunk = BoxRasterChunk::new_fill(colors::green(), 64 * 3, 64 * 3);

        let cached_canvas_raster = cache.0.unwrap();
        let cached_chunk = cached_canvas_raster.cached_chunk;

        assert_eq!(
            cached_canvas_raster.cached_chunk_position,
            (256 - 64, 256 - 64).into()
        );

        assert_raster_eq!(expected_cached_chunk, cached_chunk);
    }

    #[test]
    fn canvas_rect_rasterization_cache_doesnt_rerender() {
        // Ensure that the cache does not re-render unnecessarily

        let render_chunk = BoxRasterChunk::new_fill(colors::green(), 64, 64);
        let cached_chunk = BoxRasterChunk::new_fill(colors::red(), 64, 64);

        let mut cache = CanvasRectRasterCache(Some(CachedCanvasRaster {
            cached_chunk_position: (0, 0).into(),
            cached_chunk: cached_chunk.clone(),
        }));

        let canvas_rect = CanvasRect {
            top_left: (0, 0).into(),
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
        };

        let mut rasterizer = rasterizer_from_chunk(&render_chunk);

        let cache_result = cache
            .get_chunk_or_rasterize(&canvas_rect, &mut rasterizer)
            .to_chunk();

        assert_raster_eq!(cache_result, cached_chunk);
    }

    #[test]
    fn canvas_view_raster_cache() {
        let mut canvas_view_raster_cache = CanvasViewRasterCache::default();
        let render_chunk = {
            let mut render_chunk = BoxRasterChunk::new(100, 100);
            render_chunk.fill_rect(
                colors::red(),
                DrawRect {
                    top_left: (30, 30).into(),
                    dimensions: Dimensions {
                        width: 40,
                        height: 40,
                    },
                },
            );

            render_chunk
        };

        let mut rasterizer = rasterizer_from_chunk(&render_chunk);

        {
            let canvas_view = CanvasView {
                top_left: (20, 20).into(),
                view_dimensions: Dimensions {
                    width: 10,
                    height: 10,
                },
                canvas_dimensions: Dimensions {
                    width: 20,
                    height: 20,
                },
            };

            let cached_chunk = canvas_view_raster_cache
                .get_chunk_or_rasterize(&canvas_view, &mut rasterizer)
                .to_chunk();

            let expected_chunk = {
                let mut expected_chunk = BoxRasterChunk::new(10, 10);

                expected_chunk.fill_rect(
                    colors::red(),
                    DrawRect {
                        top_left: (5, 5).into(),
                        dimensions: Dimensions {
                            width: 5,
                            height: 5,
                        },
                    },
                );

                expected_chunk
            };

            assert_raster_eq!(cached_chunk, expected_chunk);
        }
        {
            let canvas_view = CanvasView {
                top_left: (20, 30).into(),
                view_dimensions: Dimensions {
                    width: 5,
                    height: 5,
                },
                canvas_dimensions: Dimensions {
                    width: 20,
                    height: 20,
                },
            };

            let cached_chunk = canvas_view_raster_cache
                .get_chunk_or_rasterize(&canvas_view, &mut rasterizer)
                .to_chunk();

            let expected_chunk = {
                let mut expected_chunk = BoxRasterChunk::new(5, 5);

                expected_chunk.fill_rect(
                    colors::red(),
                    DrawRect {
                        top_left: (3, 0).into(),
                        dimensions: Dimensions {
                            width: 2,
                            height: 5,
                        },
                    },
                );

                expected_chunk
            };

            assert_raster_eq!(cached_chunk, expected_chunk);
        }
    }
}
