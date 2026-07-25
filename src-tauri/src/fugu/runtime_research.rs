use super::*;

impl FuguRuntime {
    pub fn cancel(&self, request_id: &str) -> Result<bool, String> {
        let active = self
            .active
            .lock()
            .map_err(|_| "요청 상태를 잠글 수 없습니다.".to_string())?;
        if let Some(token) = active.get(request_id) {
            token.cancel();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn research(
        &self,
        window: WebviewWindow,
        request: ResearchRequest,
    ) -> Result<ResearchResponse, String> {
        validate_research_request(&request)?;
        let key = self.key()?;
        let cancellation = CancellationToken::new();
        {
            let mut active = self
                .active
                .lock()
                .map_err(|_| "요청 상태를 잠글 수 없습니다.".to_string())?;
            if active.contains_key(&request.request_id) {
                return Err("같은 ID의 요청이 이미 실행 중입니다.".into());
            }
            active.insert(request.request_id.clone(), cancellation.clone());
        }

        let result = self
            .research_inner(&window, &request, key, cancellation)
            .await;
        if let Ok(mut active) = self.active.lock() {
            active.remove(&request.request_id);
        }
        result
    }

    async fn research_inner(
        &self,
        window: &WebviewWindow,
        request: &ResearchRequest,
        key: Zeroizing<String>,
        cancellation: CancellationToken,
    ) -> Result<ResearchResponse, String> {
        emit(window, &request.request_id, "stage", "connecting");

        let input = request
            .messages
            .iter()
            .map(|message| json!({"role": message.role, "content": message.content}))
            .collect::<Vec<_>>();
        let mut body = json!({
            "model": request.model,
            "input": input,
            "instructions": instructions(&request.mode),
            "reasoning": {"effort": request.reasoning},
            "max_output_tokens": output_limit(&request.mode),
            "stream": true
        });
        if matches!(request.mode.as_str(), "search" | "deep") {
            body["tools"] = json!([{"type": "web_search"}]);
            body["tool_choice"] = json!("auto");
        }

        let send = self
            .client
            .post(format!("{API_ROOT}/responses"))
            .bearer_auth(key.as_str())
            .json(&body)
            .send();
        let response = tokio::select! {
            _ = cancellation.cancelled() => return cancelled(window, &request.request_id),
            response = send => response.map_err(network_error)?,
        };
        if !response.status().is_success() {
            return Err(http_error(response).await);
        }

        if matches!(request.mode.as_str(), "search" | "deep") {
            emit(window, &request.request_id, "stage", "searching");
        } else if request.mode == "create" {
            emit(window, &request.request_id, "stage", "creating");
        } else {
            emit(window, &request.request_id, "stage", "reasoning");
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();

        let (answer, completed) = if content_type.contains("text/event-stream") {
            consume_stream(window, request, response, cancellation).await?
        } else {
            if response.content_length().unwrap_or(0) > MAX_RESPONSE_BYTES as u64 {
                return Err("Sakana 응답이 안전한 크기 제한을 초과했습니다.".into());
            }
            let bytes = tokio::select! {
                _ = cancellation.cancelled() => return cancelled(window, &request.request_id),
                bytes = response.bytes() => bytes.map_err(network_error)?,
            };
            if bytes.len() > MAX_RESPONSE_BYTES {
                return Err("Sakana 응답이 안전한 크기 제한을 초과했습니다.".into());
            }
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "Sakana 응답을 해석하지 못했습니다.".to_string())?;
            let answer = extract_answer(&value);
            (answer, Some(value))
        };

        if answer.trim().is_empty() {
            return Err("Fugu가 빈 답변을 반환했습니다. 다시 시도해 주세요.".into());
        }
        let sources = completed.as_ref().map(extract_sources).unwrap_or_default();
        let usage = completed.as_ref().and_then(extract_usage);
        emit(window, &request.request_id, "stage", "done");
        Ok(ResearchResponse {
            request_id: request.request_id.clone(),
            answer,
            sources,
            usage,
        })
    }
}
