// Serves ../public the way the Worker does, to look at a build before it goes live:
// /help finds help.html, and /download/ files come from the live site.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const root = new URL("../../public/", import.meta.url).pathname.replace(/^\/(\w:)/, "$1");
const port = Number(process.argv[2] ?? 4321);
const types = { ".html": "text/html; charset=utf-8", ".css": "text/css", ".js": "text/javascript", ".json": "application/json", ".png": "image/png", ".webp": "image/webp", ".woff2": "font/woff2", ".sh": "text/plain; charset=utf-8", ".txt": "text/plain; charset=utf-8", ".svg": "image/svg+xml" };

createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (path.startsWith("/download/")) {
    res.writeHead(302, { location: `https://needle.nnx.fyi${path}` });
    return res.end();
  }
  for (const candidate of [path, `${path}.html`, join(path, "index.html")]) {
    const file = normalize(join(root, candidate));
    if (!file.startsWith(normalize(root))) break;
    try {
      if (!(await stat(file)).isFile()) continue;
      res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream", "cache-control": "no-store" });
      return res.end(await readFile(file));
    } catch {}
  }
  res.writeHead(404, { "content-type": "text/plain" });
  res.end("Not found");
}).listen(port, () => console.log(`http://localhost:${port}`));
