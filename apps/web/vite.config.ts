import { defineConfig } from "vite";

// Cross-origin isolation headers are required in the dev server too, so
// the same COOP/COEP-dependent code paths (SharedArrayBuffer, future
// multithreaded WASM) behave identically in `vite dev` and behind the
// production stockfly-server.
export default defineConfig({
  server: {
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Resource-Policy": "same-origin",
    },
    fs: {
      // The compiled graph and generated maps are symlinked into
      // public/vendor from outside the app root (see public/vendor/graph,
      // public/vendor/chess-maps); Vite's dev server must be allowed to
      // follow them there.
      allow: [".."],
    },
  },
  build: {
    target: "es2022",
  },
});
