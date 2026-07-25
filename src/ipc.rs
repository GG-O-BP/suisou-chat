use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], catch)]
    async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    async fn listen_raw(event: &str, handler: &js_sys::Function) -> Result<JsValue, JsValue>;
}

pub struct EventListener {
    unlisten: js_sys::Function,
    _handler: Closure<dyn FnMut(JsValue)>,
}

impl EventListener {
    pub fn unlisten(self) {
        let _ = self.unlisten.call0(&JsValue::UNDEFINED);
    }
}

#[derive(Deserialize)]
struct EventEnvelope<T> {
    payload: T,
}

pub async fn command<A, R>(name: &str, args: &A) -> Result<R, String>
where
    A: Serialize + ?Sized,
    R: DeserializeOwned,
{
    let args = serde_wasm_bindgen::to_value(args)
        .map_err(|_| "앱 요청을 준비하지 못했습니다.".to_string())?;
    let response = invoke(name, args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(response)
        .map_err(|_| "앱 응답 형식을 읽지 못했습니다.".to_string())
}

pub async fn command_unit<A>(name: &str, args: &A) -> Result<(), String>
where
    A: Serialize + ?Sized,
{
    let args = serde_wasm_bindgen::to_value(args)
        .map_err(|_| "앱 요청을 준비하지 못했습니다.".to_string())?;
    invoke(name, args).await.map_err(js_error)?;
    Ok(())
}

pub async fn listen<T, F>(event: &str, mut callback: F) -> Result<EventListener, String>
where
    T: DeserializeOwned + 'static,
    F: FnMut(T) + 'static,
{
    let closure = Closure::<dyn FnMut(JsValue)>::new(move |event| {
        if let Ok(envelope) = serde_wasm_bindgen::from_value::<EventEnvelope<T>>(event) {
            callback(envelope.payload);
        }
    });
    let unlisten = listen_raw(event, closure.as_ref().unchecked_ref())
        .await
        .map_err(js_error)?;
    Ok(EventListener {
        unlisten: unlisten
            .dyn_into::<js_sys::Function>()
            .map_err(|_| "이벤트 정리 함수를 읽지 못했습니다.".to_string())?,
        _handler: closure,
    })
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_sys::JSON::stringify(&error).ok()?.as_string())
        .unwrap_or_else(|| "네이티브 앱 요청이 실패했습니다.".into())
}
