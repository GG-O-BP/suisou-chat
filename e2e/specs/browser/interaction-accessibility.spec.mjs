import {
  bridgeCalls,
  openFixture,
  submitQuestion,
  waitForAnswer,
} from "../../helpers/app.mjs";

describe("controls, shortcuts, panels, and responsive behavior", () => {
  it("applies a creative suggestion and persists model/reasoning/mode controls", async () => {
    await openFixture("ready");
    const suggestions = await $$(".suggestion-grid button");
    await suggestions[3].click();

    await expect($("#question-input")).toHaveValue(
      expect.stringContaining("늦은 밤 수족관"),
    );
    await expect($(".composer-wrap")).toHaveElementClass("mode-create");

    const selects = await $$(".model-controls select");
    await selects[0].selectByVisibleText("Fugu Ultra");
    await selects[1].selectByVisibleText("X-High");
    await browser.waitUntil(
      async () => (await bridgeCalls("save_workspace")).length >= 3,
    );
  });

  it("copies completed content and opens a validated source URL", async () => {
    await openFixture("ready");
    await browser.execute(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async (text) => {
            window.__suisouCopied = text;
          },
        },
      });
    });

    await submitQuestion("복사와 출처 동작을 확인해 줘.", "search");
    await waitForAnswer();
    await $(".answer-actions button").click();
    const copied = await browser.execute(() => window.__suisouCopied);
    expect(copied).toContain("정통 왕자님 타입");

    await $(".answer-actions button:nth-child(2)").click();
    await $(".source-card button").click();
    const calls = await bridgeCalls("open_external");
    expect(calls.at(-1).url).toBe("https://example.com/e2e-specimen");
  });

  it("handles Escape, the new-conversation shortcut, and mobile navigation", async () => {
    await openFixture("existing");
    const history = await $$(".history-select");
    await history[0].click();
    await $(".settings-button").click();
    await browser.keys(["Escape"]);
    await expect($(".settings-panel")).not.toHaveElementClass("visible");

    const shortcut = await browser.execute(() => {
      const event = new KeyboardEvent("keydown", {
        key: "n",
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      });
      document.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(shortcut).toBe(true);
    await expect($(".welcome")).toBeDisplayed();

    await browser.setWindowSize(390, 844);
    const menu = await $('button[aria-label="대화 기록 열기"]');
    await expect(menu).toBeDisplayed();
    await menu.click();
    await expect($(".sidebar")).toHaveElementClass("visible");
    await $('button[aria-label="메뉴 닫기"]').click();
    await browser.setWindowSize(1440, 900);
  });

  it("reports bootstrap and persistence failures without silently continuing", async () => {
    await openFixture("bootstrap-error");
    await $(".toast").waitForExist({ timeout: 5_000 });
    await expect($(".toast")).toHaveText(expect.stringContaining("bootstrap 실패"));

    await openFixture("save-error");
    const modes = await $$(".mode-tabs button");
    await modes[3].click();
    await expect($(".composer-wrap")).toHaveElementClass("read-only");
    await expect($(".storage-status")).toHaveText(
      expect.stringContaining("저장 오류"),
    );
    await expect($(".composer-hint")).toHaveText(
      expect.stringContaining("저장하지 못했습니다"),
    );
  });
});
