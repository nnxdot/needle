// needle.nnx.fyi: the site's files, and downloads served from R2 (they are too big for static assets).
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/download/")) {
      const name = decodeURIComponent(url.pathname.slice("/download/".length));
      if (!/^[A-Za-z0-9._-]+$/.test(name)) return new Response("Not found", { status: 404 });
      const object = await env.DOWNLOADS.get(name);
      if (!object) return new Response("Not found", { status: 404 });
      const headers = new Headers();
      object.writeHttpMetadata(headers);
      headers.set("etag", object.httpEtag);
      headers.set("cache-control", "public, max-age=3600");
      if (!name.endsWith(".txt")) headers.set("content-disposition", `attachment; filename="${name}"`);
      else headers.set("content-type", "text/plain; charset=utf-8");
      return new Response(request.method === "HEAD" ? null : object.body, { headers });
    }
    return env.ASSETS.fetch(request);
  },
};
