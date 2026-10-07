import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { config } from "../stores.js";
import { locale } from "../i18n.js";
import CaptureStatus from "./CaptureStatus.svelte";
import { captureHidden } from "../api.js";

vi.mock("../api.js", () => ({ captureHidden: vi.fn() }));

describe("capture status", () => {
  let component;
  let target;

  beforeEach(() => {
    vi.useFakeTimers();
    captureHidden.mockReset();
    config.set(null);
    locale.set("en");
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

  function render() {
    component = mount(CaptureStatus, { target });
    flushSync();
  }

  it("shows checking until Windows returns the actual state", async () => {
    let resolve;
    captureHidden.mockImplementation(() => new Promise((done) => { resolve = done; }));
    render();
    expect(target.textContent).toContain("Checking capture status");
    resolve(true);
    await settle();
    expect(target.textContent).toContain("Hidden from capture");
  });

  it("uses the queried state even when the saved preference says hidden", async () => {
    config.set({ hideSelf: true });
    captureHidden.mockResolvedValue(false);
    render();
    await settle();
    expect(target.textContent).toContain("Visible to capture");
  });

  it("refreshes after settings changes, polling and focus", async () => {
    captureHidden.mockResolvedValue(false);
    render();
    await settle();
    captureHidden.mockResolvedValue(true);
    config.set({ hideSelf: true });
    await settle();
    expect(target.textContent).toContain("Hidden from capture");
    captureHidden.mockResolvedValue(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(target.textContent).toContain("Visible to capture");
    captureHidden.mockResolvedValue(true);
    window.dispatchEvent(new Event("focus"));
    await settle();
    expect(target.textContent).toContain("Hidden from capture");
  });

  it("shows unavailable on errors and recovers on the next successful query", async () => {
    captureHidden.mockResolvedValue(true);
    render();
    await settle();
    captureHidden.mockRejectedValue(new Error("Windows rejected the query"));
    await vi.advanceTimersByTimeAsync(1000);
    expect(target.textContent).toContain("Capture status unavailable");
    captureHidden.mockResolvedValue(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(target.textContent).toContain("Visible to capture");
  });

  it("ignores older queries that complete after a newer result", async () => {
    let resolve;
    captureHidden.mockImplementationOnce(() => new Promise((done) => { resolve = done; }))
      .mockResolvedValue(false);
    render();
    window.dispatchEvent(new Event("focus"));
    await settle();
    resolve(true);
    await settle();
    expect(target.textContent).toContain("Visible to capture");
  });

  it("stops refreshing when removed", async () => {
    captureHidden.mockResolvedValue(true);
    render();
    await settle();
    await unmount(component);
    component = undefined;
    captureHidden.mockClear();
    config.set({ hideSelf: false });
    window.dispatchEvent(new Event("focus"));
    await vi.advanceTimersByTimeAsync(2000);
    expect(captureHidden).not.toHaveBeenCalled();
  });

  it("translates the badge and describes the current app window", async () => {
    captureHidden.mockResolvedValue(true);
    locale.set("fr");
    render();
    await settle();
    const badge = target.querySelector('[role="status"]');
    expect(badge.textContent).toContain("Masquée des captures");
    expect(badge.title).toContain("cette fenêtre HideMyWindows");
  });
});
