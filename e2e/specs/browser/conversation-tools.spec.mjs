import {
  bridgeCalls,
  openFixture,
} from "../../helpers/app.mjs";

describe("conversation history and tools", () => {
  beforeEach(async () => {
    await openFixture("existing");
  });

  it("searches history and selects an existing conversation", async () => {
    const search = await $(".history-search input");
    await search.setValue("산호초");

    const items = await $$(".history-item");
    expect(items).toHaveLength(1);
    await $(".history-select").click();
    await expect($(".message.user")).toHaveText(
      expect.stringContaining("산호초"),
    );
  });

  it("pins, exports, and deletes the active conversation", async () => {
    const history = await $$(".history-select");
    await history[0].click();
    await $(".settings-button").click();

    let tools = await $$(".conversation-tools button");
    await tools[0].click();
    await browser.waitUntil(
      async () => (await bridgeCalls("save_workspace")).length >= 1,
    );

    tools = await $$(".conversation-tools button");
    await tools[1].click();
    await expect($(".toast")).toHaveText(
      expect.stringContaining("Markdown 파일로 내보냈습니다"),
    );
    expect(await bridgeCalls("export_conversation")).toHaveLength(1);
    await $('button[aria-label="알림 닫기"]').click();
    await $(".toast").waitForExist({ reverse: true });

    tools = await $$(".conversation-tools button");
    await tools[2].click();
    await expect($(".welcome")).toBeDisplayed();
    await expect($(".history-list")).not.toHaveText(
      expect.stringContaining("심해 생물 발광 원리"),
    );
  });
});
