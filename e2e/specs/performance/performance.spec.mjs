import {
  openFixture,
  setScenario,
  submitQuestion,
  waitForAnswer,
} from "../../helpers/app.mjs";

const prompt =
  "일본여성향 게임의 남자주인공이 할 것 같은 달콤한 대사 리스트를 보여줘.";

describe("renderer performance budgets", () => {
  it("boots a 600-conversation workspace and filters history promptly", async () => {
    const started = Date.now();
    await openFixture("large-history");
    const bootMs = Date.now() - started;
    expect(bootMs).toBeLessThan(6_000);

    const searchStarted = Date.now();
    await $(".history-search input").setValue("희귀 심해 표본 417");
    await browser.waitUntil(async () => (await $$(".history-item")).length === 1, {
      timeout: 2_000,
    });
    expect(Date.now() - searchStarted).toBeLessThan(2_000);
  });

  it("streams a long response without blank frames or excessive layout shift", async () => {
    await openFixture("ready");
    await setScenario("performance");
    await browser.execute(() => {
      window.__suisouPerf = {
        blankFrames: 0,
        sampledFrames: 0,
        maxLongTask: 0,
        layoutShift: 0,
        stopped: false,
      };
      const perf = window.__suisouPerf;
      const sample = () => {
        const body = document.querySelector(
          ".message.assistant.streaming .streaming-plain-text",
        );
        if (body) {
          perf.sampledFrames += 1;
          if (!body.textContent?.trim()) perf.blankFrames += 1;
        }
        if (!perf.stopped) requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
      try {
        new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) {
            perf.maxLongTask = Math.max(perf.maxLongTask, entry.duration);
          }
        }).observe({ type: "longtask", buffered: true });
      } catch {}
      try {
        new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) {
            if (!entry.hadRecentInput) perf.layoutShift += entry.value;
          }
        }).observe({ type: "layout-shift", buffered: true });
      } catch {}
    });

    const started = Date.now();
    await submitQuestion(prompt, "create");
    await waitForAnswer({ timeout: 15_000 });
    const elapsed = Date.now() - started;
    const metrics = await browser.execute(() => {
      window.__suisouPerf.stopped = true;
      return window.__suisouPerf;
    });

    expect(elapsed).toBeLessThan(8_000);
    expect(metrics.sampledFrames).toBeGreaterThan(5);
    expect(metrics.blankFrames).toBe(0);
    expect(metrics.maxLongTask).toBeLessThan(250);
    expect(metrics.layoutShift).toBeLessThan(0.25);
  });
});
