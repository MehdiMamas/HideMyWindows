import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { config, toasts } from "./stores.js";
import { connectTrayEvents } from "./trayEvents.js";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("native tray events", () => {
  let handlers;
  let stops;
  beforeEach(() => {
    vi.resetAllMocks();
    handlers = {};
    stops = {};
    config.set(null);
    toasts.set([]);
    listen.mockImplementation(async (name, callback) => {
      handlers[name] = callback;
      stops[name] = vi.fn();
      return stops[name];
    });
  });

  it("updates pause and resume in the open window while preserving unsaved edits", async () => {
    const events = await connectTrayEvents();
    config.set({ normalLaunchProtection: true, normalLaunchPauseUntilMs: 0, processPollIntervalMs: 2500, windowRules: [{ id: "unsaved" }] });
    handlers["config-changed"]({ payload: { normalLaunchProtection: true, normalLaunchPauseUntilMs: 1800000, processPollIntervalMs: 1000, windowRules: [] } });
    expect(events.receivedConfig()).toBe(true);
    expect(get(config).normalLaunchPauseUntilMs).toBe(1800000);
    expect(get(config).processPollIntervalMs).toBe(2500);
    expect(get(config).windowRules).toEqual([{ id: "unsaved" }]);
    handlers["config-changed"]({ payload: { normalLaunchProtection: true, normalLaunchPauseUntilMs: 0 } });
    expect(get(config).normalLaunchPauseUntilMs).toBe(0);
    events.stop();
    expect(stops["config-changed"]).toHaveBeenCalledOnce();
    expect(stops["tray-action-error"]).toHaveBeenCalledOnce();
  });

  it("retains a tray change that arrives before the initial config read", async () => {
    const events = await connectTrayEvents();
    const payload = { normalLaunchProtection: false, normalLaunchPauseUntilMs: 0, theme: "light" };
    handlers["config-changed"]({ payload });
    expect(get(config)).toEqual(payload);
    expect(events.receivedConfig()).toBe(true);
    events.stop();
  });

  it("reports failed saves without inventing a config change", async () => {
    const events = await connectTrayEvents();
    handlers["tray-action-error"]({ payload: "Could not save settings: disk full" });
    expect(get(config)).toBeNull();
    expect(get(toasts)[0]).toMatchObject({ severity: "error", message: "Could not save settings: disk full" });
    expect(events.receivedConfig()).toBe(false);
    events.stop();
  });
});
