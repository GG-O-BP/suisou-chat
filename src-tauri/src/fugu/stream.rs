use super::{MAX_ANSWER_BYTES, MAX_SSE_FRAME_BYTES};
use crate::models::ResearchRequest;
use futures_util::StreamExt;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::response::extract_answer;
use super::transport::{cancelled, clean_remote_error, network_error};

pub(super) async fn consume_stream<F>(
    request: &ResearchRequest,
    response: reqwest::Response,
    cancellation: CancellationToken,
    emit: &mut F,
) -> Result<(String, Option<Value>), String>
where
    F: FnMut(&str, &str) + Send,
{
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut streamed_answer = String::new();
    let mut completed = None;
    let mut writing_started = false;

    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return cancelled(),
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.map_err(network_error)?;
        buffer.extend_from_slice(&chunk);
        if buffer.len() > MAX_SSE_FRAME_BYTES {
            return Err("Sakana 스트림 프레임이 안전한 크기 제한을 초과했습니다.".into());
        }

        while let Some(frame) = take_sse_frame(&mut buffer) {
            let Some((event_name, data)) = parse_sse_frame(&frame) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let value: Value = match serde_json::from_str(&data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let event_type = if event_name.is_empty() {
                value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            } else {
                event_name.as_str()
            };

            if event_type.contains("web_search") {
                emit("stage", "searching");
            } else if request.mode == "create"
                && !writing_started
                && event_type.contains("reasoning")
            {
                emit("stage", "reasoning");
            }
            match event_type {
                "response.output_text.delta" => {
                    if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                        if !writing_started {
                            emit("stage", "writing");
                            writing_started = true;
                        }
                        if streamed_answer.len().saturating_add(delta.len()) > MAX_ANSWER_BYTES {
                            return Err("Fugu 답변이 안전한 크기 제한을 초과했습니다.".into());
                        }
                        streamed_answer.push_str(delta);
                        emit("delta", delta);
                    }
                }
                "response.completed" => {
                    completed = value.get("response").cloned().or(Some(value));
                }
                "response.failed" | "error" => {
                    let message = value
                        .pointer("/error/message")
                        .or_else(|| value.get("message"))
                        .and_then(Value::as_str)
                        .unwrap_or("Sakana가 요청 처리에 실패했습니다.");
                    return Err(clean_remote_error(message));
                }
                _ => {}
            }
        }
    }

    if !buffer.is_empty() {
        if let Some((_, data)) = parse_sse_frame(&buffer) {
            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                if value.get("type").and_then(Value::as_str) == Some("response.completed") {
                    completed = value.get("response").cloned().or(Some(value));
                }
            }
        }
    }

    if completed.is_none() {
        return Err(
            "연결이 완료 이벤트 전에 종료되었습니다. 부분 답변을 보존하고 다시 시도해 주세요."
                .into(),
        );
    }
    let completed_answer = completed.as_ref().map(extract_answer).unwrap_or_default();
    let answer = if completed_answer.trim().is_empty() {
        streamed_answer
    } else {
        completed_answer
    };
    Ok((answer, completed))
}

pub(super) fn take_sse_frame(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let mut found = None;
    for index in 0..buffer.len().saturating_sub(1) {
        if buffer[index] == b'\n' && buffer[index + 1] == b'\n' {
            found = Some((index, 2));
            break;
        }
        if index + 3 < buffer.len() && &buffer[index..index + 4] == b"\r\n\r\n" {
            found = Some((index, 4));
            break;
        }
    }
    let (index, delimiter_len) = found?;
    let frame = buffer[..index].to_vec();
    buffer.drain(..index + delimiter_len);
    Some(frame)
}

pub(super) fn parse_sse_frame(frame: &[u8]) -> Option<(String, String)> {
    let text = std::str::from_utf8(frame).ok()?;
    let mut event = String::new();
    let mut data = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            event = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start());
        }
    }
    if data.is_empty() {
        None
    } else {
        Some((event, data.join("\n")))
    }
}
