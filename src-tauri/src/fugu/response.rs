use crate::models::{Source, Usage};
use serde_json::Value;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

use super::transport::truncate_chars;

pub(super) fn extract_answer(value: &Value) -> String {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        return text.to_owned();
    }
    let mut parts = Vec::new();
    collect_output_text(value, &mut parts);
    parts.join("\n")
}

fn collect_output_text(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_output_text(value, parts);
            }
        }
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("output_text") {
                if let Some(text) = object.get("text").and_then(Value::as_str) {
                    parts.push(text.to_owned());
                    return;
                }
            }
            for value in object.values() {
                collect_output_text(value, parts);
            }
        }
        _ => {}
    }
}

pub(super) fn extract_sources(value: &Value) -> Vec<Source> {
    let mut candidates = Vec::new();
    collect_sources(value, &mut candidates);
    let mut seen = HashSet::new();
    let now = now_millis();
    candidates
        .into_iter()
        .filter_map(|candidate| {
            let mut url = Url::parse(candidate.url.as_str()).ok()?;
            if url.scheme() != "https" || url.host_str().is_none() {
                return None;
            }
            url.set_fragment(None);
            let canonical = url.to_string();
            if !seen.insert(canonical.clone()) {
                return None;
            }
            let domain = url.host_str()?.trim_start_matches("www.").to_owned();
            let index = seen.len();
            Some(Source {
                id: format!("source-{index}"),
                title: truncate_chars(
                    if candidate.title.trim().is_empty() {
                        &domain
                    } else {
                        &candidate.title
                    },
                    180,
                ),
                url: canonical,
                domain,
                snippet: truncate_chars(&candidate.snippet, 360),
                retrieved_at: now,
            })
        })
        .take(100)
        .collect()
}

struct SourceCandidate {
    title: String,
    url: String,
    snippet: String,
}

fn collect_sources(value: &Value, output: &mut Vec<SourceCandidate>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_sources(value, output);
            }
        }
        Value::Object(object) => {
            let kind = object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let likely_source = kind.contains("citation")
                || kind.contains("source")
                || kind.contains("search_result")
                || object.contains_key("snippet");
            if likely_source {
                if let Some(url) = object.get("url").and_then(Value::as_str) {
                    output.push(SourceCandidate {
                        title: object
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        url: url.to_owned(),
                        snippet: object
                            .get("snippet")
                            .or_else(|| object.get("description"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    });
                }
            }
            for value in object.values() {
                collect_sources(value, output);
            }
        }
        _ => {}
    }
}

pub(super) fn extract_usage(value: &Value) -> Option<Usage> {
    let usage = value.get("usage")?;
    let input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens + output_tokens);
    let orchestration_tokens = usage
        .get("orchestration_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .pointer("/output_tokens_details/reasoning_tokens")
                .and_then(Value::as_u64)
        })
        .unwrap_or(0);
    Some(Usage {
        input_tokens,
        output_tokens,
        total_tokens,
        orchestration_tokens,
    })
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
