import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const artifacts = path.join(root, "e2e", "artifacts");

export const projectRoot = root;

export const shared = {
  runner: "local",
  maxInstances: 1,
  logLevel: "warn",
  bail: 0,
  waitforTimeout: 12_000,
  connectionRetryTimeout: 90_000,
  connectionRetryCount: 2,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 90_000,
  },
  before: async () => {
    await browser.setTimeout({
      implicit: 0,
      pageLoad: 30_000,
      script: 30_000,
    });
  },
  afterTest: async (_test, _context, result) => {
    if (!result.passed) {
      fs.mkdirSync(artifacts, { recursive: true });
      const name = `${Date.now()}-${result.error?.name || "failure"}.png`;
      try {
        await browser.saveScreenshot(path.join(artifacts, name));
      } catch (error) {
        // Preserve the original test failure if the driver/session itself has
        // already terminated and can no longer capture a screenshot.
        console.warn(`could not save failure screenshot: ${error.message}`);
      }
    }
  },
};
