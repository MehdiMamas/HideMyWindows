import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { get } from "svelte/store";
import UpdatePrompt from "./UpdatePrompt.svelte";
import { askToInstall, updatePrompt } from "../updater.js";
import { locale } from "../i18n.js";

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));

describe("update consent", () => {
  let component;
  let target;

  beforeEach(() => {
    locale.set("en");
    updatePrompt.set(null);
    target = document.createElement("div");
    document.body.append(target);
    component = mount(UpdatePrompt, { target });
  });

  afterEach(async () => {
    await unmount(component);
    target.remove();
  });

  function buttons() {
    return [...target.querySelectorAll("button")];
  }

  it("says only HideMyWindows restarts and shows the notes", () => {
    askToInstall("9.9.9", "Save your work before updating.");
    flushSync();
    const text = target.textContent;
    expect(text).toContain("HideMyWindows 9.9.9 is ready to install.");
    expect(text).toContain("Your other apps stay open and are not affected.");
    expect(text).toContain("Save your work before updating.");
    expect(buttons().map((b) => b.textContent.trim())).toEqual(["Install and restart", "Later"]);
  });

  it("focuses Later, so Enter does not install", () => {
    askToInstall("9.9.9", "");
    flushSync();
    expect(document.activeElement?.textContent.trim()).toBe("Later");
  });

  it("installs only on an explicit yes", async () => {
    const later = askToInstall("9.9.9", "");
    flushSync();
    buttons()[1].click();
    expect(await later).toBe(false);
    expect(get(updatePrompt)).toBeNull();

    const escape = askToInstall("9.9.9", "");
    flushSync();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(await escape).toBe(false);

    const install = askToInstall("9.9.9", "");
    flushSync();
    buttons()[0].click();
    expect(await install).toBe(true);
  });
});
