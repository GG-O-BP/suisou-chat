import {
  MAIN_ACTIVITY,
  currentFocus,
  ensureMainActivityForeground,
  isPermissionGranted,
  logcatContains,
  restartApp,
  switchToWebview,
  switchToNative,
  waitForAppShell,
} from "../../helpers/android.mjs";

// Deterministic Android APK smoke test. It drives the real x86_64 debug APK
// through Appium/UiAutomator2 and crosses the APK -> System WebView -> Rust IPC
// boundary. It never provisions an API key, so it exercises the app's
// "API key missing" state only.
describe("Android APK smoke", () => {
  it("launches MainActivity as the foreground activity", async () => {
    await switchToNative();
    const focus = await ensureMainActivityForeground();
    expect(focus).toContain("MainActivity");
  });

  it("handles the research notification permission request", async () => {
    // MainActivity requests POST_NOTIFICATIONS on first launch; the session
    // grants it up front (autoGrantPermissions), so it must be granted here.
    expect(isPermissionGranted("android.permission.POST_NOTIFICATIONS")).toBe(true);
  });

  it("bootstraps the Sycamore WebView across the Rust IPC boundary", async () => {
    await switchToWebview();
    await waitForAppShell();
    expect(await $(".app-shell").isExisting()).toBe(true);
    expect(await $("#question-input").isExisting()).toBe(true);
  });

  it("renders Korean interface text without replacement glyphs", async () => {
    await switchToWebview();
    await waitForAppShell();
    const text = await browser.execute(() => document.body.innerText);
    expect(text).toContain("새 대화");
    expect(text).toContain("Sakana API 연결 필요");
    expect(text).not.toContain("\uFFFD");
    expect(await $("#question-input").getAttribute("placeholder")).toBe(
      "무엇을 알아볼까요?",
    );
  });

  it("shows the deterministic 'API key missing' state without a key", async () => {
    await switchToWebview();
    await waitForAppShell();
    const pill = await $(".connection-pill.disconnected");
    await pill.waitForExist({ timeout: 15_000 });
    expect(await pill.getAttribute("aria-label")).toContain("API 키");
  });

  it("accepts composer input and mode selection in the WebView", async () => {
    await switchToWebview();
    await waitForAppShell();
    const input = await $("#question-input");
    // ASCII keeps the assertion independent of the emulator's installed IME;
    // long Korean rendering remains covered by the deterministic browser suite.
    await input.setValue("deep sea observation smoke test");
    // WebdriverIO's element getValue endpoint currently returns an empty string
    // through this Appium/Chromedriver combination even though the text is
    // visibly entered. Read the real DOM property to verify the interaction.
    const inputValue = await browser.execute(
      () => document.querySelector("#question-input")?.value || "",
    );
    expect(inputValue).toContain("deep sea observation");

    const searchTab = await $('.mode-tabs button[role="radio"]:nth-child(2)');
    await searchTab.click();
    await browser.waitUntil(
      async () => (await searchTab.getAttribute("aria-checked")) === "true",
      { timeout: 5_000, timeoutMsg: "search mode was not selected" },
    );
  });

  it("opens the settings control room and exposes a masked key field", async () => {
    await switchToWebview();
    await waitForAppShell();
    // The Android soft keyboard can cover the welcome button after the composer
    // input test. Dispatch the same DOM activation path without moving the
    // viewport, then continue asserting through the real WebView/IPC boundary.
    await browser.execute(() => {
      const button = document.querySelector(".welcome-status button");
      if (!button) throw new Error("welcome settings button is missing");
      button.click();
    });
    await $(".settings-panel.visible").waitForExist({ timeout: 10_000 });
    const keyInput = await $("#sakana-api-key");
    await keyInput.waitForExist({ timeout: 10_000 });
    // The key field must be masked and must never be a plain text input.
    expect(await keyInput.getAttribute("type")).toBe("password");
    const glmKeyInput = await $("#zai-api-key");
    expect(await glmKeyInput.getAttribute("type")).toBe("password");
  });

  it("survives background and foreground lifecycle transitions", async () => {
    await switchToNative();
    await browser.background(2);
    await browser.waitUntil(() => currentFocus().includes(MAIN_ACTIVITY), {
      timeout: 15_000,
      interval: 500,
      timeoutMsg: "MainActivity did not return from background",
    });
    await switchToWebview();
    await waitForAppShell();
  });

  it("survives an Android configuration change", async () => {
    await switchToNative();
    await browser.setOrientation("LANDSCAPE");
    await browser.pause(1_000);
    await switchToWebview();
    await waitForAppShell();
    expect(await $(".app-shell").isExisting()).toBe(true);
    await switchToNative();
    await browser.setOrientation("PORTRAIT");
  });

  it("persists workspace settings across a process restart", async () => {
    await switchToWebview();
    await waitForAppShell();
    if (!(await $(".settings-panel.visible").isExisting())) {
      await $(".welcome-status button").click();
      await $(".settings-panel.visible").waitForExist({ timeout: 5_000 });
    }
    const darkTheme = await $(
      ".settings-panel .segmented-control button:nth-child(3)",
    );
    await darkTheme.click();
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          () => document.documentElement.getAttribute("data-theme"),
        )) === "dark",
      { timeout: 5_000, timeoutMsg: "dark theme was not applied" },
    );
    await browser.pause(750);

    restartApp();
    await switchToNative();
    await browser.waitUntil(() => currentFocus().includes(MAIN_ACTIVITY), {
      timeout: 30_000,
      interval: 1_000,
      timeoutMsg: "MainActivity did not return after restart",
    });
    await switchToWebview();
    await waitForAppShell();
    expect(await $(".app-shell").isExisting()).toBe(true);
    expect(
      await browser.execute(
        () => document.documentElement.getAttribute("data-theme"),
      ),
    ).toBe("dark");
  });

  it("does not leak an API key marker to logcat", async () => {
    // No key is ever entered, but assert the obvious leak channel is clean so a
    // regression that logs secrets is caught by the Android layer too.
    await switchToWebview();
    const webStorage = await browser.execute(() => ({
      local: { ...localStorage },
      session: { ...sessionStorage },
    }));
    expect(JSON.stringify(webStorage)).not.toContain("SAKANA_API_KEY");
    expect(JSON.stringify(webStorage)).not.toContain("Bearer ");
    expect(logcatContains("SAKANA_API_KEY=")).toBe(false);
    expect(logcatContains("Bearer sk-")).toBe(false);
  });
});
