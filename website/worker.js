// needle.nnx.fyi: the site's files, downloads served from R2 (they are too big for static
// assets), crash reports from Needle, kept in R2 for 12 months (a bucket lifecycle rule), and
// the short-lived pass Needle needs to talk to Apple Music.
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/download/")) {
      const name = decodeURIComponent(url.pathname.slice("/download/".length));
      if (!/^[A-Za-z0-9._-]+$/.test(name)) return new Response("Not found", { status: 404 });
      // Earlier 1.6.2 links pointed at unsigned APKs, which Android cannot install.
      if (/^Needle-1\.6\.2-android(?:-(?:arm64|x86_64))?-release-unsigned\.apk$/.test(name)) {
        const signedName = name.replace("-unsigned.apk", ".apk");
        return new Response(null, {
          status: 302,
          headers: { location: `/download/${signedName}`, "cache-control": "no-store" },
        });
      }
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
    if (url.pathname === "/api/apple-music-token") return appleMusicToken(request, env);
    // The Mac's What's new links here.
    if (url.pathname === "/changelog") {
      return new Response(null, {
        status: 302,
        headers: { location: "https://github.com/nnxdot/needle/releases", "cache-control": "no-store" },
      });
    }
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

// Apple Music: Needle's apps need a "developer token" (a signed pass) to ask Apple Music for a
// listener's library and to start Apple's player. The key that signs it must never be inside
// an app, so it lives here as a secret (MUSICKIT_KEY, with MUSICKIT_KEY_ID and
// MUSICKIT_TEAM_ID) and the apps ask for a pass. A pass lives 12 hours; one is kept in memory
// and given out until it has under 6 hours left, so every answer is good for at least 6.
// Nothing about the asker is kept: no IP address, no headers.
const PASS_SECONDS = 12 * 60 * 60;
let kept = null; // { token, expires }

async function appleMusicToken(request, env) {
  const headers = { "content-type": "application/json", "cache-control": "no-store" };
  if (request.method !== "GET") return new Response('{"error":"Use GET"}', { status: 405, headers });
  if (!env.MUSICKIT_KEY || !env.MUSICKIT_KEY_ID || !env.MUSICKIT_TEAM_ID) {
    return new Response('{"error":"Apple Music is not set up"}', { status: 503, headers });
  }
  const now = Math.floor(Date.now() / 1000);
  if (!kept || kept.expires - now < PASS_SECONDS / 2) {
    try {
      const expires = now + PASS_SECONDS;
      const token = await signPass(env.MUSICKIT_KEY, env.MUSICKIT_KEY_ID, env.MUSICKIT_TEAM_ID, now, expires);
      kept = { token, expires };
    } catch {
      return new Response('{"error":"Apple Music is not set up"}', { status: 503, headers });
    }
  }
  return new Response(JSON.stringify(kept), { headers });
}

const base64url = (bytes) =>
  btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

// An ES256 JWT, as Apple's "Generating developer tokens" asks: the key id in the header, the
// team id as the issuer. `origin` names the address of Needle's own player page.
export async function signPass(pem, keyId, teamId, issuedAt, expires) {
  const der = Uint8Array.from(atob(pem.replace(/-----[^-]+-----/g, "").replace(/\s+/g, "")), (c) => c.charCodeAt(0));
  const key = await crypto.subtle.importKey("pkcs8", der, { name: "ECDSA", namedCurve: "P-256" }, false, ["sign"]);
  const text = (value) => base64url(new TextEncoder().encode(JSON.stringify(value)));
  const signing = `${text({ alg: "ES256", kid: keyId })}.${text({ iss: teamId, iat: issuedAt, exp: expires, origin: ["https://needle.localhost"] })}`;
  const signature = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, key, new TextEncoder().encode(signing));
  return `${signing}.${base64url(signature)}`;
}
