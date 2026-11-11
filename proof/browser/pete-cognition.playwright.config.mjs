import { defineConfig } from "@playwright/test";
import patchbay from "./patchbay-html.playwright.config.mjs";

// Reuse the reviewed Chromium profile; this proof requires an actual run.
export default defineConfig({ ...patchbay, testMatch: ["pete-cognition.spec.mjs"], workers: 1, retries: 0 });
