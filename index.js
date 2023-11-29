import("./pkg")
  .then((lib) => {
    lib.greet("WebAssembly!");
  })
  .catch(console.error);
