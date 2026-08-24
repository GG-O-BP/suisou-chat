import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { shared, projectRoot } from "./wdio.shared.mjs";

const binary = path.join(projectRoot, "target", "debug", process.platform === "win32" ? "suisou-chat.exe" : "suisou-chat");
const port = Number(process.env.TAURI_WEBDRIVER_PORT || 4445);
let application;
let logFile;

export const config = {
  ...shared,
  specs: [path.join(projectRoot, "e2e/specs/native/**/*.spec.mjs")],
  hostname: "127.0.0.1",
  port,
  path: "/",
  services: [],
  capabilities: [
    {
      browserName: "tauri",
    },
  ],
  onPrepare: async () => {
    const artifactDir = path.join(projectRoot, "e2e", "artifacts");
    fs.mkdirSync(artifactDir, { recursive: true });
    logFile = fs.openSync(path.join(artifactDir, "native-backend.log"), "w");
    application = spawn(binary, [], {
      cwd: projectRoot,
      env: {
        ...process.env,
        TAURI_WEBDRIVER_PORT: String(port),
      },
      stdio: ["ignore", logFile, logFile],
    });

    const status = `http://127.0.0.1:${port}/status`;
    for (let attempt = 0; attempt < 300; attempt += 1) {
      if (application.exitCode !== null) {
        throw new Error(`E2E Tauri application exited with ${application.exitCode}`);
      }
      try {
        const response = await fetch(status);
        if (response.ok) return;
      } catch {}
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("embedded Tauri WebDriver did not become ready");
  },
  onComplete: async () => {
    application?.kill("SIGTERM");
    if (application && application.exitCode === null) {
      await Promise.race([
        new Promise((resolve) => application.once("exit", resolve)),
        new Promise((resolve) => setTimeout(resolve, 3_000)),
      ]);
    }
    if (logFile !== undefined) fs.closeSync(logFile);
  },
};
