import { openFixture } from "../../helpers/app.mjs";

describe("background and resume reconciliation", () => {
  it("restores a running background job after a resume event", async () => {
    await openFixture("existing");
    const history = await $$(".history-select");
    await history[0].click();
    const timestamp = Date.now();
    await browser.execute((createdAt) => {
      window.__suisouE2e.seedResearchJob({
        request_id: "request-resume-e2e",
        conversation_id: "conversation-existing-1",
        workspace_revision: 0,
        workspace_persisted: false,
        finalizing: false,
        assistant_message_id: "message-resume-e2e",
        question: "재개 검증",
        mode: "deep",
        status: "running",
        stage: "writing",
        partial_answer: "백그라운드에서 보존된 부분 답변",
        result: null,
        error: null,
        created_at: createdAt,
        updated_at: createdAt,
        events: [
          { kind: "stage", value: "writing", occurred_at: createdAt },
        ],
      });
      window.__suisouE2e.emit("tauri://resumed", null);
    }, timestamp);

    await $(".message.assistant.streaming").waitForExist({ timeout: 10_000 });
    await expect($(".streaming-plain-text")).toHaveText(
      expect.stringContaining("백그라운드에서 보존된 부분 답변"),
    );
    await expect($(".send-button.stop")).toBeDisplayed();
  });
});
