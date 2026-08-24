import fs from "node:fs";

export const browserBase = "http://127.0.0.1:1421/index.html";

export async function openFixture(name = "empty") {
  await browser.url(`${browserBase}?fixture=${encodeURIComponent(name)}`);
  await $(".app-shell").waitForExist({ timeout: 15_000 });
  await $(".loading-state").waitForExist({ reverse: true, timeout: 15_000 });
}

export async function selectMode(value) {
  await $(`.mode-tabs button[role="radio"]:nth-child(${
    { quick: 1, search: 2, deep: 3, create: 4 }[value]
  })`).click();
  await browser.waitUntil(
    async () =>
      (await $(`.composer-wrap.mode-${value}`).isExisting()) &&
      (await $(`.mode-tabs button:nth-child(${
        { quick: 1, search: 2, deep: 3, create: 4 }[value]
      })`).getAttribute("aria-checked")) === "true",
    { timeout: 5_000, timeoutMsg: `mode ${value} was not selected` },
  );
}

export async function submitQuestion(question, mode = "create") {
  await selectMode(mode);
  const input = await $("#question-input");
  await input.setValue(question);
  await $(".send-button:not(.stop)").click();
}

export async function waitForAnswer({ timeout = 20_000 } = {}) {
  const answer = await $(".message.assistant:not(.streaming)");
  await answer.waitForExist({ timeout });
  await $(".message.assistant.streaming").waitForExist({
    reverse: true,
    timeout,
  });
  return answer;
}

export async function waitForStreaming({ timeout = 10_000 } = {}) {
  const answer = await $(".message.assistant.streaming .streaming-plain-text");
  await answer.waitForExist({ timeout });
  await browser.waitUntil(async () => (await answer.getText()).length > 20, {
    timeout,
    timeoutMsg: "streaming output did not begin",
  });
  return answer;
}

export async function setScenario(scenario) {
  await browser.execute((value) => window.__suisouE2e.setScenario(value), scenario);
}

export async function bridgeCalls(command) {
  return browser.execute((value) => window.__suisouE2e.calls(value), command);
}

export async function nativeInvoke(command, args = {}) {
  const result = await browser.executeAsync((cmd, payload, done) => {
    window.__TAURI__.core
      .invoke(cmd, payload)
      .then((value) => done({ ok: true, value }))
      .catch((error) =>
        done({
          ok: false,
          error:
            typeof error === "string"
              ? error
              : error?.message || JSON.stringify(error),
        }),
      );
  }, command, args);
  if (!result.ok) {
    throw new Error(result.error);
  }
  return result.value;
}

export function removeFileIfPresent(file) {
  if (file && fs.existsSync(file)) {
    fs.unlinkSync(file);
  }
}
