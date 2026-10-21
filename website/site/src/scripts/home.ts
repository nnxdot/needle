// The home page's motion and the download chooser. Everything on the page is readable without
// this script; it only adds movement, and keeps the download buttons in step with latest.json.
import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { SplitText } from "gsap/SplitText";
import { pick } from "../lib/assets.js";
import { byId, cover, playing } from "../lib/albums.js";

gsap.registerPlugin(ScrollTrigger, SplitText);
// Phones hide and show the address bar while scrolling; that must not restart the animations.
ScrollTrigger.config({ ignoreMobileResize: true });

const $ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document) => root.querySelector<T>(sel);
const $$ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document) => [...root.querySelectorAll<T>(sel)];

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
      gsap.fromTo($(".device", panel), { opacity: 0, y: 40, scale: .94, rotateX: 8 }, { opacity: 1, y: 0, scale: 1, rotateX: 0, duration: 1, ease: "expo.out" });
      gsap.fromTo($$(".dl-info > *", panel), { opacity: 0, x: 28 }, { opacity: 1, x: 0, duration: .8, ease: "expo.out", stagger: .05 });
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

/* ---------- the drawn app plays songs in turn; the glow takes each cover's color */
const apps = $$("[data-app]");
let song = 0;
function play(next: number, fade: boolean) {
  song = next % playing.length;
  const now = playing[song];
  const album = byId[now.album];
  for (const app of apps) {
    app.style.setProperty("--glow", album.glow);
    const swap = () => {
      $$<HTMLImageElement>("[data-now-cover]", app).forEach((img) => (img.src = cover(album.id)));
      $$("[data-now-song]", app).forEach((el) => (el.textContent = now.song));
      $$("[data-now-artist]", app).forEach((el) => (el.textContent = album.artist));
      $$("[data-now-time]", app).forEach((el) => (el.textContent = now.time));
    };
    const parts = $$("[data-now-cover], [data-now-song], [data-now-artist]", app);
    if (fade) {
      gsap.timeline()
        .to(parts, { opacity: 0, y: 6, filter: "blur(4px)", duration: .35, ease: "power2.in" })
        .add(swap)
        .to(parts, { opacity: 1, y: 0, filter: "blur(0px)", duration: .6, ease: "expo.out" });
    } else swap();
  }
  $("[data-stage-in]")?.style.setProperty("--glow", album.glow);
}
play(0, false);
let timer = setInterval(() => !document.hidden && play(song + 1, !reduced()), 6000);
document.addEventListener("visibilitychange", () => {
  clearInterval(timer);
  if (!document.hidden) timer = setInterval(() => play(song + 1, !reduced()), 6000);
});

/* ---------- themes: the window wears each look in turn */
const themeSection = $("[data-themes]");
const themeApp = $(".themes-app");
const lookItems = $$("[data-look]");
const lookGlow: Record<string, string> = { night: "#e2b46c", midnight: "#9a9aa2", day: "#f0d8a8", sakura: "#ec6a9a" };
let lookNow = "night";
function setLook(id: string) {
  if (id === lookNow || !themeApp) return;
  lookNow = id;
  themeApp.classList.remove("look-night", "look-midnight", "look-day", "look-sakura");
  themeApp.classList.add(`look-${id}`);
  lookItems.forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.look === id)));
  themeSection?.style.setProperty("--look-glow", lookGlow[id]);
}
lookItems.forEach((b) => b.addEventListener("click", () => setLook(b.dataset.look!)));

/* ---------- small things that happen once, when seen */
function once(el: Element | null, run: () => void, margin = "0px 0px -15% 0px") {
  if (!el) return;
  const io = new IntersectionObserver(([e]) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    run();
  }, { rootMargin: margin });
  io.observe(el);
}
// The smart playlist rule types itself.
const rule = $<HTMLElement>("[data-type]");
once(rule, () => {
  if (!rule || reduced()) return;
  const text = rule.dataset.type!;
  rule.textContent = "";
  rule.classList.add("typing");
  let i = 0;
  const tick = setInterval(() => {
    rule.textContent = text.slice(0, ++i);
    if (i >= text.length) clearInterval(tick);
  }, 55);
});
// The year counts up.
for (const num of $$("[data-count]")) {
  once(num, () => {
    if (reduced()) return;
    const to = Number(num.dataset.count);
    const counter = { v: 0 };
    gsap.to(counter, { v: to, duration: 1.6, ease: "expo.out", onUpdate: () => (num.textContent = Math.round(counter.v).toLocaleString("en")) });
  });
}

function reduced() {
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/* ---------- motion tied to scrolling */
const mm = gsap.matchMedia();
mm.add(
  {
    motion: "(prefers-reduced-motion: no-preference)",
    wide: "(min-width: 768px)",
    fine: "(pointer: fine)",
  },
  (ctx) => {
    const { motion, wide, fine } = ctx.conditions as { motion: boolean; wide: boolean; fine: boolean };
    // A moment that plays once it is in view, and again when you come back to it.
    const seen = (trigger: gsap.DOMTarget, start = "top 80%"): ScrollTrigger.Vars =>
      ({ trigger, start, end: "bottom top", toggleActions: "play reset play reverse" });

    if (!motion) {
      // Still: the themes page cycles by tapping its names; nothing moves by itself.
      return;
    }

    /* The first view: letters rise, the record arrives, the tonearm comes down. */
    const split = SplitText.create("[data-split]", { type: "words,chars", mask: "words" });
    const intro = gsap.timeline({ defaults: { ease: "expo.out" } });
    intro
      .from(split.chars, { yPercent: 110, duration: 1.3, stagger: .028 }, .1)
      .from("[data-hero-fade]", { opacity: 0, y: 24, filter: "blur(8px)", duration: 1.2, stagger: .1 }, .45)
      .from("[data-turntable]", { opacity: 0, y: 160, scale: .9, duration: 1.8 }, .2)
      .fromTo("[data-arm]", { rotate: -16 }, { rotate: 14, duration: 1.6, ease: "power3.inOut" }, .9);

    /* Leaving the first view: the words lift away, the record sinks and grows. */
    gsap.timeline({ scrollTrigger: { trigger: "[data-hero]", start: "top top", end: "bottom top", scrub: true } })
      .to(".hero-copy", { y: -140, opacity: 0, ease: "none" }, 0)
      .to("[data-turntable]", { y: 120, scale: 1.12, ease: "none" }, 0);

    /* Lyrics: each line is sung in turn; the lines move up as karaoke does. */
    const lines = $$(".lp-line");
    const box = $("[data-lines]")!;
    const step = 1.2;
    const sing = gsap.timeline({ defaults: { ease: "none" }, paused: true });
    gsap.set($$("span", box), { opacity: .22 });
    lines.forEach((line, i) => {
      const at = i * step;
      // Snap points: each line once it is fully sung.
      sing.addLabel(`line${i}`, at + .4 + .14 * (line.children.length - 1));
      sing.to(lines, { y: () => -(line.offsetTop - box.clientHeight * .3), duration: .5, ease: "power2.inOut" }, at);
      sing.to($$("span", line), { opacity: 1, stagger: .14, duration: .2 }, at + .2);
      if (i > 0) sing.to($$("span", lines[i - 1]), { opacity: .38, duration: .3 }, at);
    });

    /* Small things that keep moving a little. */
    $$<SVGCircleElement>(".eq-dot").forEach((dot, i) => gsap.to(dot, { y: i % 2 ? 14 : -14, duration: 2.4 + i * .3, ease: "sine.inOut", repeat: -1, yoyo: true, delay: 1.2 }));
    $$(".stem-wave").forEach((wave, i) => gsap.to(wave, { backgroundPositionX: 60, duration: 1.4 + i * .2, ease: "none", repeat: -1 }));
    gsap.to(".dl-icon", { y: -10, duration: 3, ease: "sine.inOut", repeat: -1, yoyo: true });

    const looks = ["night", "midnight", "day", "sakura"];

    if (!wide) {
      // On a phone nothing waits for the scroll: every word, card, and picture is there as it is.
      // The lyrics sing over and over, and the window changes its look, while they are on the screen.
      sing.to(box, { opacity: 0, duration: .5 }, `+=${step}`).set(box, { opacity: 1 });
      sing.repeat(-1);
      const turn = gsap.timeline({ repeat: -1, paused: true });
      looks.forEach((_, k) => turn.call(() => setLook(looks[(k + 1) % 4]), undefined, (k + 1) * 2.2));
      ScrollTrigger.create({ trigger: "[data-lyrics]", start: "top bottom", end: "bottom top", onToggle: (st) => (st.isActive ? sing.play() : sing.pause()) });
      ScrollTrigger.create({ trigger: "[data-themes]", start: "top bottom", end: "bottom top", onToggle: (st) => (st.isActive ? turn.play() : turn.pause()) });
      gsap.set(".merged", { opacity: 1 });
      return () => {
        split.revert();
        sing.kill();
        turn.kill();
        gsap.set(box, { clearProps: "opacity" });
        gsap.set(".merged", { clearProps: "opacity" });
      };
    }

    /* The app stands up out of the record. */
    gsap.timeline({ scrollTrigger: { trigger: "[data-stage]", start: "top 98%", end: "top 12%", scrub: 1 } })
      .fromTo(".stage-app", { rotateX: 34, y: 120, scale: .84, opacity: .4 }, { rotateX: 0, y: 0, scale: 1, opacity: 1, ease: "power2.out" }, 0)
      .from(".stage-app .tile", { opacity: 0, scale: .7, y: 30, stagger: { each: .04, from: "center" }, ease: "power2.out" }, .15)
      .from('[data-chip="left"]', { x: -120, opacity: 0, ease: "power2.out" }, .35)
      .from('[data-chip="right"]', { x: 120, opacity: 0, ease: "power2.out" }, .4)
      .from('[data-chip="low"]', { y: 80, opacity: 0, ease: "power2.out" }, .5);
    // Then the cards drift, a little behind the window.
    gsap.to('[data-chip="left"]', { y: -60, ease: "none", scrollTrigger: { trigger: "[data-stage]", start: "top top", end: "bottom top", scrub: true } });
    gsap.to('[data-chip="right"]', { y: -100, ease: "none", scrollTrigger: { trigger: "[data-stage]", start: "top top", end: "bottom top", scrub: true } });

    /* The statement lights up word by word. */
    gsap.timeline({ scrollTrigger: { trigger: "[data-statement]", start: "center center", end: "+=110%", pin: true, scrub: true } })
      .fromTo($$(".statement .w"), { opacity: .14 }, { opacity: 1, stagger: .1, ease: "none" });

    /* The lyrics section holds still and each line is sung as you scroll, stopping on a line when you stop. */
    ScrollTrigger.create({
      animation: sing, trigger: "[data-lyrics]", start: "top top", end: "+=240%", pin: true, scrub: .6, invalidateOnRefresh: true,
      snap: { snapTo: "labelsDirectional", inertia: false, duration: { min: .2, max: .6 }, delay: .1, ease: "power2.inOut" },
    });
    gsap.fromTo(".lyrics-bg", { scale: 1.2, rotate: 0 }, { scale: 1.45, rotate: 8, ease: "none", scrollTrigger: { trigger: "[data-lyrics]", start: "top bottom", end: "bottom top", scrub: true } });
    gsap.from(".lyrics-copy > *", { opacity: 0, y: 40, filter: "blur(6px)", stagger: .1, duration: 1, ease: "expo.out", scrollTrigger: { trigger: "[data-lyrics]", start: "top 60%" } });

    /* The highlights move across while the page scrolls down. */
    const track = $("[data-pan-track]")!;
    const pan = $("[data-pan]")!;
    pan.classList.add("is-panning");
    const distance = () => Math.max(0, track.scrollWidth - pan.clientWidth);
    const panTween = gsap.to(track, {
      x: () => -distance(),
      ease: "none",
      scrollTrigger: { trigger: pan, start: "top top", end: () => `+=${distance()}`, pin: true, scrub: 1, anticipatePin: 1, invalidateOnRefresh: true },
    });
    const inPan = (target: Element | null): ScrollTrigger.Vars => ({ trigger: target, containerAnimation: panTween, start: "left 85%" });
    for (const card of $$(".card")) {
      gsap.from($$(".card-art > *", card), { opacity: 0, y: 40, scale: .94, duration: 1.1, ease: "expo.out", scrollTrigger: inPan(card) });
    }
    gsap.from(".room", { opacity: 0, x: 40, stagger: .1, duration: .9, ease: "expo.out", scrollTrigger: inPan($(".card-speakers")) });
    gsap.from(".radio-row img", { opacity: 0, y: 50, rotate: (i) => (i % 2 ? 8 : -8), stagger: .08, duration: 1, ease: "back.out(1.6)", scrollTrigger: inPan($(".card-radio")) });
    gsap.from(".radio-chips span", { opacity: 0, scale: .6, stagger: .08, duration: .7, ease: "back.out(2)", scrollTrigger: inPan($(".card-radio")) });
    gsap.from(".srow", { opacity: 0, y: 30, stagger: .1, duration: .9, ease: "expo.out", scrollTrigger: inPan($(".card-server")) });
    gsap.from(".year-bars span", { scaleY: 0, stagger: .05, duration: 1, ease: "expo.out", scrollTrigger: inPan($(".card-year")) });
    gsap.from(".pan-intro > *", { opacity: 0, y: 40, stagger: .1, duration: 1, ease: "expo.out", scrollTrigger: { trigger: pan, start: "top 70%" } });

    /* Themes: the window changes its look as you scroll past. */
    gsap.timeline({
      scrollTrigger: {
        trigger: "[data-themes]", start: "top top", end: "+=260%", pin: true, scrub: .5,
        onUpdate: (st) => setLook(looks[Math.min(3, Math.floor(st.progress * 4.2))]),
        // Stop on one look when the scrolling stops.
        snap: { snapTo: [0, .36, .6, .84, 1], inertia: false, duration: { min: .2, max: .6 }, delay: .1, ease: "power2.inOut", directional: true },
      },
    })
      .fromTo(".themes-app", { rotateY: -10, rotateX: 6, scale: .92 }, { rotateY: 0, rotateX: 0, scale: 1, ease: "power1.inOut", duration: 1 })
      .to(".themes-app", { rotateY: 8, scale: .96, ease: "power1.inOut", duration: 1 })
      .to(".themes-app", { rotateY: 0, scale: 1, ease: "power1.inOut", duration: 1 });
    gsap.set(".themes-stage", { perspective: 1600 });
    gsap.from(".themes-copy > *", { opacity: 0, y: 40, stagger: .1, duration: 1, ease: "expo.out", scrollTrigger: { trigger: "[data-themes]", start: "top 70%" } });

    /* Android: the phones rise and fan out. */
    gsap.timeline({ scrollTrigger: seen(".phones", "top 85%"), defaults: { duration: 1.3, ease: "expo.out" } })
      .from(".ps-mid", { y: 160, opacity: 0, scale: .9 }, 0)
      .from(".ps-left", { xPercent: 60, y: 120, rotate: 7, opacity: 0 }, .12)
      .from(".ps-right", { xPercent: -60, y: 120, rotate: -7, opacity: 0 }, .12)
      .from(".phone-shot figcaption", { opacity: 0, y: 12, duration: .8, stagger: .06 }, .6);
    gsap.from(".android-copy > *", { opacity: 0, y: 40, filter: "blur(6px)", stagger: .1, duration: 1, ease: "expo.out", scrollTrigger: seen("[data-android]", "top 70%") });
    gsap.from(".android-points li", { opacity: 0, y: 24, scale: .96, stagger: .07, duration: .8, ease: "expo.out", scrollTrigger: seen(".android-points", "top 90%") });
    // The side phones drift a little slower than the page.
    gsap.to(".ps-left .phone, .ps-right .phone", { yPercent: -8, ease: "none", scrollTrigger: { trigger: ".phones", start: "top bottom", end: "bottom top", scrub: true } });

    /* Sound: the cells rise in; the EQ curve draws itself; the two songs join. */
    gsap.from(".sound-title", { opacity: 0, y: 60, duration: 1.2, ease: "expo.out", scrollTrigger: { trigger: ".sound", start: "top 75%" } });
    ScrollTrigger.batch(".cell", {
      start: "top 88%",
      onEnter: (cells) => gsap.from(cells, { opacity: 0, y: 70, scale: .97, stagger: .08, duration: 1.1, ease: "expo.out" }),
      once: true,
    });
    const line = $<SVGPathElement>(".eq-line");
    if (line) {
      const length = line.getTotalLength();
      gsap.fromTo(line, { strokeDasharray: length, strokeDashoffset: length }, { strokeDashoffset: 0, ease: "none", scrollTrigger: { trigger: ".c-eq", start: "top 85%", end: "center 45%", scrub: true } });
      gsap.from(".eq-area", { opacity: 0, ease: "none", scrollTrigger: { trigger: ".c-eq", start: "top 70%", end: "center 45%", scrub: true } });
      gsap.from(".eq-dot", { scale: 0, transformOrigin: "center", stagger: .12, duration: .6, ease: "back.out(3)", scrollTrigger: { trigger: ".c-eq", start: "center 60%" } });
    }
    gsap.fromTo(".j-b", { x: 28 }, { x: 0, ease: "power2.out", scrollTrigger: { trigger: ".c-play", start: "top 85%", end: "center 55%", scrub: true } });
    gsap.timeline({ scrollTrigger: { trigger: ".c-tidy", start: "center 70%" } })
      .to(".dupe.gone", { y: -28, opacity: 0, scale: .9, duration: .8, ease: "power3.in", delay: .6 })
      .to(".dupe.keep", { scale: 1.04, duration: .25, ease: "power2.out" })
      .to(".dupe.keep", { scale: 1, duration: .6, ease: "elastic.out(1, .4)" })
      .to(".merged", { opacity: 1, duration: .5 }, "-=.4");
    gsap.from(".home-list li", { opacity: 0, y: 24, stagger: .06, duration: .8, ease: "expo.out", scrollTrigger: { trigger: ".c-home", start: "top 85%" } });

    /* Privacy */
    const privacyTitle = SplitText.create(".private-title", { type: "words", mask: "words" });
    gsap.from(privacyTitle.words, { yPercent: 110, stagger: .08, duration: 1.2, ease: "expo.out", scrollTrigger: { trigger: ".private", start: "top 70%" } });
    gsap.from(".facts li", { opacity: 0, y: 50, stagger: .12, duration: 1.1, ease: "expo.out", scrollTrigger: { trigger: ".facts", start: "top 80%" } });

    /* Download */
    gsap.from(".dl-head > *", { opacity: 0, y: 40, stagger: .1, duration: 1.1, ease: "expo.out", scrollTrigger: { trigger: "[data-download]", start: "top 70%" } });
    gsap.from(".dl-tabs, .dl-panels", { opacity: 0, y: 50, stagger: .12, duration: 1.1, ease: "expo.out", scrollTrigger: { trigger: ".dl-tabs", start: "top 85%" } });

    if (fine) {
      /* Devices lean toward the pointer. */
      for (const device of $$(".device")) {
        const target = $(".screen", device)!;
        const rx = gsap.quickTo(target, "rotateX", { duration: .8, ease: "power3.out" });
        const ry = gsap.quickTo(target, "rotateY", { duration: .8, ease: "power3.out" });
        device.addEventListener("pointermove", (e) => {
          const r = device.getBoundingClientRect();
          ry(((e.clientX - r.left) / r.width - .5) * 12);
          rx(-((e.clientY - r.top) / r.height - .5) * 10);
        });
        device.addEventListener("pointerleave", () => { rx(0); ry(0); });
      }
      /* The main buttons lean toward the pointer, too. */
      for (const button of $$("[data-magnet]")) {
        const x = gsap.quickTo(button, "x", { duration: .6, ease: "power3.out" });
        const y = gsap.quickTo(button, "y", { duration: .6, ease: "power3.out" });
        button.addEventListener("pointermove", (e) => {
          const r = button.getBoundingClientRect();
          x((e.clientX - r.left - r.width / 2) * .25);
          y((e.clientY - r.top - r.height / 2) * .35);
        });
        button.addEventListener("pointerleave", () => { x(0); y(0); });
      }
    }

    return () => {
      pan.classList.remove("is-panning");
      split.revert();
      privacyTitle.revert();
      sing.kill();
    };
  },
);

document.fonts?.ready.then(() => ScrollTrigger.refresh());
addEventListener("load", () => ScrollTrigger.refresh());
