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

/**
 * True when MainActivity is the top resumed activity of its task.
 *
 * This reflects the app's real foreground lifecycle state and, unlike the
 * momentary focused *window*, is not disturbed when a system overlay (for
 * example a boot-time `com.android.systemui` ANR dialog) transiently steals
 * window focus on a software-GPU emulator.
 */
export function mainActivityResumed() {
  const output = adb(["shell", "dumpsys", "activity", "activities"]);
  return output
    .split("\n")
    .some(
      (line) =>
        /ResumedActivity/.test(line) && line.includes(`${APP_PACKAGE}/.MainActivity`),
    );
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

function centerOfNode(dump, token) {
  // uiautomator dumps a flat XML string; find the <node ...> element that
  // carries `token` and return the center of its `bounds` rectangle.
  const segment = dump.split("<node").find((node) => node.includes(token));
  if (!segment) {
    return null;
  }
  const match = segment.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);
  if (!match) {
    return null;
  }
  const [x1, y1, x2, y2] = match.slice(1).map(Number);
  return { x: Math.round((x1 + x2) / 2), y: Math.round((y1 + y2) / 2) };
}

/**
 * Dismiss a transient "isn't responding" (ANR) dialog if one is showing.
 *
 * Cold-booting a software-GPU emulator can make `com.android.systemui` (or the
 * launcher) ANR, and its dialog is a system-level modal that steals focus from
 * every other window. Always choose "Wait" (`android:id/aerr_wait`) so a slow
 * system process is given more time instead of being force-closed. Returns true
 * when a dialog was found and a dismissal tap was issued.
 */
export function dismissAnrDialogs() {
  let dump;
  try {
    dump = execFileSync("adb", ["exec-out", "uiautomator", "dump", "/dev/tty"], {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
  } catch {
    return false;
  }
  if (!dump.includes("isn't responding")) {
    return false;
  }
  const target = centerOfNode(dump, "aerr_wait") || centerOfNode(dump, "Wait");
  if (!target) {
    return false;
  }
  try {
    adb(["shell", "input", "tap", String(target.x), String(target.y)]);
    return true;
  } catch {
    return false;
  }
}

/**
 * Bring MainActivity to the foreground and wait until it is the focused window.
 *
 * On a cold-booted software-GPU emulator a transient system dialog (a
 * `com.android.systemui` ANR, the launcher, or the POST_NOTIFICATIONS grant
 * dialog) can own focus for several seconds after the app is already running.
 * MainActivity uses `launchMode="singleTask"`, so re-issuing `am start` simply
 * refocuses the existing instance without restarting it. This keeps the
 * assertion meaningful (MainActivity must become the focused window) while
 * staying deterministic under boot-time focus contention.
 */
export async function ensureMainActivityForeground({ timeout = 60_000 } = {}) {
  const deadline = Date.now() + timeout;
  let lastFocus = "";
  while (Date.now() < deadline) {
    lastFocus = currentFocus();
    if (lastFocus.includes(MAIN_ACTIVITY)) {
      return lastFocus;
    }
    // The app can be the foreground (resumed) activity while a transient
    // system overlay still owns window focus. That still satisfies "MainActivity
    // launched to the foreground" without waiting out an unrelated system stall.
    if (mainActivityResumed()) {
      return `resumed:${APP_PACKAGE}/.MainActivity`;
    }
    // A system ANR dialog (often `com.android.systemui` at cold boot) is a
    // modal that no `am start` can push behind, so clear it first.
    dismissAnrDialogs();
    try {
      adb(["shell", "am", "start", "-n", `${APP_PACKAGE}/.MainActivity`]);
    } catch {
      // adb can transiently fail while the system is busy; retry on next tick.
    }
    await browser.pause(1_500);
  }
  throw new Error(
    `MainActivity never became the focused window (last focus: ${lastFocus || "unknown"})`,
  );
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
