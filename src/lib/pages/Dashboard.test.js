import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { locale } from "../i18n.js";
import Dashboard from "./Dashboard.svelte";
import * as api from "../api.js";

vi.mock("../api.js", () => ({
  listProcesses: vi.fn(), listWindows: vi.fn(), captureStatuses: vi.fn(),
  hideProcess: vi.fn(), hideWindow: vi.fn(),
}));

describe("target capture badges", () => {
  let component;
  let target;
  const visible = { processes: { 11: "visible", 22: "visible", 33: "visible" }, windows: { 101: "visible", 102: "visible" } };
  const hidden = { processes: { 11: "hidden", 22: "partial", 33: "visible" }, windows: { 101: "hidden", 102: "visible" } };

  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    locale.set("en");
    api.listProcesses.mockResolvedValue([
      { pid: 11, name: "private.exe" }, { pid: 22, name: "mixed.exe" },
      { pid: 33, name: "visible.exe" }, { pid: 44, name: "background.exe" },
    ]);
    api.listWindows.mockResolvedValue([
      { hwnd: 101, pid: 11, title: "Private notes", class: "Editor" },
      { hwnd: 102, pid: 33, title: "Public notes", class: "Editor" },
    ]);
    api.captureStatuses.mockResolvedValue(hidden);
    api.hideProcess.mockResolvedValue();
    api.hideWindow.mockResolvedValue();
    target = document.createElement("div");
    document.body.append(target);
  });

  afterEach(async () => {
    if (component) await unmount(component);
    component = undefined;
    target.remove();
    vi.useRealTimers();
  });

  async function settle() {
    await vi.advanceTimersByTimeAsync(0);
    flushSync();
  }

  async function render() {
    component = mount(Dashboard, { target });
    flushSync();
    await settle();
  }

  function row(name) {
    const found = [...target.querySelectorAll(".item")].find((item) => item.querySelector(".name").textContent === name);
    expect(found).toBeTruthy();
    return found;
  }

  async function click(text) {
    const button = [...target.querySelectorAll("button")].find((item) => item.textContent.trim() === text);
    expect(button).toBeTruthy();
    button.click();
    await settle();
  }

  it("marks hidden and partly hidden processes, without marking visible or windowless processes", async () => {
    await render();
    expect(row("private.exe").textContent).toContain("Hidden");
    expect(row("mixed.exe").textContent).toContain("Partly hidden");
    expect(row("visible.exe").querySelector(".badge")).toBeNull();
    expect(row("background.exe").querySelector(".badge")).toBeNull();
  });

  it("puts the badge on the specific hidden window", async () => {
    await render();
    await click("Windows");
    expect(row("Private notes").textContent).toContain("Hidden");
    expect(row("Public notes").querySelector(".badge")).toBeNull();
  });

  it("reads back status after hiding and unhiding a process", async () => {
    api.captureStatuses.mockResolvedValue(visible);
    await render();
    row("private.exe").click();
    await settle();
    api.captureStatuses.mockResolvedValue(hidden);
    await click("Hide all windows");
    expect(api.hideProcess).toHaveBeenCalledWith(11, "hideProcessWindows");
    expect(row("private.exe").textContent).toContain("Hidden");
    api.captureStatuses.mockResolvedValue(visible);
    await click("Unhide all windows");
    expect(api.hideProcess).toHaveBeenCalledWith(11, "unhideProcessWindows");
    expect(row("private.exe").querySelector(".badge")).toBeNull();
  });

  it("reads back the specific window after hide/unhide actions", async () => {
    api.captureStatuses.mockResolvedValue(visible);
    await render();
    await click("Windows");
    row("Private notes").click();
    await settle();
    api.captureStatuses.mockResolvedValue(hidden);
    await click("Hide this window");
    expect(api.hideWindow).toHaveBeenCalledWith(101, "hideWindow");
    expect(row("Private notes").textContent).toContain("Hidden");
    api.captureStatuses.mockResolvedValue(visible);
    await click("Unhide this window");
    expect(api.hideWindow).toHaveBeenCalledWith(101, "unhideWindow");
    expect(row("Private notes").querySelector(".badge")).toBeNull();
  });

  it("does not invent a Hidden badge after a rejected hide action", async () => {
    api.captureStatuses.mockResolvedValue(visible);
    await render();
    row("private.exe").click();
    await settle();
    api.hideProcess.mockRejectedValue(new Error("Access denied"));
    await click("Hide all windows");
    expect(row("private.exe").querySelector(".badge")).toBeNull();
  });

  it("refreshes badges for rules and Quick Launch without a manual action", async () => {
    api.captureStatuses.mockResolvedValue(visible);
    await render();
    api.captureStatuses.mockResolvedValue(hidden);
    await vi.advanceTimersByTimeAsync(1000);
    expect(row("private.exe").textContent).toContain("Hidden");
    api.listProcesses.mockResolvedValue([{ pid: 55, name: "launched.exe" }]);
    api.captureStatuses.mockResolvedValue({ processes: { 55: "hidden" }, windows: {} });
    await vi.advanceTimersByTimeAsync(4000);
    expect(row("launched.exe").textContent).toContain("Hidden");
  });

  it("replaces stale Hidden badges with unknown when querying fails", async () => {
    await render();
    api.captureStatuses.mockRejectedValue(new Error("Query failed"));
    await vi.advanceTimersByTimeAsync(1000);
    expect(row("private.exe").textContent).toContain("Status unknown");
    expect(row("private.exe").textContent).not.toContain("Hidden");
    api.captureStatuses.mockResolvedValue(hidden);
    window.dispatchEvent(new Event("focus"));
    await settle();
    expect(row("private.exe").textContent).toContain("Hidden");
  });

  it("ignores an older snapshot that arrives after a newer query", async () => {
    let resolve;
    api.captureStatuses.mockImplementationOnce(() => new Promise((done) => { resolve = done; }))
      .mockResolvedValue(visible);
    await render();
    await vi.advanceTimersByTimeAsync(1000);
    resolve(hidden);
    await settle();
    expect(row("private.exe").querySelector(".badge")).toBeNull();
  });

  it("stops polling and listening for focus when the page is removed", async () => {
    await render();
    await unmount(component);
    component = undefined;
    vi.clearAllMocks();
    window.dispatchEvent(new Event("focus"));
    await vi.advanceTimersByTimeAsync(10000);
    expect(api.captureStatuses).not.toHaveBeenCalled();
    expect(api.listProcesses).not.toHaveBeenCalled();
  });

  it("translates row badges", async () => {
    locale.set("fr");
    await render();
    expect(row("private.exe").textContent).toContain("Masquée");
    expect(row("mixed.exe").textContent).toContain("Partiellement masquée");
  });
});
