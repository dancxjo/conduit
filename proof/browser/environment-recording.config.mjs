import base from "./playwright.config.mjs";
export default { ...base, testMatch: ["environment-recording.cases.mjs"],
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
  outputDir: "test-results/environment-recording",
};
