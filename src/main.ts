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

interface ConfigSummary {
  session_key: string | null;
  org_id: string | null;
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

function render(r: UsageReport) {
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
    console.log("[claude-usage-tray] get_usage =>", r);
    render(r);
  } catch (e) {
    console.error("[claude-usage-tray] invoke error:", e);
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
  const cfg = await invoke<ConfigSummary>("get_config");
  if (cfg.session_key) {
    setupCurrent.textContent = cfg.org_id
      ? `connected · ${cfg.org_id.slice(0, 8)}…`
      : "connected · org pending";
  } else {
    setupCurrent.textContent = "not connected";
  }
  setupInput.value = "";
  setupOverlay.hidden = false;
  requestAnimationFrame(() => setupInput.focus());
}

function closeSetup() {
  setupOverlay.hidden = true;
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
  panel.classList.remove("is-shown");
  void panel.offsetWidth;
  requestAnimationFrame(() => panel.classList.add("is-shown"));
}

window.addEventListener("DOMContentLoaded", async () => {
  refreshBtn.addEventListener("click", refresh);
  closeBtn.addEventListener("click", () => getCurrentWindow().hide());
  settingsBtn.addEventListener("click", openSetup);
  setupCancelBtn.addEventListener("click", closeSetup);
  setupClearBtn.addEventListener("click", clearSetup);
  setupForm.addEventListener("submit", (e) => {
    e.preventDefault();
    submitSetup();
  });
  emptyOverlay.addEventListener("click", openSetup);

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (!setupOverlay.hidden) closeSetup();
      else getCurrentWindow().hide();
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
  await listen("refresh-usage", () => refresh());

  setInterval(() => {
    if (document.visibilityState === "visible") refresh();
  }, 60_000);

  playShowAnimation();
  refresh();
});
