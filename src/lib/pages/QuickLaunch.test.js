import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { get } from "svelte/store";
import QuickLaunch from "./QuickLaunch.svelte";
import { config, toasts } from "../stores.js";
import { locale } from "../i18n.js";
import * as api from "../api.js";

vi.mock("../api.js", () => ({
  saveConfig: vi.fn(),
  quickLaunch: vi.fn(),
  listInstalledApps: vi.fn(),
  createLaunchShortcut: vi.fn(),
  removeLaunchShortcut: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const WHATSAPP = "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App";
const SHORTCUT = "C:\\Start\\WhatsApp HideMyWindows.lnk";
const APPS = [
  { name: "Discord", appId: "", path: "C:\\Discord\\Discord.exe", arguments: "", icon: null },
  { name: "WhatsApp", appId: WHATSAPP, path: "", arguments: "", icon: null },
];

describe("quick launch app picker", () => {
  let component;
  let target;

  beforeEach(() => {
    vi.resetAllMocks();
    api.saveConfig.mockResolvedValue();
    api.listInstalledApps.mockResolvedValue(APPS);
    api.createLaunchShortcut.mockResolvedValue(SHORTCUT);
    api.removeLaunchShortcut.mockResolvedValue();
    locale.set("en");
    config.set({ quickLaunch: [] });
    toasts.set([]);
    target = document.createElement("div");
    document.body.append(target);
  });

  afterEach(async () => {
    if (component) await unmount(component);
    component = undefined;
    target.remove();
  });

  async function settle() {
    for (let i = 0; i < 5; i++) await Promise.resolve();
    flushSync();
  }

  async function render() {
    component = mount(QuickLaunch, { target });
    flushSync();
    await settle();
  }

  function button(text) {
    return [...target.querySelectorAll("button")].find((el) => el.textContent.trim() === text);
  }

  function pickerApp(name) {
    return [...target.querySelectorAll(".picker .app")].find((el) => el.textContent.includes(name));
  }

  async function search(text) {
    const input = target.querySelector(".picker input");
    input.value = text;
    input.dispatchEvent(new Event("input"));
    flushSync();
  }

  it("adds a searched Store app with a Start menu shortcut", async () => {
    await render();
    button("+ Add app").click();
    await settle();
    await search("whats");
    expect(target.querySelectorAll(".app")).toHaveLength(1);
    expect(target.querySelector(".app").textContent).toContain("Microsoft Store app");

    pickerApp("WhatsApp").click();
    await settle();

    expect(api.createLaunchShortcut).toHaveBeenCalledWith("WhatsApp", "", "", WHATSAPP);
    const [entry] = get(config).quickLaunch;
    expect(entry).toMatchObject({ name: "WhatsApp", appId: WHATSAPP, shortcut: SHORTCUT });
    expect(api.saveConfig).toHaveBeenCalled();
    expect(get(toasts)[0].message).toContain("WhatsApp HideMyWindows");
    expect(target.querySelector(".picker")).toBeNull();
  });

  it("skips the shortcut when the toggle is off and does not add an app twice", async () => {
    await render();
    button("+ Add app").click();
    await settle();
    target.querySelector(".picker [role=switch]").click();
    flushSync();
    pickerApp("Discord").click();
    await settle();

    expect(api.createLaunchShortcut).not.toHaveBeenCalled();
    expect(get(config).quickLaunch).toHaveLength(1);
    expect(get(config).quickLaunch[0]).toMatchObject({ path: "C:\\Discord\\Discord.exe", shortcut: "" });

    button("+ Add app").click();
    await settle();
    pickerApp("Discord").click();
    await settle();
    expect(get(config).quickLaunch).toHaveLength(1);
    expect(get(toasts).at(-1).message).toContain("already in the list");
  });

  it("removes the Start menu shortcut with the entry", async () => {
    config.set({
      quickLaunch: [{ id: "a", name: "WhatsApp", path: "", arguments: "", appId: WHATSAPP, shortcut: SHORTCUT }],
    });
    await render();
    button("Remove").click();
    await settle();
    expect(api.removeLaunchShortcut).toHaveBeenCalledWith(SHORTCUT);
    expect(get(config).quickLaunch).toEqual([]);
  });

  it("launches a Store entry by its app ID", async () => {
    api.quickLaunch.mockResolvedValue(42);
    config.set({
      quickLaunch: [{ id: "a", name: "WhatsApp", path: "", arguments: "", appId: WHATSAPP, shortcut: "" }],
    });
    await render();
    target.querySelector(".launch").click();
    await settle();
    expect(api.quickLaunch).toHaveBeenCalledWith("", "", WHATSAPP);
  });
});
