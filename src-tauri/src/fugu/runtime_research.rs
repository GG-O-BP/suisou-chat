use super::*;

impl FuguRuntime {
    pub async fn research<F>(
        &self,
        request: ResearchRequest,
        cancellation: CancellationToken,
        mut emit: F,
    ) -> Result<ResearchResponse, String>
    where
        F: FnMut(&str, &str) + Send,
    {
        validate_research_request(&request)?;
        let provider = provider_for_model(&request.model)?;
        let key = self.key(provider)?;
        match provider {
            Provider::Sakana => {
                self.research_sakana(&request, key, cancellation, &mut emit)
                    .await
            }
            Provider::Zai => {
                self.research_zai(&request, key, cancellation, &mut emit)
                    .await
            }
        }
    }

    async fn research_sakana<F>(
        &self,
        request: &ResearchRequest,
        key: Zeroizing<String>,
        cancellation: CancellationToken,
        emit: &mut F,
    ) -> Result<ResearchResponse, String>
    where
        F: FnMut(&str, &str) + Send,
    {
        emit("stage", "connecting");

        let input = request
            .messages
            .iter()
            .map(|message| json!({"role": message.role, "content": message.content}))
            .collect::<Vec<_>>();
        let mut body = json!({
            "model": request.model,
            "input": input,
            "instructions": instructions(&request.mode, Provider::Sakana),
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
            _ = cancellation.cancelled() => return cancelled(),
            response = send => response.map_err(|error| network_error(error, Provider::Sakana))?,
        };
        if !response.status().is_success() {
            return Err(http_error(response, Provider::Sakana).await);
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

        let (answer, completed) = if content_type.contains("text/event-stream") {
            consume_stream(request, response, cancellation, emit).await?
        } else {
            if response.content_length().unwrap_or(0) > MAX_RESPONSE_BYTES as u64 {
                return Err("Sakana 응답이 안전한 크기 제한을 초과했습니다.".into());
            }
            let bytes = tokio::select! {
                _ = cancellation.cancelled() => return cancelled(),
                bytes = response.bytes() => bytes.map_err(|error| network_error(error, Provider::Sakana))?,
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
        Ok(ResearchResponse {
            request_id: request.request_id.clone(),
            answer,
            sources,
            usage,
        })
    }

    async fn research_zai<F>(
        &self,
        request: &ResearchRequest,
        key: Zeroizing<String>,
        cancellation: CancellationToken,
        emit: &mut F,
    ) -> Result<ResearchResponse, String>
    where
        F: FnMut(&str, &str) + Send,
    {
        zai::research(&self.client, request, key, cancellation, &mut *emit).await
    }
}
