async function main() {
  let wasm = await import('../pkg/index_bg.wasm');

  wasm.main_js();
  console.log(wasm.get_circle());
  console.log(wasm.get_circle_pixels());
}

main();
