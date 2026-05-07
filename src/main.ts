import "@fontsource-variable/inter";
import "@fontsource-variable/source-serif-4";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface UsageReport {
  available: boolean;
  needs_setup: boolean;
  error: string | null;
  five_hour_pct: number | null;
  five_hour_reset_at: string | null;
  org_id: string | null;
}

type Position =
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right"
  | "center";

interface ConfigSummary {
  session_key: string | null;
  org_id: string | null;
  position: Position;
  shortcut: string;
  shortcut_supported: boolean;
}

const $ = <T extends HTMLElement = HTMLElement>(sel: string) =>
  document.querySelector(sel) as T;

const panel = $("#panel");
const percentNum = $("#percent-num");
const timeFigure = $("#time-figure");
const progressFill = $("#progress-fill");
const emptyOverlay = $("#empty-overlay");
const errorOverlay = $("#error-overlay");
const errorBody = $("#error-body");
const setupOverlay = $("#setup-overlay");
const setupForm = $<HTMLFormElement>("#setup-form");
const setupInput = $<HTMLInputElement>("#setup-input");
const setupCurrent = $("#setup-current");
const setupClearBtn = $<HTMLButtonElement>("#setup-clear");
const setupCancelBtn = $<HTMLButtonElement>("#setup-cancel");
const refreshBtn = $<HTMLButtonElement>("#refresh");
const settingsBtn = $<HTMLButtonElement>("#settings");
const closeBtn = $<HTMLButtonElement>("#close");
const positionGrid = $("#position-grid");
const positionCells = positionGrid.querySelectorAll<HTMLButtonElement>(
  ".position-cell",
);
const shortcutInput = $<HTMLButtonElement>("#shortcut-input");
const shortcutHint = $("#shortcut-hint");

function unitSpan(text: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.className = "unit";
  span.textContent = text;
  return span;
}

function setTimeFigure(resetIso: string | null): void {
  timeFigure.replaceChildren();
  const reset = resetIso ? Date.parse(resetIso) : NaN;
  if (!isFinite(reset)) {
    timeFigure.append("—");
    return;
  }
  const mins = Math.max(0, Math.floor((reset - Date.now()) / 60_000));
  const h = Math.floor(mins / 60);
  const m = mins % 60;

  if (h > 0) {
    timeFigure.append(String(h));
    timeFigure.appendChild(unitSpan("h"));
    timeFigure.append(` ${m}`);
    timeFigure.appendChild(unitSpan("m"));
  } else {
    timeFigure.append(String(m));
    timeFigure.appendChild(unitSpan("m"));
  }
}

function showOverlay(which: "none" | "empty" | "error" | "setup", msg?: string) {
  emptyOverlay.hidden = which !== "empty";
  errorOverlay.hidden = which !== "error";
  setupOverlay.hidden = which !== "setup";
  if (which === "error") {
    errorBody.textContent =
      msg && msg.length > 0 ? msg : "(no error message returned)";
  }
}

// Last successful render — used by closeSetup to restore the underlying
// overlay state when the user dismisses the setup form.
let lastReport: UsageReport | null = null;

function render(r: UsageReport) {
  lastReport = r;
  if (r.needs_setup) {
    showOverlay("empty");
    panel.dataset.state = "idle";
    return;
  }
  if (!r.available) {
    showOverlay("error", r.error ?? "claude.ai request failed");
    panel.dataset.state = "error";
    return;
  }
  showOverlay("none");
  panel.dataset.state = "ready";

  if (r.five_hour_pct != null) {
    percentNum.textContent = String(Math.min(Math.round(r.five_hour_pct), 999));
    progressFill.style.setProperty(
      "--pct",
      `${Math.min(r.five_hour_pct, 100)}%`,
    );
  } else {
    percentNum.textContent = "··";
    progressFill.style.setProperty("--pct", `0%`);
  }

  setTimeFigure(r.five_hour_reset_at);
}

let inFlight = false;

async function refresh() {
  if (inFlight) return;
  inFlight = true;
  refreshBtn.classList.remove("is-spinning");
  void refreshBtn.offsetWidth;
  refreshBtn.classList.add("is-spinning");
  try {
    const r = await invoke<UsageReport>("get_usage");
    render(r);
  } catch (e) {
    console.error("[claude-hourglass] invoke error:", e);
    const detail =
      e instanceof Error
        ? `${e.name}: ${e.message}`
        : typeof e === "string"
          ? e
          : JSON.stringify(e);
    showOverlay("error", `invoke threw:\n${detail}`);
    panel.dataset.state = "error";
  } finally {
    inFlight = false;
  }
}

async function openSetup() {
  let cfg: ConfigSummary;
  try {
    cfg = await invoke<ConfigSummary>("get_config");
  } catch (e) {
    showOverlay(
      "error",
      `couldn't load settings: ${e instanceof Error ? e.message : String(e)}`,
    );
    return;
  }
  if (cfg.session_key) {
    setupCurrent.textContent = cfg.org_id
      ? `connected · ${cfg.org_id.slice(0, 8)}…`
      : "connected · org pending";
  } else {
    setupCurrent.textContent = "not connected";
  }
  setupInput.value = "";
  applyPositionUI(cfg.position);
  applyShortcutUI(cfg);
  // Route through showOverlay so the empty / error overlay underneath
  // is hidden — otherwise the setup form renders behind them.
  showOverlay("setup");
  requestAnimationFrame(() => setupInput.focus());
}

function closeSetup() {
  // Re-render based on current state so we restore the right overlay
  // (empty / error / none) instead of leaving whatever was underneath.
  setupOverlay.hidden = true;
  cancelShortcutCapture();
  if (lastReport) render(lastReport);
}

async function submitSetup() {
  const key = setupInput.value.trim();
  if (key.length === 0) {
    setupInput.focus();
    return;
  }
  try {
    await invoke("set_session_key", { key });
    setupInput.value = ""; // scrub the cleartext value out of the DOM
    closeSetup();
    await refresh();
  } catch (e) {
    showOverlay(
      "error",
      `couldn't save session key: ${e instanceof Error ? e.message : String(e)}`,
    );
  }
}

async function clearSetup() {
  try {
    await invoke("set_session_key", { key: null });
    setupInput.value = "";
    closeSetup();
    await refresh();
  } catch (e) {
    showOverlay(
      "error",
      `couldn't clear session key: ${e instanceof Error ? e.message : String(e)}`,
    );
  }
}

function playShowAnimation() {
  // Snap opacity to 0 with the transition disabled so the next class
  // add runs a clean full-range 0 → 1 fade. Without this, a class
  // toggle starting from opacity 1 would only progress ~9% before
  // reversing, reading as no animation.
  panel.style.transition = "none";
  panel.classList.remove("is-shown");
  void panel.offsetWidth; // force the snap to commit
  panel.style.transition = "";
  // Double rAF: WebKit needs two frames to register the initial state
  // before treating the class change as a transition. With a single
  // rAF, the popup snaps to the .is-shown state on first show under
  // macOS.
  requestAnimationFrame(() => {
    requestAnimationFrame(() => panel.classList.add("is-shown"));
  });
}

// Fade out, then ask the OS to hide the window once the opacity
// transition finishes. Using transitionend with `{ once: true }` makes
// the chain self-cleaning even if the user rapidly toggles.
function fadeOutAndHide() {
  if (!panel.classList.contains("is-shown")) {
    // Already hidden / mid-fade — just hide.
    void getCurrentWindow().hide();
    return;
  }
  const onEnd = (e: TransitionEvent) => {
    if (e.propertyName !== "opacity") return;
    panel.removeEventListener("transitionend", onEnd);
    void getCurrentWindow().hide();
  };
  panel.addEventListener("transitionend", onEnd);
  panel.classList.remove("is-shown");
}

// ── Shortcut: capture + format helpers ──────────────────────────

function codeToKey(code: string): string | null {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  const map: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
    Escape: "Escape",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Semicolon: ";",
    Quote: "'",
    Backquote: "`",
    Backslash: "\\",
    Comma: ",",
    Period: ".",
    Slash: "/",
  };
  if (map[code]) return map[code];
  if (/^F\d+$/.test(code)) return code;
  return null;
}

function formatShortcut(s: string): string {
  return s.split("+").join(" + ");
}

let capturingShortcut = false;
let currentShortcut = "Ctrl+Alt+L";

function applyShortcutUI(cfg: ConfigSummary) {
  currentShortcut = cfg.shortcut;
  shortcutInput.textContent = formatShortcut(currentShortcut);
  shortcutInput.classList.remove("is-capturing");
  shortcutInput.disabled = !cfg.shortcut_supported;
  if (cfg.shortcut_supported) {
    shortcutHint.hidden = true;
    shortcutHint.replaceChildren();
  } else {
    shortcutHint.hidden = false;
    shortcutHint.replaceChildren();
    shortcutHint.append("Wayland blocks app-level shortcuts. Bind one in your DE → run ");
    const cmd = document.createElement("b");
    cmd.textContent = "claude-hourglass --toggle";
    shortcutHint.appendChild(cmd);
    shortcutHint.append(".");
  }
}

function startShortcutCapture() {
  if (shortcutInput.disabled || capturingShortcut) return;
  capturingShortcut = true;
  shortcutInput.textContent = "press a combo…";
  shortcutInput.classList.add("is-capturing");
  // If the user clicks anywhere outside the shortcut field while we're
  // armed, cancel — otherwise capture stays armed and the next keystroke
  // (e.g. typing into the cookie input) gets stolen as the new shortcut.
  document.addEventListener("pointerdown", outsideClickCancel, true);
}

function outsideClickCancel(e: PointerEvent) {
  if (!capturingShortcut) return;
  const target = e.target as Node | null;
  if (target && shortcutInput.contains(target)) return;
  cancelShortcutCapture();
}

function cancelShortcutCapture() {
  capturingShortcut = false;
  shortcutInput.classList.remove("is-capturing");
  shortcutInput.textContent = formatShortcut(currentShortcut);
  document.removeEventListener("pointerdown", outsideClickCancel, true);
}

async function handleShortcutKeydown(e: KeyboardEvent) {
  if (!capturingShortcut) return;

  // Lone modifier presses: swallow but wait for the user to add a real key.
  const lone = [
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "ShiftLeft",
    "ShiftRight",
    "MetaLeft",
    "MetaRight",
  ];
  if (lone.includes(e.code)) {
    e.preventDefault();
    e.stopImmediatePropagation();
    return;
  }

  if (e.key === "Escape") {
    e.preventDefault();
    e.stopImmediatePropagation();
    cancelShortcutCapture();
    return;
  }

  const key = codeToKey(e.code);
  // Unrecognised keys (Tab, dead keys, etc.) — let them pass through so
  // the user isn't trapped in capture mode and can still tab around.
  if (!key) return;

  // Real key: swallow so it doesn't double-fire elsewhere.
  // stopImmediatePropagation also blocks the bubble-phase Escape/Ctrl+R
  // listener registered later on `document`.
  e.preventDefault();
  e.stopImmediatePropagation();

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Cmd");
  if (parts.length === 0) {
    // Bare key — refuse, would conflict with everything.
    return;
  }
  parts.push(key);
  const newShortcut = parts.join("+");

  capturingShortcut = false;
  shortcutInput.classList.remove("is-capturing");
  try {
    await invoke("set_shortcut", { shortcut: newShortcut });
    currentShortcut = newShortcut;
    shortcutInput.textContent = formatShortcut(newShortcut);
  } catch (err) {
    shortcutInput.textContent = formatShortcut(currentShortcut);
    showOverlay(
      "error",
      `couldn't set shortcut: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

// ── Position picker ─────────────────────────────────────────────

const VALID_POSITIONS: ReadonlySet<Position> = new Set([
  "top-left",
  "top-right",
  "bottom-left",
  "bottom-right",
  "center",
]);

function isValidPosition(p: string | undefined): p is Position {
  return p !== undefined && VALID_POSITIONS.has(p as Position);
}

function applyPositionUI(pos: Position) {
  positionCells.forEach((cell) => {
    cell.classList.toggle("is-active", cell.dataset.position === pos);
  });
}

async function handlePositionClick(cell: HTMLButtonElement) {
  const pos = cell.dataset.position;
  if (!isValidPosition(pos)) return;
  // Don't optimistically update: if the Rust call fails the grid would lie.
  // Apply visually only after persistence succeeds.
  try {
    await invoke("set_position", { position: pos });
    applyPositionUI(pos);
  } catch (err) {
    showOverlay(
      "error",
      `couldn't set position: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  refreshBtn.addEventListener("click", refresh);
  closeBtn.addEventListener("click", fadeOutAndHide);
  settingsBtn.addEventListener("click", openSetup);
  setupCancelBtn.addEventListener("click", closeSetup);
  setupClearBtn.addEventListener("click", clearSetup);
  setupForm.addEventListener("submit", (e) => {
    e.preventDefault();
    submitSetup();
  });
  emptyOverlay.addEventListener("click", openSetup);

  // Position grid: each cell saves the chosen position.
  positionCells.forEach((cell) => {
    cell.addEventListener("click", () => handlePositionClick(cell));
  });

  // Shortcut input: click → capture next combo.
  shortcutInput.addEventListener("click", startShortcutCapture);

  // Capture listener uses the capture phase so it intercepts before
  // the regular keydown handler below would catch Escape / Ctrl+R.
  document.addEventListener("keydown", handleShortcutKeydown, true);

  document.addEventListener("keydown", (e) => {
    if (capturingShortcut) return;
    if (e.key === "Escape") {
      if (!setupOverlay.hidden) closeSetup();
      else fadeOutAndHide();
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "r") {
      e.preventDefault();
      refresh();
    }
  });

  await listen("popup-shown", () => {
    playShowAnimation();
    refresh();
  });
  await listen("popup-hide", () => fadeOutAndHide());
  await listen("refresh-usage", () => refresh());

  setInterval(() => {
    if (document.visibilityState === "visible") refresh();
  }, 60_000);

  playShowAnimation();
  refresh();
});
