import {
  openFixture,
  setScenario,
  submitQuestion,
  waitForAnswer,
  waitForStreaming,
} from "../../helpers/app.mjs";

const creativePrompt =
  "일본여성향 게임의 남자주인공이 할 것 같은 달콤한 대사 리스트를 보여줘.";

describe("research lifecycle", () => {
  beforeEach(async () => {
    await openFixture("ready");
  });

  it("streams and completes creative output with sources and usage", async () => {
    await submitQuestion(creativePrompt, "create");
    const streaming = await waitForStreaming();
    await expect(streaming).toHaveText(expect.stringContaining("왕자님"));

    const answer = await waitForAnswer();
    await expect(answer).toHaveText(expect.stringContaining("고백 직전 분위기"));
    await expect(answer).toHaveText(expect.stringContaining("복사"));
    await expect($(".usage")).toHaveText("총 540 tokens");
    await expect($("#question-input")).toBeEnabled();

    await $(".answer-actions button:nth-child(2)").click();
    await expect($(".source-card")).toHaveText(
      expect.stringContaining("Suisou E2E specimen"),
    );
  });

  it("cancels a slow stream and keeps the partial output", async () => {
    await setScenario("slow");
    await submitQuestion(creativePrompt, "create");
    const streaming = await waitForStreaming();
    const partial = await streaming.getText();

    await $(".send-button.stop").click();
    await $(".message.status-cancelled").waitForExist({ timeout: 10_000 });
    await expect($(".message.status-cancelled")).toHaveText(
      expect.stringContaining("네가 웃는 순간"),
    );
    await expect($(".retry-banner.cancelled")).toBeDisplayed();
    await expect($("#question-input")).toBeEnabled();
  });

  it("surfaces a partial failed answer and retry affordance", async () => {
    await setScenario("failure");
    await submitQuestion(creativePrompt, "create");
    await $(".message.status-failed").waitForExist({ timeout: 15_000 });

    await expect($(".message.status-failed")).toHaveText(
      expect.stringContaining("일부만 작성됨"),
    );
    await expect($(".retry-banner.failed")).toBeDisplayed();
  });

  it("opens settings instead of sending without a key", async () => {
    await openFixture("empty");
    await selectCreateAndType();
    await $(".send-button").click();

    await expect($(".settings-panel")).toHaveElementClass("visible");
    await expect($(".toast")).toHaveText(
      expect.stringContaining("Sakana API 키"),
    );
  });
});

async function selectCreateAndType() {
  const fourthMode = await $$(".mode-tabs button")[3];
  await fourthMode.click();
  await $("#question-input").setValue(creativePrompt);
}
