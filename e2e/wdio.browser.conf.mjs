import { spawn } from "node:child_process";
import path from "node:path";
import { shared, projectRoot } from "./wdio.shared.mjs";

const url = "http://127.0.0.1:1421/index.html";
let server;

export const config = {
  ...shared,
  specs: [path.join(projectRoot, "e2e/specs/browser/**/*.spec.mjs")],
  services: [],
  capabilities: [
    {
      browserName: "chrome",
      "goog:chromeOptions": {
        args: [
          "--headless=new",
          "--disable-gpu",
          "--no-sandbox",
          "--window-size=1440,900",
        ],
      },
    },
  ],
  onPrepare: async () => {
    server = spawn(
      process.execPath,
      ["e2e/server.mjs", "dist-e2e", "1421"],
      { cwd: projectRoot, stdio: "ignore" },
    );
    for (let attempt = 0; attempt < 100; attempt += 1) {
      try {
        const response = await fetch(url);
        if (response.ok) return;
      } catch {}
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("E2E static server did not become ready");
  },
  onComplete: () => {
    server?.kill("SIGTERM");
  },
};
