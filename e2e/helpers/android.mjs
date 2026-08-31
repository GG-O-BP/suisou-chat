import { execFileSync } from "node:child_process";

export const APP_PACKAGE = "com.ggobp.suisou_chat";
export const MAIN_ACTIVITY = "com.ggobp.suisou_chat.MainActivity";
export const NATIVE_CONTEXT = "NATIVE_APP";

function adb(args) {
  return execFileSync("adb", args, { encoding: "utf8" }).trim();
}

export function currentFocus() {
  const output = adb(["shell", "dumpsys", "window"]);
  const match = output.match(/mCurrentFocus=Window\{[^}]*\s([^ }]+)\}/);
  return match ? match[1] : "";
}

export function isPermissionGranted(permission) {
  const output = adb(["shell", "dumpsys", "package", APP_PACKAGE]);
  const line = output
    .split("\n")
    .find((row) => row.includes(permission) && row.includes("granted="));
  return Boolean(line && /granted=true/.test(line));
}

export function logcatContains(needle) {
  const output = execFileSync("adb", ["logcat", "-d"], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  return output.includes(needle);
}

export function restartApp() {
  adb(["shell", "am", "force-stop", APP_PACKAGE]);
  adb(["shell", "am", "start", "-n", `${APP_PACKAGE}/.MainActivity`]);
}

/**
 * Wait for the debug WebView context to appear and switch to it. Debug APKs are
 * WebView-debuggable, so Appium exposes a WEBVIEW_<package> context that lets us
 * assert against the real Sycamore DOM through CSS selectors.
 */
export async function switchToWebview({ timeout = 60_000 } = {}) {
  let webview;
  await browser.waitUntil(
    async () => {
      try {
        const contexts = await browser.getContexts();
        const names = contexts.map((entry) =>
          typeof entry === "string" ? entry : entry.id,
        );
        webview = names.find((name) => String(name).startsWith("WEBVIEW_"));
        if (!webview) return false;
        // A context can disappear briefly while Android finishes attaching the
        // System WebView. Retry the switch itself, not just context discovery.
        await browser.switchAppiumContext(webview);
        return (await browser.getAppiumContext()) === webview;
      } catch {
        return false;
      }
    },
    { timeout, interval: 1_000, timeoutMsg: "WebView context never appeared" },
  );
  return webview;
}

export async function switchToNative() {
  await browser.switchAppiumContext(NATIVE_CONTEXT);
}

/** Wait for the Sycamore shell to finish bootstrapping inside the WebView. */
export async function waitForAppShell({ timeout = 30_000 } = {}) {
  await $(".app-shell").waitForExist({ timeout });
  await $(".loading-state").waitForExist({ reverse: true, timeout });
}
