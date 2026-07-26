use crate::research_jobs::BackgroundExecution;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::research_jobs::NoopBackgroundExecution;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

#[cfg(target_os = "android")]
mod android {
    use super::*;
    use crate::models::ResearchJob;
    use jni::objects::{GlobalRef, JObject, JString, JValue};
    use jni::JavaVM;
    use std::sync::OnceLock;

    static SERVICE_CLASS: OnceLock<GlobalRef> = OnceLock::new();

    pub(super) struct AndroidBackgroundExecution;

    impl BackgroundExecution for AndroidBackgroundExecution {
        fn start(&self, job: &ResearchJob) -> Result<(), String> {
            send_service_intent("START", job)
        }

        fn update(&self, job: &ResearchJob) {
            let _ = send_service_intent("UPDATE", job);
        }

        fn stop(&self, request_id: &str, _succeeded: bool) {
            let job = ResearchJob {
                request_id: request_id.to_owned(),
                conversation_id: String::new(),
                workspace_revision: 0,
                workspace_persisted: false,
                assistant_message_id: String::new(),
                question: String::new(),
                mode: String::new(),
                status: String::new(),
                stage: String::new(),
                partial_answer: String::new(),
                result: None,
                error: None,
                created_at: 0,
                updated_at: 0,
            };
            let _ = send_service_intent("STOP", &job);
        }
    }

    fn send_service_intent(action: &str, job: &ResearchJob) -> Result<(), String> {
        let context = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }
            .map_err(|_| "Android VM에 연결하지 못했습니다.".to_string())?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|_| "Android 백그라운드 스레드를 연결하지 못했습니다.".to_string())?;
        let application_context = unsafe { JObject::from_raw(context.context().cast()) };
        let intent_class = env
            .find_class("android/content/Intent")
            .map_err(|_| "Android Intent 클래스를 찾지 못했습니다.".to_string())?;
        let service_class = service_class(&mut env)?;
        let service_class_object = service_class.as_obj();
        let intent = env
            .new_object(
                intent_class,
                "(Landroid/content/Context;Ljava/lang/Class;)V",
                &[
                    JValue::Object(&application_context),
                    JValue::Object(service_class_object),
                ],
            )
            .map_err(|_| "Android 연구 서비스 Intent를 만들지 못했습니다.".to_string())?;
        let action_value = format!("com.ggobp.suisou_chat.background.{action}");
        set_action(&mut env, &intent, &action_value)?;
        put_string_extra(&mut env, &intent, "request_id", &job.request_id)?;
        put_string_extra(&mut env, &intent, "stage", &job.stage)?;
        put_string_extra(&mut env, &intent, "status", &job.status)?;
        put_string_extra(&mut env, &intent, "mode", &job.mode)?;
        put_bool_extra(
            &mut env,
            &intent,
            "has_output",
            !job.partial_answer.is_empty(),
        )?;

        let (method, signature) = if action == "STOP" {
            ("stopService", "(Landroid/content/Intent;)Z")
        } else if action == "START" {
            (
                "startForegroundService",
                "(Landroid/content/Intent;)Landroid/content/ComponentName;",
            )
        } else {
            (
                "startService",
                "(Landroid/content/Intent;)Landroid/content/ComponentName;",
            )
        };
        env.call_method(
            &application_context,
            method,
            signature,
            &[JValue::Object(&intent)],
        )
        .map_err(|_| "Android 연구 서비스를 호출하지 못했습니다.".to_string())?;
        Ok(())
    }

    fn service_class(env: &mut jni::JNIEnv<'_>) -> Result<GlobalRef, String> {
        if let Some(class) = SERVICE_CLASS.get() {
            return Ok(class.clone());
        }
        let thread = env
            .call_static_method(
                "java/lang/Thread",
                "currentThread",
                "()Ljava/lang/Thread;",
                &[],
            )
            .and_then(|value| value.l())
            .map_err(|_| "Android 스레드 컨텍스트를 읽지 못했습니다.".to_string())?;
        let class_loader = env
            .call_method(
                &thread,
                "getContextClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )
            .and_then(|value| value.l())
            .map_err(|_| "Android 클래스 로더를 찾지 못했습니다.".to_string())?;
        let class_name = env
            .new_string("com.ggobp.suisou_chat.background.SuisouResearchService")
            .map_err(|_| "Android 연구 서비스 이름을 만들지 못했습니다.".to_string())?;
        let class_name = JObject::from(class_name);
        let class = env
            .call_method(
                class_loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&class_name)],
            )
            .and_then(|value| value.l())
            .map_err(|_| "Android 연구 서비스를 찾지 못했습니다.".to_string())?;
        let class = env
            .new_global_ref(class)
            .map_err(|_| "Android 연구 서비스 참조를 보존하지 못했습니다.".to_string())?;
        let _ = SERVICE_CLASS.set(class.clone());
        Ok(class)
    }

    fn set_action(
        env: &mut jni::JNIEnv<'_>,
        intent: &JObject<'_>,
        action: &str,
    ) -> Result<(), String> {
        let action = env
            .new_string(action)
            .map_err(|_| "Android 서비스 작업 이름을 만들지 못했습니다.".to_string())?;
        let action = JObject::from(action);
        env.call_method(
            intent,
            "setAction",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&action)],
        )
        .map_err(|_| "Android 서비스 작업 이름을 설정하지 못했습니다.".to_string())?;
        Ok(())
    }

    fn put_string_extra(
        env: &mut jni::JNIEnv<'_>,
        intent: &JObject<'_>,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let key = JString::from(
            env.new_string(key)
                .map_err(|_| "Android 서비스 키를 만들지 못했습니다.".to_string())?,
        );
        let value = JString::from(
            env.new_string(value)
                .map_err(|_| "Android 서비스 값을 만들지 못했습니다.".to_string())?,
        );
        let key = JObject::from(key);
        let value = JObject::from(value);
        env.call_method(
            intent,
            "putExtra",
            "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&key), JValue::Object(&value)],
        )
        .map_err(|_| "Android 서비스 값을 설정하지 못했습니다.".to_string())?;
        Ok(())
    }

    fn put_bool_extra(
        env: &mut jni::JNIEnv<'_>,
        intent: &JObject<'_>,
        key: &str,
        value: bool,
    ) -> Result<(), String> {
        let key = JString::from(
            env.new_string(key)
                .map_err(|_| "Android 서비스 키를 만들지 못했습니다.".to_string())?,
        );
        let key = JObject::from(key);
        env.call_method(
            intent,
            "putExtra",
            "(Ljava/lang/String;Z)Landroid/content/Intent;",
            &[JValue::Object(&key), JValue::Bool(value.into())],
        )
        .map_err(|_| "Android 서비스 값을 설정하지 못했습니다.".to_string())?;
        Ok(())
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use super::*;
    use crate::models::ResearchJob;
    use std::ffi::CString;
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn suisou_background_start(
            request_id: *const c_char,
            title: *const c_char,
            subtitle: *const c_char,
        ) -> bool;
        fn suisou_background_update(
            request_id: *const c_char,
            subtitle: *const c_char,
            completed: f64,
        );
        fn suisou_background_stop(request_id: *const c_char, succeeded: bool);
    }

    pub(super) struct IosBackgroundExecution;

    impl BackgroundExecution for IosBackgroundExecution {
        fn start(&self, job: &ResearchJob) -> Result<(), String> {
            let request_id = cstring(&job.request_id)?;
            let title = cstring("Suisou 연구 잠수")?;
            let subtitle = cstring(stage_label(job))?;
            let started = unsafe {
                suisou_background_start(request_id.as_ptr(), title.as_ptr(), subtitle.as_ptr())
            };
            if started {
                Ok(())
            } else {
                Err("iOS 백그라운드 연구 작업을 시작하지 못했습니다.".into())
            }
        }

        fn update(&self, job: &ResearchJob) {
            let Ok(request_id) = cstring(&job.request_id) else {
                return;
            };
            let Ok(subtitle) = cstring(stage_label(job)) else {
                return;
            };
            unsafe {
                suisou_background_update(
                    request_id.as_ptr(),
                    subtitle.as_ptr(),
                    stage_progress(job),
                );
            }
        }

        fn stop(&self, request_id: &str, succeeded: bool) {
            if let Ok(request_id) = cstring(request_id) {
                unsafe {
                    suisou_background_stop(request_id.as_ptr(), succeeded);
                }
            }
        }
    }

    fn cstring(value: &str) -> Result<CString, String> {
        CString::new(value).map_err(|_| "백그라운드 작업 문자열이 올바르지 않습니다.".into())
    }

    fn stage_progress(job: &ResearchJob) -> f64 {
        match job.stage.as_str() {
            "connecting" => 0.05,
            "searching" | "creating" => 0.3,
            "reasoning" => 0.55,
            "writing" => {
                if job.partial_answer.is_empty() {
                    0.7
                } else {
                    0.85
                }
            }
            "done" => 1.0,
            _ => 0.1,
        }
    }

    fn stage_label(job: &ResearchJob) -> &'static str {
        if !job.partial_answer.is_empty() {
            return if job.mode == "create" {
                "창작물을 쓰는 중"
            } else {
                "발견한 내용을 비추는 중"
            };
        }
        match job.stage.as_str() {
            "connecting" => "Sakana에 연결 중",
            "searching" => "웹 자료를 탐색하는 중",
            "creating" => "아이디어를 빚는 중",
            "reasoning" if job.mode == "create" => "구성을 다듬는 중",
            "reasoning" => "출처를 비교하는 중",
            "writing" if job.mode == "create" => "창작물을 쓰는 중",
            "writing" => "답변을 작성하는 중",
            _ => "연구를 계속하는 중",
        }
    }
}

pub fn execution<R: Runtime>(_app: &AppHandle<R>) -> Arc<dyn BackgroundExecution> {
    #[cfg(target_os = "android")]
    return Arc::new(android::AndroidBackgroundExecution);
    #[cfg(target_os = "ios")]
    return Arc::new(ios::IosBackgroundExecution);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Arc::new(NoopBackgroundExecution)
}
