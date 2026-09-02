import { nativeInvoke } from "../../helpers/app.mjs";

describe("live Sakana smoke", () => {
  after(async () => {
    // A completed background research job can commit its durable workspace
    // between bootstrap and this test cleanup save. Reload and retry the empty
    // snapshot on that optimistic-concurrency conflict instead of failing a
    // successful live request in the after hook.
    let lastError;
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const bootstrap = await nativeInvoke("bootstrap");
      try {
        await nativeInvoke("save_workspace", {
          workspace: {
            version: 1,
            revision: bootstrap.workspace_revision,
            conversations: [],
            settings: {
              provider: "sakana",
              model: "fugu",
              reasoning: "high",
              theme: "system",
              last_mode: "search",
              language: "auto",
              sync_mode: "local",
            },
          },
        });
        return;
      } catch (error) {
        lastError = error;
        if (!String(error).includes("다른 저장 작업이 먼저 완료되었습니다")) {
          throw error;
        }
        await browser.pause(250 * (attempt + 1));
      }
    }
    throw lastError;
  });

  it("completes one real creative request when explicitly enabled", async () => {
    await $(".app-shell").waitForExist({ timeout: 20_000 });
    const connected = await $(".connection-pill").getAttribute("aria-label");
    if (!connected.includes("연결됨")) {
      throw new Error(
        "The live smoke requires an API key already stored in the native credential store",
      );
    }

    const modes = await $$(".mode-tabs button");
    await modes[3].click();
    await $("#question-input").setValue(
      "한 문장으로 조용한 수족관의 분위기를 묘사해 줘.",
    );
    await $(".send-button").click();
    await $(".message.assistant:not(.streaming)").waitForExist({
      timeout: 180_000,
    });
    await expect($(".message.assistant:not(.streaming) .message-body")).not.toHaveText("");
    await expect($("#question-input")).toBeEnabled();
  });
});
