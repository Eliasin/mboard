#![feature(int_roundings)]

use raster::{chunks::RasterProduct, shapes::Circle, shapes::RasterPolygon};
use wasm_bindgen::prelude::*;

pub mod canvas;
pub mod raster;

// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    Ok(())
}

#[wasm_bindgen]
pub fn get_circle() -> RasterProduct {
    let circle = Circle::new(5.0);

    circle.rasterize().into()
}

#[wasm_bindgen]
pub fn get_circle_pixels() -> Vec<u32> {
    let circle = Circle::new(5.0);

    RasterProduct::from(circle.rasterize()).pixels
}
