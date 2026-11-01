import { buildSync } from "esbuild";

buildSync({
  entryPoints: ["src/game.js"],
  bundle: true,
  minify: true,
  treeShaking: true,
  outfile: "dist/bundle.js",
});
