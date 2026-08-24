import fs from "node:fs";
import {
  nativeInvoke,
  removeFileIfPresent,
} from "../../helpers/app.mjs";

describe("native Tauri boundary", () => {
  let revision = 0;

  before(async () => {
    await $(".app-shell").waitForExist({ timeout: 20_000 });
    const bootstrap = await nativeInvoke("bootstrap");
    revision = bootstrap.workspace_revision;
  });

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

  it("round-trips a workspace through the real Rust storage command", async () => {
    const timestamp = Date.now();
    const workspace = {
      version: 1,
      revision,
      conversations: [
        {
          id: "conversation-native-e2e",
          title: "네이티브 저장 검증",
          pinned: false,
          created_at: timestamp,
          updated_at: timestamp,
          messages: [
            {
              id: "message-native-e2e",
              role: "user",
              content: "실제 Tauri IPC 저장 검증",
              created_at: timestamp,
              status: "complete",
              sources: [],
              usage: null,
            },
          ],
        },
      ],
      settings: {
        model: "fugu",
        reasoning: "high",
        theme: "dark",
        last_mode: "create",
        language: "auto",
        sync_mode: "local",
      },
    };

    revision = await nativeInvoke("save_workspace", { workspace });
    const bootstrap = await nativeInvoke("bootstrap");
    expect(bootstrap.workspace_revision).toBe(revision);
    expect(bootstrap.workspace.conversations[0].title).toBe("네이티브 저장 검증");
    expect(bootstrap.workspace.settings.theme).toBe("dark");
  });

  it("loads the native WebView within the startup budget", async () => {
    const metrics = await browser.execute(() => {
      const navigation = performance.getEntriesByType("navigation")[0];
      return {
        interactive: navigation?.domInteractive || 0,
        complete: navigation?.loadEventEnd || performance.now(),
      };
    });
    expect(metrics.interactive).toBeLessThan(5_000);
    expect(metrics.complete).toBeLessThan(8_000);
    await expect($(".app-shell")).toBeDisplayed();
  });

  it("rejects unsafe external URLs in the real command", async () => {
    let error;
    try {
      await nativeInvoke("open_external", {
        url: "http://unsafe.example",
      });
    } catch (value) {
      error = value;
    }
    expect(String(error)).toContain("안전한 HTTPS");
  });

  it("exports real Markdown and leaves no API key in browser storage", async () => {
    const bootstrap = await nativeInvoke("bootstrap");
    const conversation = bootstrap.workspace.conversations[0];
    const file = await nativeInvoke("export_conversation", { conversation });
    try {
      expect(fs.existsSync(file)).toBe(true);
      expect(fs.readFileSync(file, "utf8")).toContain("실제 Tauri IPC 저장 검증");
    } finally {
      removeFileIfPresent(file);
    }

    const exposed = await browser.execute(() => ({
      local: Object.entries(localStorage),
      session: Object.entries(sessionStorage),
      body: document.body.innerText,
    }));
    expect(JSON.stringify(exposed.local)).not.toContain("fish_");
    expect(JSON.stringify(exposed.session)).not.toContain("fish_");
    expect(exposed.body).not.toContain("fish_");
  });
});
