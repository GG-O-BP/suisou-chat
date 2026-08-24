import { nativeInvoke } from "../../helpers/app.mjs";

describe("live Sakana smoke", () => {
  after(async () => {
    const bootstrap = await nativeInvoke("bootstrap");
    await nativeInvoke("save_workspace", {
      workspace: {
        version: 1,
        revision: bootstrap.workspace_revision,
        conversations: [],
        settings: {
          model: "fugu",
          reasoning: "high",
          theme: "system",
          last_mode: "search",
          language: "auto",
          sync_mode: "local",
        },
      },
    });
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
