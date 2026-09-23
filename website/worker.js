// needle.nnx.fyi: the site's files, downloads served from R2 (they are too big for static
// assets), and crash reports from Needle, kept in R2 for 12 months (a bucket lifecycle rule).
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
    if (url.pathname === "/api/crash") return crash(request, env);
    return env.ASSETS.fetch(request);
  },
};

// A crash report: { version, os, report }. Needle takes out paths and names before sending.
// Nothing else is kept: no IP address, no headers.
async function crash(request, env) {
  if (request.method !== "POST") return new Response("Use POST", { status: 405 });
  const text = await request.text();
  if (text.length > 32 * 1024) return new Response("Too large", { status: 413 });
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    return new Response("Not JSON", { status: 400 });
  }
  const field = (value, limit) => (typeof value === "string" ? value.slice(0, limit) : "");
  const report = {
    version: field(body.version, 32),
    os: field(body.os, 64),
    report: field(body.report, 30 * 1024),
    received: new Date().toISOString(),
  };
  if (!report.version || !report.report) return new Response("Missing fields", { status: 400 });
  const key = `${report.received.slice(0, 10)}/${crypto.randomUUID()}.json`;
  await env.CRASHES.put(key, JSON.stringify(report), {
    httpMetadata: { contentType: "application/json" },
  });
  return new Response(null, { status: 204 });
}
