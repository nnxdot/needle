// Builds the site into dist/; `npm run build` then copies it into ../public, which Wrangler
// serves. Pages are built as help.html, not help/index.html: the Android app copies
// public/help.html and public/privacy.html in at build time.
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://needle.nnx.fyi",
  build: { format: "file", assets: "_astro" },
  trailingSlash: "never",
  devToolbar: { enabled: false },
});
