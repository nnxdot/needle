# needle.nnx.fyi

The site's source. It builds into `../public`, which Wrangler serves (`wrangler deploy` in `website/`).

```sh
npm install
npm run build   # builds, then copies the pages into ../public
npm run serve   # looks at ../public on http://localhost:4321
```

- `../public/latest.json`, `latest-macos.json`, `latest-android.json`, `install-mac.sh`, and the icons are not made by the build.
  Needle's updaters and the Mac installer read them; edit them there, as before.
- The download buttons take their file names and versions from those two JSON files when the
  site is built, and again in the browser, so a release that only changes `latest.json` still
  shows the right files. Run `npm run build` after a release so sizes and versions are right
  without JavaScript.
- `src/docs/*.html` are the Help, Privacy, Plugins, and Themes pages, unchanged in shape. The
  Android app copies `public/help.html` and `public/privacy.html` in and reads their `<article>`.
- Cover photos in `public/img/covers` are from Unsplash (through picsum.photos), under the
  Unsplash license. The albums and artists are made up. Geist is under the SIL Open Font License.
