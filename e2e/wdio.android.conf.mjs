import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { shared, projectRoot } from "./wdio.shared.mjs";

const appPackage = "com.ggobp.suisou_chat";
const appActivity = ".MainActivity";

const defaultApk = path.join(
  projectRoot,
  "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk",
);
const apkPath = process.env.SUISOU_ANDROID_APK || defaultApk;

if (!fs.existsSync(apkPath)) {
  throw new Error(
    `Android debug APK not found at ${apkPath}. Build it with ` +
      "`scripts/android-build-e2e-apk.sh` or run `npm run e2e:android`.",
  );
}

const appiumPort = Number(process.env.APPIUM_PORT || 4723);
const artifactDir = path.join(projectRoot, "e2e", "artifacts");
const appiumHome = process.env.APPIUM_HOME || path.join(projectRoot, "e2e", ".appium");

export const config = {
  ...shared,
  mochaOpts: {
    ...shared.mochaOpts,
    // Context reattachment after Android process/configuration changes can be
    // slow on software-rendered CI emulators.
    timeout: 180_000,
  },
  specs: [path.join(projectRoot, "e2e/specs/android/**/*.spec.mjs")],
  hostname: "127.0.0.1",
  port: appiumPort,
  path: "/",
  services: [
    [
      "appium",
      {
        command: path.join(projectRoot, "node_modules", ".bin", "appium"),
        args: {
          address: "127.0.0.1",
          port: appiumPort,
          basePath: "/",
          // Permit only the one Appium extension needed to match the
          // emulator's System WebView. Do not enable broad relaxed security.
          allowInsecure: "chromedriver_autodownload",
          log: path.join(artifactDir, "appium.log"),
          logTimestamp: true,
        },
      },
    ],
  ],
  capabilities: [
    {
      platformName: "Android",
      "appium:automationName": "UiAutomator2",
      "appium:deviceName": "Android Emulator",
      "appium:app": apkPath,
      "appium:appPackage": appPackage,
      "appium:appActivity": appActivity,
      // Grant POST_NOTIFICATIONS up front so the run is deterministic; the
      // permission-request path itself is asserted separately in the spec.
      "appium:autoGrantPermissions": true,
      "appium:newCommandTimeout": 240,
      "appium:fullReset": false,
      "appium:noReset": false,
      "appium:androidInstallTimeout": 180_000,
      "appium:uiautomator2ServerLaunchTimeout": 120_000,
      "appium:adbExecTimeout": 120_000,
      "appium:appWaitActivity": "*",
      "appium:ensureWebviewsHavePages": true,
      "appium:chromedriverAutodownload": true,
    },
  ],
  // UiAutomator2 does not implement the W3C script timeout endpoint. The
  // browser/native suites keep the shared timeout hook; Android relies on the
  // capability and explicit waitUntil timeouts instead.
  before: async () => {},
  onPrepare: () => {
    fs.mkdirSync(artifactDir, { recursive: true });
    fs.mkdirSync(appiumHome, { recursive: true });
  },
  onComplete: () => {
    // Best-effort logcat capture for post-mortem debugging.
    try {
      const logcat = execFileSync("adb", ["logcat", "-d"], {
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024,
      });
      fs.writeFileSync(path.join(artifactDir, "android-logcat.txt"), logcat);
    } catch {
      // adb may be unavailable in onComplete; the wrapper script also captures it.
    }
  },
};
