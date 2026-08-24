import { accessSync, constants } from "node:fs";
import { execFileSync } from "node:child_process";

const required = [
  ["cargo", ["--version"]],
  ["trunk", ["--version"]],
  ["node", ["--version"]],
  ["npm", ["--version"]],
];

let failed = false;
for (const [command, args] of required) {
  try {
    const version = execFileSync(command, args, { encoding: "utf8" }).trim();
    console.log(`ok  ${command}: ${version}`);
  } catch {
    console.error(`missing  ${command}`);
    failed = true;
  }
}

const chromeCandidates = [
  process.env.CHROME_BIN,
  "/usr/bin/google-chrome-stable",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean);
const chrome = chromeCandidates.find((candidate) => {
  try {
    accessSync(candidate, constants.X_OK);
    return true;
  } catch {
    return false;
  }
});
if (chrome) console.log(`ok  chrome: ${chrome}`);
else {
  console.error("missing  Chrome/Chromium for browser E2E");
  failed = true;
}

if (process.platform === "linux") {
  if (process.env.DISPLAY) console.log(`ok  DISPLAY: ${process.env.DISPLAY}`);
  else console.warn("note  native E2E requires DISPLAY or xvfb-run on Linux");
}

if (failed) process.exit(1);
