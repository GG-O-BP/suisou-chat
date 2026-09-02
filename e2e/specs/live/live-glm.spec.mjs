import { nativeInvoke } from "../../helpers/app.mjs";

describe("live Z.ai GLM smoke", () => {
  after(async () => {
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

  it("completes one real standard Model API request when explicitly enabled", async () => {
    await $(".app-shell").waitForExist({ timeout: 20_000 });
    const bootstrap = await nativeInvoke("bootstrap");
    const glm = bootstrap.credentials.find(
      (credential) => credential.provider === "zai",
    );
    if (!glm?.key_configured) {
      throw new Error(
        "The GLM live smoke requires a standard Z.ai key already stored in the native credential store",
      );
    }

    const modes = await $$(".mode-tabs button");
    await modes[0].click();
    await browser.execute(() => {
      const select = document.querySelector(".model-controls select");
      if (!select) throw new Error("model selector is missing");
      select.value = "glm-5.3";
      select.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await browser.waitUntil(
      async () =>
        (await $(".connection-pill").getAttribute("aria-label")).includes(
          "Z.ai GLM",
        ),
      {
        timeout: 10_000,
        timeoutMsg: "GLM was not selected before the live request",
      },
    );
    await $("#question-input").setValue(
      "한 문장으로 조용한 수족관의 분위기를 묘사해 줘.",
    );
    await $(".send-button").click();
    await $(".message.assistant:not(.streaming), .retry-banner.failed").waitForExist({
      timeout: 180_000,
    });
    if (await $(".retry-banner.failed").isExisting()) {
      const failureState = await browser.execute(() => ({
        notice: document.querySelector(".toast")?.textContent?.trim() || "",
        retryBanner: document
          .querySelector(".retry-banner.failed")
          ?.textContent?.trim() || "",
      }));
      throw new Error(
        `Live GLM request failed: ${JSON.stringify(failureState, null, 2)}`,
      );
    }
    await expect($(".message.assistant:not(.streaming) .message-body")).not.toHaveText("");
    await expect($("#question-input")).toBeEnabled();
  });
});
