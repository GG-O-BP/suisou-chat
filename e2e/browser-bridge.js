(() => {
  "use strict";

  // A native Tauri build already exposes the complete public API. The bridge and
  // deterministic fixtures below are only installed in WDIO browser mode.
  if (window.__TAURI__?.core?.invoke && window.__TAURI__?.event?.listen) {
    return;
  }

  const internals = (window.__TAURI_INTERNALS__ ||= {});
  if (!internals.callbacks) {
    const callbacks = new Map();
    let callbackId = 0;
    internals.callbacks = callbacks;
    internals.transformCallback = (callback, once = false) => {
      const id = ++callbackId;
      callbacks.set(id, (data) => {
        if (once) callbacks.delete(id);
        return callback?.(data);
      });
      return id;
    };
    internals.runCallback = (id, data) => callbacks.get(id)?.(data);
    internals.unregisterCallback = (id) => callbacks.delete(id);
  }
  const listeners = (window.__wdio_tauri_listeners__ ||= {});
  let eventId = 0;
  window.__wdio_emit_tauri_event__ = (event, payload) => {
    for (const [id, entry] of Object.entries(listeners[event] || {})) {
      internals.runCallback(entry.handler, {
        event,
        id: Number(id),
        payload,
      });
    }
  };
  internals.invoke = async (command, args = {}) => {
    if (command === "plugin:event|listen") {
      const id = ++eventId;
      (listeners[args.event] ||= {})[id] = { handler: args.handler };
      return id;
    }
    if (command === "plugin:event|unlisten") {
      delete listeners[args.event]?.[args.eventId];
      return;
    }
    if (command === "plugin:event|emit") {
      window.__wdio_emit_tauri_event__(args.event, args.payload);
      return;
    }
    const mock = window.__wdio_mocks__?.[command];
    if (typeof mock === "function") {
      return mock(args);
    }
    if (command.startsWith("plugin:")) return;
    throw new Error(`unmocked Tauri command in browser E2E: ${command}`);
  };

  window.__TAURI__ = {
    core: {
      invoke(command, args = {}) {
        return internals.invoke(command, args);
      },
    },
    event: {
      async listen(event, handler, options = {}) {
        const handlerId = internals.transformCallback(handler);
        const eventId = await internals.invoke("plugin:event|listen", {
          event,
          target: options.target,
          handler: handlerId,
        });
        return async () => {
          await internals.invoke("plugin:event|unlisten", { event, eventId });
          internals.unregisterCallback?.(handlerId);
        };
      },
    },
  };

  const clone = (value) => structuredClone(value);
  const now = () => Date.now();
  const query = new URLSearchParams(location.search);
  const fixture = query.get("fixture") || "empty";

  const settings = {
    provider: "sakana",
    model: "fugu",
    reasoning: "high",
    theme: "system",
    last_mode: "search",
    language: "auto",
    sync_mode: "local",
  };

  const existingConversations = [
    {
      id: "conversation-existing-1",
      title: "심해 생물 발광 원리",
      pinned: true,
      created_at: now() - 120_000,
      updated_at: now() - 60_000,
      messages: [
        {
          id: "message-existing-user-1",
          role: "user",
          content: "심해 생물은 왜 빛을 내?",
          created_at: now() - 120_000,
          status: "complete",
          sources: [],
          usage: null,
        },
        {
          id: "message-existing-assistant-1",
          role: "assistant",
          content: "심해 생물은 의사소통, 위장, 먹이 유인에 생물발광을 사용합니다.",
          created_at: now() - 110_000,
          status: "complete",
          sources: [],
          usage: {
            input_tokens: 20,
            output_tokens: 35,
            total_tokens: 55,
            orchestration_tokens: 0,
          },
        },
      ],
    },
    {
      id: "conversation-existing-2",
      title: "산호초 보전 자료",
      pinned: false,
      created_at: now() - 240_000,
      updated_at: now() - 180_000,
      messages: [
        {
          id: "message-existing-user-2",
          role: "user",
          content: "산호초 보전 자료를 정리해 줘.",
          created_at: now() - 240_000,
          status: "complete",
          sources: [],
          usage: null,
        },
      ],
    },
  ];
  const largeHistory = Array.from({ length: 600 }, (_, index) => ({
    id: `conversation-load-${index}`,
    title: index === 417 ? "희귀 심해 표본 417" : `부하 시험 대화 ${index}`,
    pinned: index % 100 === 0,
    created_at: now() - (index + 1) * 2_000,
    updated_at: now() - index * 1_000,
    messages: [
      {
        id: `message-load-${index}`,
        role: "user",
        content: `성능 검증용 질문 ${index}`,
        created_at: now() - (index + 1) * 2_000,
        status: "complete",
        sources: [],
        usage: null,
      },
    ],
  }));

  const state = {
    fixture,
    scenario: "success",
    calls: {},
    jobs: new Map(),
    timers: new Map(),
    credentials: {
      sakana: fixture === "ready",
      zai: false,
    },
    workspace: {
      version: 1,
      revision: 0,
      conversations:
        fixture === "existing"
          ? existingConversations
          : fixture === "large-history"
            ? largeHistory
            : [],
      settings: { ...settings },
    },
  };

  const record = (command, args) => {
    (state.calls[command] ||= []).push(clone(args ?? {}));
  };

  const source = {
    id: "source-1",
    title: "Suisou E2E specimen",
    url: "https://example.com/e2e-specimen",
    domain: "example.com",
    snippet: "결정론적 E2E 출처 표본입니다.",
    retrieved_at: now(),
  };

  const answer = [
    "### 정통 왕자님 타입",
    ...Array.from({ length: 10 }, (_, i) => `${i + 1}. “네가 웃는 순간 ${i + 1}, 오늘은 더 특별해졌어.”`),
    "",
    "### 무뚝뚝한 엘리트 타입",
    ...Array.from({ length: 10 }, (_, i) => `${i + 11}. “말로 설명하긴 어렵지만, 네 곁이라면 괜찮아.”`),
    "",
    "### 다정한 소꿉친구 타입",
    ...Array.from({ length: 10 }, (_, i) => `${i + 21}. “오래 기다렸어. 이제는 네 손을 놓치지 않을게.”`),
    "",
    "### 고백 직전 분위기",
    ...Array.from({ length: 10 }, (_, i) => `${i + 31}. “오늘보다 내일, 내일보다 그다음 날 더 좋아할게.”`),
  ].join("\n");

  const emit = (payload) => {
    window.__wdio_emit_tauri_event__?.("research-job-event", payload);
  };

  const updateJob = (job, values) => {
    Object.assign(job, values, { updated_at: now() });
    state.jobs.set(job.request_id, clone(job));
  };

  const finish = (job, scenario) => {
    state.timers.delete(job.request_id);
    const failed = scenario === "failure";
    const status = failed ? "failed" : "complete";
    const error = failed ? "모의 네트워크 연결이 중단되었습니다." : null;
    const response = failed
      ? null
      : {
          request_id: job.request_id,
          answer,
          sources: [source],
          usage: {
            input_tokens: 120,
            output_tokens: 420,
            total_tokens: 540,
            orchestration_tokens: 32,
          },
        };
    updateJob(job, {
      status,
      stage: status === "complete" ? "done" : "failed",
      partial_answer: failed ? job.partial_answer : answer,
      result: response,
      error,
      workspace_persisted: true,
      finalizing: false,
    });
    emit({
      request_id: job.request_id,
      kind: "snapshot",
      value: "",
      sequence: 0,
      job: clone(job),
    });
  };

  const stream = (job) => {
    const scenario = state.scenario;
    const chunkSize = scenario === "performance" ? 28 : 20;
    const delay = scenario === "slow" ? 80 : scenario === "performance" ? 4 : 12;
    const chunks = [];
    for (let i = 0; i < answer.length; i += chunkSize) {
      chunks.push(answer.slice(i, i + chunkSize));
    }
    let index = 0;
    let sequence = 0;
    emit({
      request_id: job.request_id,
      kind: "stage",
      value: job.mode === "create" ? "creating" : "searching",
      sequence: 0,
      job: null,
    });
    setTimeout(() => {
      emit({
        request_id: job.request_id,
        kind: "stage",
        value: "writing",
        sequence: 0,
        job: null,
      });
    }, delay);

    const timer = setInterval(() => {
      if (!state.jobs.has(job.request_id)) {
        clearInterval(timer);
        return;
      }
      const chunk = chunks[index++];
      if (chunk === undefined || (scenario === "failure" && index > 8)) {
        clearInterval(timer);
        finish(job, scenario);
        return;
      }
      sequence += 1;
      job.partial_answer += chunk;
      updateJob(job, { partial_answer: job.partial_answer, stage: "writing" });
      emit({
        request_id: job.request_id,
        kind: "delta",
        value: chunk,
        sequence,
        job: null,
      });
    }, delay);
    state.timers.set(job.request_id, timer);
  };

  const mocks = (window.__wdio_mocks__ ||= {});

  mocks.bootstrap = () => {
    record("bootstrap", {});
    if (state.fixture === "bootstrap-error") {
      throw "모의 bootstrap 실패";
    }
    const response = {
      workspace: clone(state.workspace),
      workspace_revision: state.workspace.revision,
      credentials: [
        { provider: "sakana", key_configured: state.credentials.sakana },
        { provider: "zai", key_configured: state.credentials.zai },
      ],
      credential_notice: null,
      recovery_notice:
        state.fixture === "readonly"
          ? "손상된 기본 대화 기록을 복구해야 합니다."
          : null,
      storage_label:
        state.fixture === "readonly" ? "복구 필요 · 읽기 전용" : "이 기기에만 저장됨",
      storage_writable: state.fixture !== "readonly",
    };
    if (state.fixture === "slow-bootstrap") {
      return new Promise((resolve) => setTimeout(() => resolve(response), 700));
    }
    return response;
  };

  mocks.save_workspace = ({ workspace }) => {
    record("save_workspace", { workspace });
    if (state.fixture === "save-error") {
      throw "모의 저장 실패";
    }
    state.workspace = clone(workspace);
    state.workspace.revision += 1;
    return state.workspace.revision;
  };

  mocks.connect_api_key = ({ apiKey, providerName }) => {
    record("connect_api_key", {
      apiKey: apiKey ? "[redacted]" : "",
      providerName,
    });
    if (!apiKey || apiKey.length < 10) {
      throw "API 키 형식이 올바르지 않습니다.";
    }
    const provider = providerName === "zai" ? "zai" : "sakana";
    state.credentials[provider] = true;
    return {
      provider,
      message:
        provider === "zai"
          ? "Z.ai GLM Coding Plan API 키 형식을 확인했습니다. 첫 요청에서 구독 권한을 확인합니다."
          : "Sakana API에 안전하게 연결되었습니다.",
      models: provider === "zai" ? ["glm-5.3"] : ["fugu", "fugu-ultra"],
    };
  };

  mocks.clear_api_key = ({ providerName }) => {
    record("clear_api_key", { providerName });
    state.credentials[providerName === "zai" ? "zai" : "sakana"] = false;
  };
  mocks.forget_api_key = mocks.clear_api_key;

  mocks.start_research = (args) => {
    record("start_research", args);
    const createdAt = now();
    const job = {
      request_id: args.request.request_id,
      conversation_id: args.conversationId,
      workspace_revision: (args.workspace?.revision ?? 0) + 1,
      workspace_persisted: false,
      finalizing: false,
      assistant_message_id: args.assistantMessageId,
      question: args.question,
      mode: args.request.mode,
      provider: args.request.model === "glm-5.3" ? "zai" : "sakana",
      model: args.request.model,
      status: "running",
      stage: "connecting",
      partial_answer: "",
      result: null,
      error: null,
      created_at: createdAt,
      updated_at: createdAt,
      events: [
        { kind: "stage", value: "connecting", occurred_at: createdAt },
      ],
    };
    state.jobs.set(job.request_id, clone(job));
    setTimeout(() => stream(job), 20);
    return { job: clone(job) };
  };

  mocks.cancel_research = ({ requestId }) => {
    record("cancel_research", { requestId });
    const job = state.jobs.get(requestId);
    if (!job || job.status !== "running") return false;
    const timer = state.timers.get(requestId);
    if (timer) clearInterval(timer);
    updateJob(job, {
      status: "cancelled",
      stage: "cancelled",
      error: "사용자가 답변 생성을 중단했습니다.",
      result: null,
      workspace_persisted: true,
      finalizing: false,
    });
    emit({
      request_id: requestId,
      kind: "snapshot",
      value: "",
      sequence: 0,
      job: clone(job),
    });
    return true;
  };

  mocks.list_research_jobs = () => Array.from(state.jobs.values()).map(clone);
  mocks.get_research_job = ({ requestId }) => clone(state.jobs.get(requestId) ?? null);
  mocks.discard_research_job = ({ requestId }) => {
    const existed = state.jobs.delete(requestId);
    const timer = state.timers.get(requestId);
    if (timer) clearInterval(timer);
    state.timers.delete(requestId);
    return existed;
  };
  mocks.open_external = ({ url }) => {
    record("open_external", { url });
    if (!String(url).startsWith("https://")) {
      throw "안전한 HTTPS 링크만 열 수 있습니다.";
    }
  };
  mocks.export_conversation = ({ conversation }) => {
    record("export_conversation", { conversationId: conversation.id });
    return `/tmp/suisou-e2e-${conversation.id}.md`;
  };

  window.__suisouE2e = {
    state,
    answer,
    setScenario(scenario) {
      state.scenario = scenario;
    },
    calls(command) {
      return clone(state.calls[command] || []);
    },
    clearCalls() {
      state.calls = {};
    },
    seedResearchJob(job) {
      state.jobs.set(job.request_id, clone(job));
    },
    emit(event, payload) {
      window.__wdio_emit_tauri_event__(event, payload);
    },
  };
})();
