// The home page's motion and the download chooser. Everything on the page is readable without
// this script; it only adds movement, and keeps the download buttons in step with latest.json.
import { pick } from "../lib/assets.js";

const $ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document) => root.querySelector<T>(sel);
const $$ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document) => [...root.querySelectorAll<T>(sel)];
function reduced() {
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/* ---------- which device is this? */
type Platform = "windows" | "mac" | "linux" | "android" | "iphone";
function detect(): Platform {
  const ua = navigator.userAgent;
  const hint = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? "";
  if (/android/i.test(ua) || /android/i.test(hint)) return "android";
  if (/iphone|ipod/i.test(ua)) return "iphone";
  if (/mac|iphone|ipad|ipod/i.test(ua) || /macos/i.test(hint)) return "mac";
  if (/linux|cros|x11/i.test(ua) || /linux|chrome os/i.test(hint)) return "linux";
  return "windows";
}
const names: Record<Platform, string> = { windows: "Windows", mac: "Mac", linux: "Linux", android: "Android", iphone: "iPhone" };
const platform = detect();
const osLabel = $("[data-os-label]");
if (osLabel) osLabel.textContent = `Download for ${names[platform]}`;

/* ---------- download tabs */
const tabs = $$<HTMLButtonElement>("[data-tab]");
const pill = $("[data-tab-pill]");
function placePill(tab: HTMLElement) {
  if (!pill) return;
  pill.style.width = `${tab.offsetWidth}px`;
  pill.style.transform = `translateX(${tab.offsetLeft}px)`;
}
function choose(id: string, animate: boolean) {
  for (const tab of tabs) {
    const on = tab.dataset.tab === id;
    tab.setAttribute("aria-selected", String(on));
    tab.tabIndex = on ? 0 : -1;
    if (on) placePill(tab);
  }
  for (const panel of $$("[data-panel]")) {
    const on = panel.dataset.panel === id;
    panel.hidden = !on;
    if (on && animate && !reduced()) {
      panel.animate([{ opacity: 0, transform: "translateY(16px)", filter: "blur(6px)" }, { opacity: 1, transform: "none", filter: "none" }], { duration: 600, easing: "cubic-bezier(.16, 1, .3, 1)" });
    }
  }
}
tabs.forEach((tab, i) => {
  tab.addEventListener("click", () => choose(tab.dataset.tab!, true));
  tab.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = tabs[(i + step + tabs.length) % tabs.length];
    next.focus();
    choose(next.dataset.tab!, true);
  });
});
choose(platform, false);
addEventListener("resize", () => {
  const on = tabs.find((t) => t.getAttribute("aria-selected") === "true");
  if (on) placePill(on);
});
document.fonts?.ready.then(() => {
  const on = tabs.find((t) => t.getAttribute("aria-selected") === "true");
  if (on) placePill(on);
});
// The hero button opens the right tab, too, and other links open the tab they name.
$("[data-os-cta]")?.addEventListener("click", () => choose(platform, false));
$$("[data-choose]").forEach((link) => link.addEventListener("click", () => choose(link.dataset.choose!, false)));

/* ---------- copy the Mac command */
const copyButton = $<HTMLButtonElement>("[data-copy]");
copyButton?.addEventListener("click", async () => {
  const text = $("[data-command]")!.textContent!.trim();
  const label = $("[data-copy-label]", copyButton)!;
  try {
    await navigator.clipboard.writeText(text);
    label.textContent = "Copied";
  } catch {
    getSelection()?.selectAllChildren($("[data-command]")!);
    label.textContent = "Press Ctrl C";
  }
  setTimeout(() => (label.textContent = "Copy"), 2200);
});

/* ---------- the buttons follow latest.json, the same file Needle's updaters read */
(async () => {
  try {
    const [latest, latestMac, latestAndroid] = await Promise.all(
      ["/latest.json", "/latest-macos.json", "/latest-android.json"].map((u) => fetch(u, { cache: "no-store" }).then((r) => (r.ok ? r.json() : null))),
    );
    if (!latest) return;
    const { files, version, macVersion, androidVersion } = pick(latest, latestMac, latestAndroid);
    for (const link of $$<HTMLAnchorElement>("[data-asset]")) {
      const kind = link.dataset.asset!;
      const name = files[kind as keyof typeof files];
      if (!name || link.pathname.endsWith(`/${name}`)) continue;
      link.href = `/download/${name}`;
      // The size shown was for the older file.
      for (const size of $$(`[data-size="${kind}"]`)) size.textContent = "";
    }
    if (version) $$('[data-version="main"]').forEach((v) => (v.textContent = version));
    if (macVersion) $$('[data-version="mac"]').forEach((v) => (v.textContent = macVersion));
    if (androidVersion) $$('[data-version="android"]').forEach((v) => (v.textContent = androidVersion));
  } catch {
    // Offline or blocked: the links from the build stay.
  }
})();

/* ---------- the first view comes in, one line after another */
$$("[data-rise]").forEach((el, i) => setTimeout(() => el.classList.add("is-in"), 80 + i * 90));

/* ---------- the rest comes in as you reach it */
const seen = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    e.target.classList.add("is-in");
    seen.unobserve(e.target);
  }
}, { rootMargin: "0px 0px -12% 0px" });
$$("[data-reveal]").forEach((el) => seen.observe(el));

/* ---------- the floating cards drift a little slower than the page */
const floats = $$("[data-float]");
if (floats.length && !reduced()) {
  let ticking = false;
  const drift = () => {
    ticking = false;
    const y = Math.min(scrollY, 900);
    for (const f of floats) f.style.translate = `0 ${-(f.dataset.float === "left" ? .12 : .2) * y}px`;
  };
  addEventListener("scroll", () => { if (!ticking) { ticking = true; requestAnimationFrame(drift); } }, { passive: true });
}
