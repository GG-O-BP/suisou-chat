import { accessSync, constants } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";

// Preflight checks for the Android APK E2E layer. This never provisions an API
// key and only inspects tool availability.

let failed = false;

function ok(label, detail) {
  console.log(`ok  ${label}${detail ? `: ${detail}` : ""}`);
}
function missing(label) {
  console.error(`missing  ${label}`);
  failed = true;
}
function note(label) {
  console.warn(`note  ${label}`);
}

function tryVersion(command, args) {
  try {
    return execFileSync(command, args, { encoding: "utf8" }).trim().split("\n")[0];
  } catch {
    return null;
  }
}

const androidHome = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT;
const projectAppiumHome =
  process.env.APPIUM_HOME ||
  new URL("./.appium", import.meta.url).pathname.replace(/\/$/, "");
const appiumEnv = { ...process.env, APPIUM_HOME: projectAppiumHome };
if (androidHome) ok("ANDROID_HOME", androidHome);
else missing("ANDROID_HOME/ANDROID_SDK_ROOT (source scripts/android-env.sh)");

for (const [label, command, args] of [
  ["adb", "adb", ["--version"]],
  ["cargo tauri", "cargo", ["tauri", "--version"]],
  ["node", "node", ["--version"]],
  ["npm", "npm", ["--version"]],
]) {
  const version = tryVersion(command, args);
  if (version) ok(label, version);
  else missing(label);
}

const emulator = androidHome ? `${androidHome}/emulator/emulator` : null;
if (emulator) {
  try {
    accessSync(emulator, constants.X_OK);
    ok("emulator", emulator);
  } catch {
    missing("Android emulator package (sdkmanager --install 'emulator')");
  }
}

const appiumBin = new URL("../node_modules/.bin/appium", import.meta.url).pathname;
try {
  accessSync(appiumBin, constants.X_OK);
  let appiumVersion = null;
  try {
    appiumVersion = execFileSync(appiumBin, ["--version"], {
      encoding: "utf8",
      env: appiumEnv,
    }).trim();
  } catch {}
  ok("appium", appiumVersion || appiumBin);
  const driverResult = spawnSync(appiumBin, ["driver", "list", "--installed"], {
    encoding: "utf8",
    env: appiumEnv,
  });
  const drivers = `${driverResult.stdout || ""}\n${driverResult.stderr || ""}`;
  if (drivers && /uiautomator2/.test(drivers)) ok("appium driver", "uiautomator2");
  else note("uiautomator2 driver not installed yet (installed on first e2e:android run)");
} catch {
  missing("appium (run: npm install)");
}

try {
  accessSync("/dev/kvm", constants.R_OK | constants.W_OK);
  ok("/dev/kvm", "hardware acceleration available");
} catch {
  note("/dev/kvm not writable; x86_64 emulator will be slow or unavailable");
}

if (failed) process.exit(1);
