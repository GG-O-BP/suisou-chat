import { openFixture } from "../../helpers/app.mjs";

describe("bootstrap, settings, and environment states", () => {
  it("shows the API-key onboarding state and opens settings", async () => {
    await openFixture("empty");

    await expect($(".welcome-status")).toHaveText(
      expect.stringContaining("Sakana API 연결 필요"),
    );
    await expect($("#question-input")).toBeEnabled();

    await $(".welcome-status button").click();
    await expect($(".settings-panel")).toHaveElementClass("visible");
    await expect($("#api-key")).toBeDisplayed();
  });

  it("connects and disconnects an API key through the renderer flow", async () => {
    await openFixture("empty");
    await $(".settings-button").click();

    await $("#api-key").setValue("e2e-test-credential-1234567890");
    await $(".key-form button[type=submit]").click();

    await expect($(".key-connected")).toHaveText(
      expect.stringContaining("Sakana Fugu 준비 완료"),
    );
    await expect($(".toast")).toHaveText(
      expect.stringContaining("안전하게 저장"),
    );

    await $(".key-connected button").click();
    await expect($("#api-key")).toBeDisplayed();
  });

  it("switches light and dark themes and persists the setting", async () => {
    await openFixture("empty");
    await $(".settings-button").click();
    const themeButtons = await $$(".segmented-control button");

    await themeButtons[2].click();
    await browser.waitUntil(
      async () =>
        (await browser.execute(() =>
          document.documentElement.getAttribute("data-theme"),
        )) === "dark",
    );

    await (await $$(".segmented-control button"))[1].click();
    await browser.waitUntil(
      async () =>
        (await browser.execute(() =>
          document.documentElement.getAttribute("data-theme"),
        )) === "light",
    );
  });

  it("renders loading and read-only recovery states", async () => {
    await browser.url(
      "http://127.0.0.1:1421/index.html?fixture=slow-bootstrap",
    );
    await expect($(".loading-state")).toBeDisplayed();
    await $(".loading-state").waitForExist({ reverse: true, timeout: 5_000 });

    await openFixture("readonly");
    await expect($(".composer-wrap")).toHaveElementClass("read-only");
    await expect($("#question-input")).toBeDisabled();
    await expect($(".storage-status")).toHaveText(
      expect.stringContaining("복구 필요"),
    );
  });
});
