#!/usr/bin/env node
// mobile-check.mjs: a hard test of the phone version of docs/site/index.html in headless Chromium.
//
//   node tools/site/mobile-check.mjs [file.html | URL] [--shots DIR] [--no-guide]
//
// Each phone width (320 to 430 and an 844 x 390 landscape) is loaded with touch emulated and a device pixel ratio of 2
// or 3, once normally and once with prefers-reduced-motion. It prints every failure and exits 1 if there is one,
// 2 if the check could not run. --shots writes a full-page screenshot per width. Then it runs the lxxvii guide's own
// checker at 390 x 844 (skip with --no-guide). No dependencies; Node 20.10 or later.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

if (typeof WebSocket === "undefined") {
	const r = spawnSync(process.execPath, ["--experimental-websocket", "--no-warnings", ...process.argv.slice(1)], { stdio: "inherit" });
	process.exit(r.status ?? 1);
}

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const flag = (name) => { const i = args.indexOf("--" + name); return i < 0 ? null : args[i + 1]; };
const target = args.find((a, i) => !a.startsWith("--") && args[i - 1] !== "--shots") || join(here, "../../docs/site/index.html");
const url = /^[a-z]+:\/\//.test(target) ? target : pathToFileURL(resolve(target)).href;
const shots = flag("shots");

// Ceilings from the lxxvii guide (rules.md, R2) for a landing page of five blocks.
const CEIL = { total: 530, block: 90, para: 50, hero: 25 };

const VIEWPORTS = [
	{ w: 320, h: 568, dpr: 2 },
	{ w: 360, h: 740, dpr: 3 },
	{ w: 375, h: 667, dpr: 2 },
	{ w: 390, h: 844, dpr: 3 },
	{ w: 414, h: 896, dpr: 2 },
	{ w: 430, h: 932, dpr: 3 },
	{ w: 844, h: 390, dpr: 3, landscape: true },
];

function findChrome() {
	const pinned = join(homedir(), "Library/Caches/ms-playwright/chromium_headless_shell-1255/chrome-headless-shell-mac-arm64/chrome-headless-shell");
	if (process.env.CHROME_BIN && existsSync(process.env.CHROME_BIN)) return process.env.CHROME_BIN;
	if (existsSync(pinned)) return pinned;
	for (const cache of [join(homedir(), "Library/Caches/ms-playwright"), join(homedir(), ".cache/ms-playwright")]) {
		if (!existsSync(cache)) continue;
		for (const dir of readdirSync(cache).filter((d) => /^chromium/.test(d)).sort().reverse())
			for (const sub of readdirSync(join(cache, dir)))
				for (const bin of ["chrome-headless-shell", "chrome"]) if (existsSync(join(cache, dir, sub, bin))) return join(cache, dir, sub, bin);
	}
	return null;
}

async function launch() {
	const bin = findChrome();
	if (!bin) throw new Error("no Chromium found; set CHROME_BIN");
	const dir = mkdtempSync(join(tmpdir(), "mobile-check-"));
	const proc = spawn(bin, ["--headless=new", "--remote-debugging-port=0", "--user-data-dir=" + dir, "--no-first-run", "--hide-scrollbars", "about:blank"], { stdio: ["ignore", "ignore", "pipe"] });
	const exited = new Promise((r) => proc.once("exit", r));
	const ws = await new Promise((res, rej) => {
		let buf = "";
		const t = setTimeout(() => rej(new Error("Chromium did not start: " + buf.slice(0, 300))), 20000);
		proc.stderr.on("data", (d) => { buf += d; const m = buf.match(/DevTools listening on (ws:\/\/\S+)/); if (m) { clearTimeout(t); res(m[1]); } });
	});
	const page = (await (await fetch(`http://127.0.0.1:${new URL(ws).port}/json`)).json()).find((t) => t.type === "page");
	const sock = new WebSocket(page.webSocketDebuggerUrl);
	await new Promise((r) => (sock.onopen = r));
	let id = 0; const pending = new Map(), waiters = [];
	sock.onmessage = (e) => {
		const m = JSON.parse(e.data);
		if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p.rej(new Error(m.error.message)) : p.res(m.result); }
		else waiters.filter((w) => w.method === m.method).forEach((w) => { waiters.splice(waiters.indexOf(w), 1); w.res(); });
	};
	const send = (method, params = {}) => new Promise((res, rej) => { pending.set(++id, { res, rej }); sock.send(JSON.stringify({ id, method, params })); });
	const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
	await send("Page.enable"); await send("Runtime.enable");
	return {
		send, sleep,
		async goto(u, wait = 900) { const loaded = new Promise((res) => waiters.push({ method: "Page.loadEventFired", res })); await send("Page.navigate", { url: u }); await Promise.race([loaded, sleep(20000)]); await sleep(wait); },
		async eval(expression) { const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text); return r.result.value; },
		async close() { try { sock.close(); } catch {} proc.kill(); await Promise.race([exited, sleep(3000)]); try { rmSync(dir, { recursive: true, force: true }); } catch {} },
	};
}

// ---------------------------------------------------------------------------------------------------------------
// Runs inside the page. Returns a list of failures, as strings, and a few figures.
function inPage(CEIL) {
	const out = [], info = {};
	const W = innerWidth, H = innerHeight;
	const cs = (el) => getComputedStyle(el);
	// shown: rendered, not inside a closed <details> (whose answer Chromium keeps laid out but never paints)
	const shown = (el) => { const s = cs(el), b = el.getBoundingClientRect(); return s.display !== "none" && s.visibility !== "hidden" && +s.opacity !== 0 && b.width > 1 && b.height > 1 && !el.closest("details:not([open]) > :not(summary)"); };
	const decor = (el) => !!el.closest('[aria-hidden="true"], svg, .cramp, .tablet-edge, .skip');
	const words = (t) => (t || "").trim().split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).length;
	const name = (el) => { const t = (el.innerText || el.getAttribute("aria-label") || el.getAttribute("alt") || "").trim().replace(/\s+/g, " "); return `<${el.tagName.toLowerCase()}${el.id ? "#" + el.id : ""}${el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\s+/).join(".") : ""}> “${t.slice(0, 48)}${t.length > 48 ? "…" : ""}”`; };
	const all = [...document.querySelectorAll("body *")].filter((el) => !/^(SCRIPT|STYLE|TEMPLATE)$/.test(el.tagName) && shown(el));
	// elements that carry their own text (a text node of more than white space)
	const texty = all.filter((el) => !decor(el) && [...el.childNodes].some((n) => n.nodeType === 3 && n.nodeValue.trim()));
	const absTop = (el) => el.getBoundingClientRect().top + scrollY;

	// 1. no horizontal scroll
	const sw = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
	if (sw > W) out.push(`horizontal scroll: the page is ${sw}px wide in a ${W}px viewport`);

	// 2. text inside the viewport and not clipped by a box that hides overflow
	for (const el of texty) {
		const b = el.getBoundingClientRect();
		if (b.right > W + .5 || b.left < -.5) out.push(`text runs outside the viewport (${Math.round(b.left)} to ${Math.round(b.right)}): ${name(el)}`);
		if (el.scrollWidth > el.clientWidth + 1 && cs(el).overflowX !== "visible" && el.clientWidth > 0) out.push(`text is clipped sideways (${el.scrollWidth} in ${el.clientWidth}px): ${name(el)}`);
		for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
			const s = cs(a);
			if (s.overflowX === "visible" && s.overflowY === "visible") continue;
			const ab = a.getBoundingClientRect();
			if (b.left < ab.left - .5 || b.right > ab.right + .5 || (s.overflowY !== "visible" && (b.top < ab.top - .5 || b.bottom > ab.bottom + .5))) out.push(`text is clipped by ${name(a).split(" ")[0]}: ${name(el)}`);
			break;
		}
	}

	// 3. words broken in the middle, and headings that leave one word on their last line
	const lines = (el) => {   // the words of an element, grouped by the line they stand on
		const ws = [];
		const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
		for (let n; (n = walk.nextNode());) {
			if (n.parentElement.closest('[aria-hidden="true"]')) continue;
			const re = /\S+/g; let m;
			while ((m = re.exec(n.nodeValue))) {
				const r = document.createRange(); r.setStart(n, m.index); r.setEnd(n, m.index + m[0].length);
				const rects = [...r.getClientRects()].filter((x) => x.width > 0);
				if (!rects.length) continue;
				const tops = new Set(rects.map((x) => Math.round(x.top)));
				ws.push({ w: m[0], top: Math.round(rects[0].top), broken: tops.size > 1 && !/[-–—/·,]/.test(m[0]) });
			}
		}
		const ls = [];
		for (const w of ws) { const l = ls.find((x) => Math.abs(x.top - w.top) < 6); l ? l.words.push(w.w) : ls.push({ top: w.top, words: [w.w] }); }
		return { ws, ls };
	};
	for (const el of texty) for (const w of lines(el).ws) if (w.broken) out.push(`the word “${w.w}” is broken across two lines in ${name(el)}`);
	const hero = document.querySelector(".tablet");
	const heads = [...document.querySelectorAll("h1, h2, summary")].concat(hero ? [...hero.querySelectorAll(".caps")] : []).filter(shown);
	for (const h of heads) {
		const { ls } = lines(h), n = ls.reduce((s, l) => s + l.words.length, 0);
		if (ls.length > 1 && n >= 3 && ls[ls.length - 1].words.length === 1) out.push(`a single word, “${ls[ls.length - 1].words[0]}”, alone on the last line of ${name(h)}`);
	}

	// 4. tap targets: 44 x 44 at least
	for (const el of all.filter((e) => e.matches("a[href], button, summary, [role=button], input, select") && !e.closest(".skip"))) {
		const b = el.getBoundingClientRect();
		if (b.height < 44 - .5 || b.width < 44 - .5) out.push(`a tap target of ${Math.round(b.width)} × ${Math.round(b.height)}px (44 × 44 at least): ${name(el)}`);
	}

	// 5. running text (a sentence of more than six words) is 16px or more, with sensible lines. The style's caps label
	// (11px capitals, the hero's line) is a label and not running text; headings and questions are judged in 3.
	for (const el of texty.filter((e) => !e.closest(".caps, h1, h2, summary"))) {
		const own = [...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.nodeValue).join(" ");
		const n = words(el.innerText);
		if (n <= 6 || words(own) < 3) continue;
		const fs = parseFloat(cs(el).fontSize);
		if (fs < 16) out.push(`running text at ${fs}px (16 at least): ${name(el)}`);
		const { ls } = lines(el);
		if (ls.length >= 3) {
			const full = ls.slice(0, -1), cpl = full.reduce((s, l) => s + l.words.join(" ").length, 0) / full.length;
			if (cpl > 80) out.push(`lines of ${Math.round(cpl)} characters (80 at most): ${name(el)}`);
			if (cpl < 22) out.push(`lines of ${Math.round(cpl)} characters (22 at least): ${name(el)}`);
		}
		const lh = parseFloat(cs(el).lineHeight) / fs;
		if (lh < 1.3) out.push(`a line height of ${lh.toFixed(2)} on running text (1.3 at least): ${name(el)}`);
	}

	// 6. the hero fits: the name, the line and the download button are in the first screen, inside the tablet
	if (hero) {
		const hb = hero.getBoundingClientRect(), btn = hero.querySelector(".btn"), h1 = hero.querySelector("h1"), cue = hero.querySelector(".cue");
		for (const el of [h1, btn, ...hero.querySelectorAll(".caps")]) {
			if (!el) continue;
			const b = el.getBoundingClientRect();
			if (b.top < hb.top || b.bottom > Math.min(hb.bottom, H) || b.left < hb.left || b.right > hb.right) out.push(`not wholly visible in the first screen, inside the tablet: ${name(el)} (${Math.round(b.top)} to ${Math.round(b.bottom)} of ${H})`);
		}
		if (btn && cue) { const a = btn.getBoundingClientRect(), c = cue.getBoundingClientRect(); if (a.bottom > c.top - 8 && a.right > c.left && a.left < c.right) out.push(`the download button (bottom ${Math.round(a.bottom)}) runs into the scroll cue (top ${Math.round(c.top)})`); }
		info.btnBottom = btn ? Math.round(btn.getBoundingClientRect().bottom) : null;
		const heroWords = words(hero.innerText) - words(h1 ? h1.innerText : "");
		if (heroWords > CEIL.hero) out.push(`the hero holds ${heroWords} words besides the name (${CEIL.hero} at most)`);
	} else out.push("no hero tablet");

	// 7. the chart: every bar drawn, every label readable and on one line, in the viewport
	const chart = document.querySelector(".chart");
	if (chart) {
		const cb = chart.getBoundingClientRect();
		if (cb.right > W || cb.left < 0) out.push(`the chart runs outside the viewport (${Math.round(cb.left)} to ${Math.round(cb.right)})`);
		for (const bar of chart.querySelectorAll(".bar")) { const b = bar.getBoundingClientRect(), t = bar.parentElement.getBoundingClientRect(); if (b.width < 2 || b.height < 4) out.push(`a bar of ${b.width.toFixed(1)} × ${b.height.toFixed(1)}px in the chart`); if (b.right > t.right + .5) out.push("a bar runs past its track"); }
		for (const el of chart.querySelectorAll(".v, th, figcaption")) {
			const fs = parseFloat(cs(el).fontSize);
			if (fs < 14) out.push(`chart text at ${fs}px (14 at least): ${name(el)}`);
			if (el.matches(".v") && lines(el).ls.length > 1) out.push(`a chart value wraps: ${name(el)}`);
		}
		const rows = [...chart.querySelectorAll("tbody tr")].map((r) => r.querySelectorAll(".track")[0]?.getBoundingClientRect().width);
		if (rows.some((w) => w < 100)) out.push(`a chart track is under 100px wide (${rows.map((w) => Math.round(w)).join(", ")})`);
		info.chartTrack = Math.round(rows[0] || 0);
	} else out.push("no chart");

	// 8. the table: no cell outside the table, no column squeezed under 100px when the rows stand side by side
	for (const t of document.querySelectorAll(".tbl")) {
		const tb = t.getBoundingClientRect();
		if (tb.right > W + .5) out.push(`the table runs outside the viewport (right edge ${Math.round(tb.right)})`);
		for (const c of t.querySelectorAll("tbody th, tbody td")) {
			if (!c.getClientRects().length) continue; // not drawn at this width (display:none), so not squeezed
			const b = c.getBoundingClientRect();
			if (b.right > tb.right + .5 || b.left < tb.left - .5) out.push(`a table cell runs outside its table: ${name(c)}`);
			if (b.width < 100) out.push(`a table cell only ${Math.round(b.width)}px wide: ${name(c)}`);
		}
	}

	// 9. images scale: inside the viewport and their column, in their own proportions
	for (const img of document.querySelectorAll("img")) {
		const b = img.getBoundingClientRect(), p = img.parentElement.getBoundingClientRect();
		if (b.right > W + .5 || b.width > p.width + .5) out.push(`an image wider than its place (${Math.round(b.width)} in ${Math.round(p.width)}): ${name(img)}`);
		const want = +img.getAttribute("height") / +img.getAttribute("width");
		if (want && Math.abs(b.height / b.width - want) > .01) out.push(`an image out of proportion (${(b.height / b.width).toFixed(3)}, should be ${want.toFixed(3)}): ${name(img)}`);
		if (img.complete && !img.naturalWidth) out.push(`an image did not load: ${name(img)}`);
	}

	// 10. nothing overlaps: text, images, buttons and figures, each against every other that is not its own ancestor
	// an inline element that wraps is judged by each of its line boxes, not by the rectangle around them all
	const boxes = all.filter((el) => !decor(el) && (texty.includes(el) || el.matches("img, .btn, .cue, .bar, summary, .go"))).map((el) => ({ el, rs: cs(el).display === "inline" ? [...el.getClientRects()] : [el.getBoundingClientRect()] }));
	const seen = new Set();
	for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) {
		const A = boxes[i], B = boxes[j];
		if (A.el.contains(B.el) || B.el.contains(A.el)) continue;
		for (const a of A.rs) for (const b of B.rs) {
			const x = Math.min(a.right, b.right) - Math.max(a.left, b.left), y = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
			if (x > 1 && y > 1) { const k = name(A.el) + " / " + name(B.el); if (!seen.has(k)) { seen.add(k); out.push(`overlap of ${Math.round(x)} × ${Math.round(y)}px: ${k}`); } }
		}
	}

	// 11. word ceilings (R2): per paragraph, per block's copy, on the page; and how many screens it takes
	const vis = (el) => { let t = el.innerText || ""; for (const h of el.querySelectorAll('[aria-hidden="true"]')) { const ht = (h.innerText || "").trim(); if (ht) t = t.replace(ht, " "); } return t; };
	info.words = { total: words(vis(document.body)) };
	if (info.words.total > CEIL.total) out.push(`${info.words.total} visible words on the page (${CEIL.total} at most)`);
	for (const sec of document.querySelectorAll("main > section, footer")) {
		const id = sec.id || sec.tagName.toLowerCase(), n = words(vis(sec));
		info.words[id] = n;
		const copy = sec.querySelector(".copy");
		if (copy && words(vis(copy)) > CEIL.block) out.push(`${words(vis(copy))} words in the copy of #${id} (${CEIL.block} at most)`);
	}
	info.words.hero = hero ? words(vis(hero)) : 0;
	for (const p of document.querySelectorAll("main p, footer p")) if (shown(p) && words(p.innerText) > CEIL.para) out.push(`a paragraph of ${words(p.innerText)} words (${CEIL.para} at most): ${name(p)}`);
	info.screens = +(document.documentElement.scrollHeight / H).toFixed(1);
	return { out, info };
}

// Tap each question twice through real touch events: it must open with its answer visible, then close.
async function tapFaq(c, reduced) {
	const out = [];
	const n = await c.eval(`document.querySelectorAll('.qa').length`);
	if (!n) return ["no FAQ found"];
	for (let i = 0; i < n; i++) {
		for (const want of [true, false]) {
			const p = await c.eval(`(() => { const s = document.querySelectorAll('.qa summary')[${i}]; s.scrollIntoView({ block: 'center', behavior: 'instant' }); const b = s.getBoundingClientRect(); return { x: b.left + b.width / 2, y: b.top + b.height / 2 }; })()`);
			await c.sleep(120);
			const pt = [{ x: p.x, y: p.y, radiusX: 4, radiusY: 4, force: 1, id: 1 }];
			await c.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: pt });
			await c.sleep(40);
			await c.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
			await c.sleep(reduced ? 150 : 700);
			const s = await c.eval(`(() => { const d = document.querySelectorAll('.qa')[${i}], a = d.querySelector('.a'), b = a.getBoundingClientRect(); return { open: d.open, h: b.height, natural: a.scrollHeight, op: +getComputedStyle(a).opacity, bottom: b.bottom, vh: innerHeight, words: a.innerText.trim().split(/\\s+/).length } })()`);
			const q = `question ${i + 1}${reduced ? " (reduced motion)" : ""}`;
			if (want) {
				if (!s.open) out.push(`${q} did not open on tap`);
				else if (s.h < s.natural - 1 || s.op < .99) out.push(`${q} opened but its answer shows ${Math.round(s.h)} of ${s.natural}px at opacity ${s.op}`);
			} else if (s.open) out.push(`${q} did not close on the second tap`);
		}
	}
	return out;
}

let c;
const report = [];
try {
	c = await launch();
	await c.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
	await c.send("Emulation.setEmitTouchEventsForMouse", { enabled: true, configuration: "mobile" });
	await c.send("Emulation.setUserAgentOverride", { userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1", platform: "iPhone" });
	if (shots) mkdirSync(shots, { recursive: true });
	for (const v of VIEWPORTS) {
		for (const reduced of [false, true]) {
			await c.send("Emulation.setDeviceMetricsOverride", { width: v.w, height: v.h, deviceScaleFactor: v.dpr, mobile: true, screenOrientation: v.landscape ? { type: "landscapePrimary", angle: 90 } : { type: "portraitPrimary", angle: 0 } });
			await c.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: reduced ? "reduce" : "no-preference" }] });
			await c.goto(url);
			// the first screen is judged as loaded; then the page is walked once so every reveal has fired and every image loaded
			const first = await c.eval(`(${inPage})(${JSON.stringify(CEIL)})`);
			await c.eval(`(async () => { for (let y = 0; y < document.documentElement.scrollHeight; y += innerHeight * .5) { scrollTo({ top: y, behavior: 'instant' }); await new Promise((r) => setTimeout(r, 120)); } await new Promise((r) => setTimeout(r, 1300)); scrollTo({ top: 0, behavior: 'instant' }); await new Promise((r) => setTimeout(r, 200)); })()`);
			const walked = await c.eval(`(${inPage})(${JSON.stringify(CEIL)})`);
			const hidden = await c.eval(`[...document.querySelectorAll('main *, footer *')].filter((el) => +getComputedStyle(el).opacity === 0 && el.innerText && el.innerText.trim() && !el.closest('details:not([open])')).length`);
			const anims = reduced ? await c.eval(`document.getAnimations().filter((a) => a.playState === 'running').map((a) => a.animationName || 'script').join(', ')`) : "";
			const fails = [...new Set([...first.out.filter((f) => /hero|first screen|download button|scroll cue/.test(f)), ...walked.out.filter((f) => !/hero|first screen|download button|scroll cue/.test(f))])];
			if (hidden) fails.push(`${hidden} element(s) with text still invisible after the page was walked`);
			if (anims) fails.push(`animations still running under reduced motion: ${anims}`);
			fails.push(...(await tapFaq(c, reduced)));
			if (shots && !reduced) {
				// the questions were tapped: close them and go back to the top, so the picture is the page as it loads
				const m = await c.eval(`(async () => { document.querySelectorAll('.qa').forEach((d) => (d.open = false)); scrollTo({ top: 0, behavior: 'instant' }); await new Promise((r) => setTimeout(r, 300)); return { w: innerWidth, h: document.documentElement.scrollHeight }; })()`);
				const r = await c.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true, clip: { x: 0, y: 0, width: m.w, height: m.h, scale: 1 } });
				writeFileSync(join(shots, `phone-${v.w}x${v.h}.png`), Buffer.from(r.data, "base64"));
				if (v.w === 390) {
					const ch = await c.eval(`(() => { const b = document.querySelector('.block#efficiency').getBoundingClientRect(); return { y: b.top + scrollY + 40, h: b.height - 40 } })()`);
					const r2 = await c.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true, clip: { x: 0, y: ch.y, width: m.w, height: ch.h, scale: 1 } });
					writeFileSync(join(shots, "chart-390.png"), Buffer.from(r2.data, "base64"));
				}
			}
			report.push({ v, reduced, fails, info: walked.info, btnBottom: first.info.btnBottom });
		}
	}
} catch (e) { console.error("The check did not finish: " + e.message); await c?.close(); process.exit(2); }
await c.close();

let total = 0;
for (const r of report) {
	const tag = `${r.v.w} × ${r.v.h} @${r.v.dpr}x${r.v.landscape ? " landscape" : ""}${r.reduced ? ", reduced motion" : ""}`;
	total += r.fails.length;
	console.log(`${r.fails.length ? "FAIL" : "ok  "} ${tag.padEnd(34)} ${r.info.screens} screens · download button bottom ${r.btnBottom}px · chart track ${r.info.chartTrack}px`);
	for (const f of r.fails) console.log("       " + f);
}
const w = report[0].info.words;
console.log(`\nwords: ${Object.entries(w).map(([k, n]) => `${k} ${n}`).join(" · ")} (ceilings: page ${CEIL.total}, block ${CEIL.block}, paragraph ${CEIL.para})`);

if (!args.includes("--no-guide")) {
	const guide = join(homedir(), ".claude/skills/lxxvii-style/tools/check-page.mjs");
	if (existsSync(guide)) {
		const g = spawnSync(process.execPath, [guide, target, "--width", "390", "--height", "844"], { encoding: "utf8" });
		const last = (g.stdout || "").trim().split("\n");
		console.log(`\nguide checker at 390 × 844: ${last[last.length - 1]}`);
		for (const l of last.filter((x) => /^FAIL/.test(x))) console.log("       " + l);
		if (g.status !== 0) total++;
	} else console.log("\nguide checker not found; skipped");
}
console.log(`\n${total} failure(s) across ${report.length} runs.`);
process.exitCode = total ? 1 : 0;
