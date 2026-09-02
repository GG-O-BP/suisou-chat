use super::stream::{parse_sse_frame, take_sse_frame};
use super::transport::clean_remote_error;
use super::*;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::time::{sleep_until, Instant};

const OUTPUT_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(super) async fn research<F>(
    client: &Client,
    request: &ResearchRequest,
    key: Zeroizing<String>,
    cancellation: CancellationToken,
    emit: &mut F,
) -> Result<ResearchResponse, String>
where
    F: FnMut(&str, &str) + Send,
{
    emit("stage", "connecting");
    let body = request_body(request);
    let send = client
        .post(format!("{ZAI_API_ROOT}/chat/completions"))
        .bearer_auth(key.as_str())
        .json(&body)
        .send();
    let response = tokio::select! {
        _ = cancellation.cancelled() => return cancelled(),
        response = send => response.map_err(|error| network_error(error, Provider::Zai))?,
    };
    if !response.status().is_success() {
        return Err(http_error(response, Provider::Zai).await);
    }

    if matches!(request.mode.as_str(), "search" | "deep") {
        emit("stage", "searching");
    } else if request.mode == "create" {
        emit("stage", "creating");
    } else {
        emit("stage", "reasoning");
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    if content_type.contains("text/event-stream") {
        consume_stream(request, response, cancellation, emit).await
    } else {
        if response.content_length().unwrap_or(0) > MAX_RESPONSE_BYTES as u64 {
            return Err("Z.ai 응답이 안전한 크기 제한을 초과했습니다.".into());
        }
        let bytes = tokio::select! {
            _ = cancellation.cancelled() => return cancelled(),
            bytes = response.bytes() => bytes.map_err(|error| network_error(error, Provider::Zai))?,
        };
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("Z.ai 응답이 안전한 크기 제한을 초과했습니다.".into());
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "Z.ai 응답을 해석하지 못했습니다.".to_string())?;
        complete_response(request, &value)
    }
}

pub(super) fn request_body(request: &ResearchRequest) -> Value {
    let mut messages = vec![json!({
        "role": "system",
        "content": instructions(&request.mode, Provider::Zai),
    })];
    messages.extend(
        request
            .messages
            .iter()
            .map(|message| json!({"role": message.role, "content": message.content})),
    );

    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
        "max_tokens": output_limit(&request.mode),
        "thinking": {"type": "enabled"},
        "reasoning_effort": reasoning_effort(&request.reasoning),
    });
    if matches!(request.mode.as_str(), "search" | "deep") {
        body["tools"] = json!([{
            "type": "web_search",
            "web_search": {
                "enable": true,
                "search_result": true,
                "search_engine": "search-prime",
                "count": if request.mode == "deep" { 10 } else { 5 }
            }
        }]);
    }
    body
}

pub(super) fn reasoning_effort(value: &str) -> &'static str {
    match value {
        "max" => "max",
        // GLM-5.3 supports low/high/max. Preserve the stronger setting when
        // legacy Fugu xhigh settings are carried into a GLM request.
        _ => "high",
    }
}

pub(in crate::fugu) enum FrameOutcome {
    Continue,
    Done,
}

pub(in crate::fugu) struct GlmStreamState {
    pub(in crate::fugu) answer: String,
    pub(in crate::fugu) metadata: Value,
    pub(in crate::fugu) saw_finish: bool,
    pub(in crate::fugu) writing_started: bool,
    pub(in crate::fugu) last_output_at: Option<Instant>,
}

async fn consume_stream<F>(
    request: &ResearchRequest,
    response: reqwest::Response,
    cancellation: CancellationToken,
    emit: &mut F,
) -> Result<ResearchResponse, String>
where
    F: FnMut(&str, &str) + Send,
{
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut state = GlmStreamState {
        answer: String::new(),
        metadata: Value::Object(Default::default()),
        saw_finish: false,
        writing_started: false,
        last_output_at: None,
    };

    loop {
        let idle_deadline = state
            .last_output_at
            .map(|instant| instant + OUTPUT_IDLE_TIMEOUT);
        let chunk = if let Some(deadline) = idle_deadline {
            tokio::select! {
                _ = cancellation.cancelled() => return cancelled(),
                _ = sleep_until(deadline) => {
                    return Err("Z.ai 출력 수신이 5분 이상 멈춰 요청을 종료했습니다. 부분 답변을 보존하고 다시 시도해 주세요.".into());
                }
                chunk = stream.next() => chunk,
            }
        } else {
            tokio::select! {
                _ = cancellation.cancelled() => return cancelled(),
                chunk = stream.next() => chunk,
            }
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.map_err(|error| network_error(error, Provider::Zai))?;
        buffer.extend_from_slice(&chunk);
        if buffer.len() > MAX_SSE_FRAME_BYTES {
            return Err("Z.ai 스트림 프레임이 안전한 크기 제한을 초과했습니다.".into());
        }

        while let Some(frame) = take_sse_frame(&mut buffer) {
            if let FrameOutcome::Done = handle_frame(&frame, &mut state, emit)? {
                return finish_stream(request, &state);
            }
        }
    }

    if !buffer.is_empty() {
        if let Some((_, data)) = parse_sse_frame(&buffer) {
            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                merge_metadata(&mut state.metadata, &value);
            }
        }
    }

    Err(
        "Z.ai 스트림이 data: [DONE] 없이 종료되었습니다. 부분 답변을 보존하고 다시 시도해 주세요."
            .into(),
    )
}

pub(in crate::fugu) fn handle_frame<F>(
    frame: &[u8],
    state: &mut GlmStreamState,
    emit: &mut F,
) -> Result<FrameOutcome, String>
where
    F: FnMut(&str, &str),
{
    let Some((_, data)) = parse_sse_frame(frame) else {
        return Ok(FrameOutcome::Continue);
    };
    if data == "[DONE]" {
        if !state.saw_finish {
            return Err(
                "Z.ai 스트림이 완료 청크 없이 종료되었습니다. 부분 답변을 보존하고 다시 시도해 주세요."
                    .into(),
            );
        }
        return Ok(FrameOutcome::Done);
    }
    let Ok(value) = serde_json::from_str::<Value>(&data) else {
        return Ok(FrameOutcome::Continue);
    };
    state.last_output_at = Some(Instant::now());

    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .or_else(|| error.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("Z.ai가 요청 처리에 실패했습니다.");
        return Err(clean_remote_error(message));
    }
    if value.get("error").and_then(Value::as_str).is_some() {
        return Err(clean_remote_error(
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ));
    }
    if let Some(message) = value.get("message").and_then(Value::as_str) {
        if !message.is_empty() && value.get("choices").is_none() {
            return Err(clean_remote_error(message));
        }
    }

    if value.get("web_search").is_some() {
        emit("stage", "searching");
    }
    merge_metadata(&mut state.metadata, &value);

    if let Some(delta) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        if !state.writing_started {
            emit("stage", "writing");
            state.writing_started = true;
        }
        if state.answer.len().saturating_add(delta.len()) > MAX_ANSWER_BYTES {
            return Err("GLM 답변이 안전한 크기 제한을 초과했습니다.".into());
        }
        state.answer.push_str(delta);
        if !delta.is_empty() {
            state.last_output_at = Some(Instant::now());
        }
        emit("delta", delta);
    }

    match value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("stop") | Some("tool_calls") => state.saw_finish = true,
        Some("length") => {
            return Err("GLM 출력 토큰 한도에 도달해 응답이 끝까지 완성되지 않았습니다. 부분 답변을 보존하고 다시 시도해 주세요.".into());
        }
        Some("content_filter") => {
            return Err("GLM 응답이 안전 정책에 의해 중단되었습니다. 부분 답변을 확인한 뒤 요청을 조정해 주세요.".into());
        }
        _ => {}
    }
    Ok(FrameOutcome::Continue)
}

fn merge_metadata(metadata: &mut Value, chunk: &Value) {
    let Some(target) = metadata.as_object_mut() else {
        return;
    };
    if let Some(search) = chunk.get("web_search") {
        match target.get_mut("web_search") {
            Some(Value::Array(existing)) if search.is_array() => {
                existing.extend(search.as_array().cloned().unwrap_or_default());
            }
            _ => {
                target.insert("web_search".into(), search.clone());
            }
        }
    }
    if let Some(usage) = chunk.get("usage") {
        target.insert("usage".into(), usage.clone());
    }
}

pub(in crate::fugu) fn finish_stream(
    request: &ResearchRequest,
    state: &GlmStreamState,
) -> Result<ResearchResponse, String> {
    response_from_parts(request, &state.answer, &state.metadata)
}

fn complete_response(request: &ResearchRequest, value: &Value) -> Result<ResearchResponse, String> {
    let answer = extract_answer(value);
    if answer.trim().is_empty() {
        return Err("GLM이 빈 답변을 반환했습니다. 다시 시도해 주세요.".into());
    }
    response_from_parts(request, &answer, value)
}

fn response_from_parts(
    request: &ResearchRequest,
    answer: &str,
    metadata: &Value,
) -> Result<ResearchResponse, String> {
    if answer.trim().is_empty() {
        return Err("GLM이 빈 답변을 반환했습니다. 다시 시도해 주세요.".into());
    }
    Ok(ResearchResponse {
        request_id: request.request_id.clone(),
        answer: answer.to_owned(),
        sources: extract_sources(metadata),
        usage: extract_usage(metadata),
    })
}
