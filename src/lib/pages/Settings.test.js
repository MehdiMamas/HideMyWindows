import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { get } from "svelte/store";
import Settings from "./Settings.svelte";
import { config, launchProtectionStatus, toasts } from "../stores.js";
import { locale } from "../i18n.js";
import * as api from "../api.js";

vi.mock("../api.js", () => ({ saveConfig: vi.fn(), getConfigDir: vi.fn(), resetSettings: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));

describe("ordinary-launch protection controls", () => {
  let component;
  let target;
  const base = {
    theme: "system", language: "en", normalLaunchProtection: true,
    hideSelf: false, hideNotificationToasts: false, closeToTray: true,
    minimizeToTray: false, startWithWindows: false,
    normalLaunchPauseUntilMs: 0, ruleReapplyIntervalMs: 1000,
    processPollIntervalMs: 1000, windowRules: [{ id: "keep-rule" }],
    quickLaunch: [{ id: "keep-launch" }],
  };

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-10T10:00:00Z"));
    vi.resetAllMocks();
    api.saveConfig.mockResolvedValue();
    locale.set("en");
    config.set(structuredClone(base));
    toasts.set([]);
    launchProtectionStatus.set({ phase: "active", pauseUntilMs: 0, error: null });
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
    component = mount(Settings, { target });
    flushSync();
  }

  function button(label) {
    return [...target.querySelectorAll("button")].find((el) => el.textContent.trim() === label);
  }

  it("defaults to a 30 minute pause and waits for backend status", async () => {
    render();
    button("Pause").click();
    await settle();
    const until = Date.now() + 30 * 60000;
    expect(api.saveConfig).toHaveBeenCalledWith({ ...base, normalLaunchPauseUntilMs: until });
    expect(get(config).windowRules).toEqual(base.windowRules);
    expect(get(config).quickLaunch).toEqual(base.quickLaunch);
    expect(target.textContent).not.toContain("Protection before apps appear is paused.");
    launchProtectionStatus.set({ phase: "paused", pauseUntilMs: until, error: null });
    flushSync();
    expect(target.textContent).toContain("Protection before apps appear is paused.");
    expect(target.textContent).toContain("30 min remaining");
  });

  it("supports all durations and manual resume", async () => {
    render();
    const select = target.querySelector("#pause-duration");
    expect([...select.options].map((o) => o.value)).toEqual(["15", "30", "60", "120"]);
    select.value = "120";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    await settle();
    button("Pause").click();
    await settle();
    expect(get(config).normalLaunchPauseUntilMs).toBe(Date.now() + 120 * 60000);
    button("Resume now").click();
    await settle();
    expect(get(config).normalLaunchPauseUntilMs).toBe(0);
    expect(get(config).normalLaunchProtection).toBe(true);
  });

  it("turning off cancels a pause and prevents new pauses until enabled", async () => {
    config.set({ ...base, normalLaunchPauseUntilMs: Date.now() + 60000 });
    render();
    target.querySelector(".protection-switch").click();
    await settle();
    expect(get(config).normalLaunchProtection).toBe(false);
    expect(get(config).normalLaunchPauseUntilMs).toBe(0);
    expect(button("Pause").disabled).toBe(true);
    target.querySelector(".protection-switch").click();
    await settle();
    expect(get(config).normalLaunchProtection).toBe(true);
    expect(button("Pause").disabled).toBe(false);
  });

  it("does not show a successful pause or disable after a save failure", async () => {
    api.saveConfig.mockRejectedValue(new Error("Disk full"));
    render();
    button("Pause").click();
    await settle();
    expect(get(config).normalLaunchPauseUntilMs).toBe(0);
    target.querySelector(".protection-switch").click();
    await settle();
    expect(get(config).normalLaunchProtection).toBe(true);
    expect(target.querySelector(".protection-switch").getAttribute("aria-checked")).toBe("true");
    expect(get(toasts).some((toast) => toast.message.includes("Disk full"))).toBe(true);
  });

  it("restores a saved pause and shows pending resume until the backend confirms it", async () => {
    const until = Date.now() + 60000;
    config.set({ ...base, normalLaunchPauseUntilMs: until });
    launchProtectionStatus.set({ phase: "paused", pauseUntilMs: until, error: null });
    render();
    expect(button("Resume now")).toBeTruthy();
    await vi.advanceTimersByTimeAsync(60000);
    flushSync();
    expect(target.textContent).toContain("Applying protection settings");
    expect(api.saveConfig).not.toHaveBeenCalled(); // Expiry is owned by Rust, even in the tray.
    launchProtectionStatus.set({ phase: "active", pauseUntilMs: 0, error: null });
    flushSync();
    expect(target.textContent).toContain("Protection before apps appear is active.");
    expect(button("Resume now")).toBeUndefined();
  });

  it("reports installation failure rather than claiming protection is active", () => {
    launchProtectionStatus.set({ phase: "error", pauseUntilMs: 0, error: "32-bit gate failed" });
    render();
    expect(target.textContent).toContain("could not be fully applied");
    expect(target.textContent).toContain("32-bit gate failed");
  });

  it("translates the new controls and cleans up page timers", async () => {
    locale.set("fr");
    render();
    expect(target.textContent).toContain("Suspendre pendant");
    expect(target.textContent).toContain("La protection avant l’affichage est active.");
    await unmount(component);
    component = undefined;
    expect(vi.getTimerCount()).toBe(0);
  });
});
