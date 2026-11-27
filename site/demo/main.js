// Connects the page to the LittleDraw editor compiled to WebAssembly
// (crates/web): DOM events in, files, clipboard and storage out.

import init, { Demo } from "./pkg/littledraw_web.js";

const STORAGE = "littledraw-demo";
const AUTOSAVE_DELAY = 700;
const DOUBLE_CLICK_TIME = 400;
const DOUBLE_CLICK_DISTANCE = 4;
const WHEEL_LINE = 40;
// The editor's interface is laid out for at least this many CSS pixels of
// width; narrower screens see everything scaled down to fit.
const MIN_LAYOUT_WIDTH = 580;

const $ = (id) => document.getElementById(id);
const canvas = $("board");
const keys = $("keys");

function load(name) {
  try { return localStorage.getItem(`${STORAGE}:${name}`); } catch { return null; }
}
function store(name, value) {
  try { localStorage.setItem(`${STORAGE}:${name}`, value); return true; } catch { return false; }
}

let noticeTimer;
function notice(text) {
  const el = $("notice");
  el.textContent = text;
  el.hidden = false;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => { el.hidden = true; }, 4000);
}

await init();
const saved = load("scene");
const demo = new Demo(canvas, saved, document.documentElement.dataset.theme === "dark");

// ----- painting -----------------------------------------------------------

let scheduled = false;
function redraw() {
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    let loading = false;
    try { loading = demo.render(); } catch (err) { console.error(err); }
    canvas.style.cursor = demo.cursor();
    // Images decode asynchronously: paint again until they are ready.
    if (loading) setTimeout(redraw, 50);
  });
}

// CSS pixels per editor pixel: below 1 on narrow screens.
let uiScale = 1;
function resize() {
  uiScale = Math.min(1, canvas.clientWidth / MIN_LAYOUT_WIDTH);
  const dpr = window.devicePixelRatio || 1;
  demo.resize(canvas.clientWidth / uiScale, canvas.clientHeight / uiScale, dpr * uiScale);
  redraw();
}
resize();
if (!saved) demo.zoom_to_fit();
new ResizeObserver(resize).observe(canvas);
$("loading").hidden = true;
canvas.focus({ preventScroll: true });

// ----- after every input --------------------------------------------------

let autosaveTimer;
let quotaWarned = false;
function after() {
  const request = demo.take_request();
  if (request === "image") $("pick-image").click();
  if (request === "export") toggleExportMenu(true);

  const dark = demo.is_dark();
  if ((document.documentElement.dataset.theme === "dark") !== dark) {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    store("theme", dark ? "dark" : "light");
  }

  // Typed text goes to the hidden textarea only while a text is edited,
  // so phones show their keyboard just then.
  const editing = demo.is_editing_text();
  const target = editing ? keys : canvas;
  if (document.activeElement !== target) target.focus({ preventScroll: true });

  if (demo.is_modified()) {
    clearTimeout(autosaveTimer);
    autosaveTimer = setTimeout(autosave, AUTOSAVE_DELAY);
  }
  redraw();
}

function autosave() {
  if (!demo.is_modified()) return;
  if (store("scene", demo.to_json())) {
    demo.mark_saved();
  } else if (!quotaWarned) {
    quotaWarned = true;
    notice("El navegador no tiene espacio para guardar este dibujo. Usa «Guardar» para descargarlo.");
  }
}

// ----- pointer ------------------------------------------------------------

function point(e) {
  const rect = canvas.getBoundingClientRect();
  return [(e.clientX - rect.left) / uiScale, (e.clientY - rect.top) / uiScale];
}
const ctrlOf = (e) => e.ctrlKey || e.metaKey;
const pressureOf = (e) => (e.pointerType === "pen" ? e.pressure : undefined);

let lastPress = null;
canvas.addEventListener("pointerdown", (e) => {
  if (e.button > 2) return;
  toggleExportMenu(false);
  canvas.setPointerCapture(e.pointerId);
  const [x, y] = point(e);
  const now = performance.now();
  const double = e.button === 0 && lastPress && now - lastPress.time <= DOUBLE_CLICK_TIME
    && Math.hypot(x - lastPress.x, y - lastPress.y) <= DOUBLE_CLICK_DISTANCE;
  if (double) {
    lastPress = null;
    demo.double_click(x, y);
  } else {
    if (e.button === 0) lastPress = { time: now, x, y };
    demo.pointer_down(x, y, e.button, e.shiftKey, ctrlOf(e), e.altKey, pressureOf(e));
  }
  e.preventDefault();
  after();
});

canvas.addEventListener("pointermove", (e) => {
  // Every intermediate position, for smooth freehand strokes.
  const events = e.getCoalescedEvents?.() ?? [];
  for (const ev of events.length ? events : [e]) {
    const [x, y] = point(ev);
    demo.pointer_move(x, y, e.shiftKey, ctrlOf(e), e.altKey, pressureOf(ev));
  }
  redraw();
});

function release(e) {
  if (e.button > 2) return;
  const [x, y] = point(e);
  demo.pointer_up(x, y, Math.max(e.button, 0), e.shiftKey, ctrlOf(e), e.altKey);
  after();
}
canvas.addEventListener("pointerup", release);
canvas.addEventListener("pointercancel", release);
canvas.addEventListener("contextmenu", (e) => e.preventDefault());

canvas.addEventListener("wheel", (e) => {
  e.preventDefault();
  const unit = e.deltaMode === 1 ? WHEEL_LINE : e.deltaMode === 2 ? canvas.clientHeight : 1;
  const [x, y] = point(e);
  // The editor expects positive deltas when scrolling up or left.
  demo.wheel(-e.deltaX * unit / uiScale, -e.deltaY * unit / uiScale, x, y, e.shiftKey, ctrlOf(e), e.altKey);
  redraw();
}, { passive: false });

// ----- keyboard -----------------------------------------------------------

document.addEventListener("keydown", (e) => {
  if (e.target !== canvas && e.target !== keys) return;
  if (e.isComposing || e.keyCode === 229) return;
  const ctrl = ctrlOf(e);
  if (ctrl && !e.altKey) {
    const key = e.key.toLowerCase();
    if (key === "s") { e.preventDefault(); saveDrawing(); return; }
    if (key === "o") { e.preventDefault(); $("pick-drawing").click(); return; }
    // Copy, cut and paste arrive as clipboard events.
    if (key === "c" || key === "x" || key === "v") return;
  }
  const known = demo.key_down(e.key, e.shiftKey, ctrl, e.altKey);
  // Shortcuts must not reach the browser (Ctrl++ would zoom the page);
  // plain typing must reach the textarea, which turns it into text.
  if (ctrl || e.key === "Tab" || (known && e.target === canvas)) e.preventDefault();
  after();
});

document.addEventListener("keyup", (e) => {
  if (e.target !== canvas && e.target !== keys) return;
  demo.key_up(e.key);
  after();
});

keys.addEventListener("input", (e) => {
  if (e.inputType === "insertText" && e.data) demo.text_input(e.data);
  if (!e.isComposing) keys.value = "";
  after();
});
keys.addEventListener("compositionend", (e) => {
  if (e.data) demo.text_input(e.data);
  keys.value = "";
  after();
});

// ----- clipboard ----------------------------------------------------------

document.addEventListener("copy", (e) => {
  const text = demo.copy();
  if (text != null) {
    e.clipboardData.setData("text/plain", text);
    e.preventDefault();
  }
});
document.addEventListener("cut", (e) => {
  const text = demo.cut();
  if (text != null) {
    e.clipboardData.setData("text/plain", text);
    e.preventDefault();
  }
  after();
});
document.addEventListener("paste", (e) => {
  const image = [...e.clipboardData.files].find((f) => f.type.startsWith("image/"));
  if (image) {
    insertImage(image);
  } else {
    demo.paste(e.clipboardData.getData("text/plain"));
  }
  e.preventDefault();
  after();
});

// ----- files --------------------------------------------------------------

function readAs(file, method) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader[method](file);
  });
}

async function insertImage(file) {
  try {
    const url = await readAs(file, "readAsDataURL");
    const img = new Image();
    img.src = url;
    await img.decode();
    demo.insert_image(url, file.type || "image/png", img.naturalWidth, img.naturalHeight);
    after();
  } catch {
    notice("No se pudo leer la imagen.");
  }
}

$("pick-image").addEventListener("change", (e) => {
  const file = e.target.files[0];
  e.target.value = "";
  if (file) insertImage(file);
});

$("pick-drawing").addEventListener("change", async (e) => {
  const file = e.target.files[0];
  e.target.value = "";
  if (!file) return;
  try {
    demo.load_json(await readAs(file, "readAsText"));
    store("scene", demo.to_json());
    after();
  } catch {
    notice("Ese archivo no es un dibujo de Excalidraw válido.");
  }
});

function download(blob, name) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

function saveDrawing() {
  download(new Blob([demo.to_json()], { type: "application/json" }), "dibujo.excalidraw");
}

async function exportPng() {
  const out = document.createElement("canvas");
  // Wait briefly for images that are still decoding.
  for (let i = 0; i < 20 && demo.render_export(out); i++) {
    await new Promise((r) => setTimeout(r, 100));
  }
  out.toBlob((blob) => blob ? download(blob, "dibujo.png") : notice("No se pudo exportar la imagen."), "image/png");
}

function toggleExportMenu(open) {
  const menu = $("export-menu");
  const show = open ?? menu.hidden;
  menu.hidden = !show;
  $("export").setAttribute("aria-expanded", String(show));
}

$("new").addEventListener("click", () => {
  if (!confirm("¿Empezar un dibujo nuevo? El actual se perderá si no lo guardaste.")) return;
  demo.clear();
  store("scene", demo.to_json());
  after();
});
$("open").addEventListener("click", () => $("pick-drawing").click());
$("save").addEventListener("click", saveDrawing);
$("export").addEventListener("click", () => toggleExportMenu());
$("export-png").addEventListener("click", () => { toggleExportMenu(false); exportPng(); });
$("export-svg").addEventListener("click", () => {
  toggleExportMenu(false);
  download(new Blob([demo.to_svg()], { type: "image/svg+xml" }), "dibujo.svg");
});

// Drop .excalidraw files or images onto the page.
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", async (e) => {
  e.preventDefault();
  const file = e.dataTransfer.files[0];
  if (!file) return;
  if (file.type.startsWith("image/")) return insertImage(file);
  try {
    demo.load_json(await readAs(file, "readAsText"));
    store("scene", demo.to_json());
    after();
  } catch {
    notice("Solo se pueden abrir dibujos .excalidraw o imágenes.");
  }
});

// Save before leaving, even inside the autosave delay.
window.addEventListener("pagehide", autosave);
