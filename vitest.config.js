import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.js";

export default mergeConfig(viteConfig, defineConfig({
  resolve: { conditions: ["browser"] },
  test: { environment: "jsdom" },
}));
