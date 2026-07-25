use crate::models::{validate_workspace, Conversation, Workspace};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct LoadedWorkspace {
    pub workspace: Workspace,
    pub recovered_from_backup: bool,
    pub warning: Option<String>,
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

pub fn load_workspace(path: &Path) -> LoadedWorkspace {
    match read_valid_workspace(path) {
        Ok(workspace) => {
            return LoadedWorkspace {
                workspace,
                recovered_from_backup: false,
                warning: None,
            };
        }
        Err(primary_error) if path.exists() => {
            let backup = backup_path(path);
            if let Ok(workspace) = read_valid_workspace(&backup) {
                let restored = fs::copy(&backup, path).is_ok();
                return LoadedWorkspace {
                    workspace,
                    recovered_from_backup: restored,
                    warning: Some(if restored {
                        format!("저장 파일을 읽지 못해 백업을 복구했습니다: {primary_error}")
                    } else {
                        format!("백업 데이터는 읽었지만 기본 파일 복원에 실패했습니다. 읽기 전용으로 계속합니다: {primary_error}")
                    }),
                };
            }
            return LoadedWorkspace {
                workspace: Workspace::default(),
                recovered_from_backup: false,
                warning: Some(format!(
                    "기존 대화 기록을 읽지 못했습니다. 손상된 파일을 덮어쓰지 않도록 새 저장을 잠시 중지해 주세요: {primary_error}"
                )),
            };
        }
        Err(_) => {}
    }

    LoadedWorkspace {
        workspace: Workspace::default(),
        recovered_from_backup: false,
        warning: None,
    }
}

fn read_valid_workspace(path: &Path) -> Result<Workspace, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > 100 * 1024 * 1024 {
        return Err("대화 기록 파일이 100MB를 초과했습니다.".into());
    }
    let workspace: Workspace = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    validate_workspace(&workspace)?;
    Ok(workspace)
}

pub fn save_workspace(path: &Path, workspace: &Workspace) -> Result<(), String> {
    validate_workspace(workspace)?;
    let parent = path
        .parent()
        .ok_or_else(|| "대화 기록 경로가 올바르지 않습니다.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("저장 폴더 생성 실패: {error}"))?;

    let bytes = serde_json::to_vec_pretty(workspace)
        .map_err(|error| format!("대화 기록 직렬화 실패: {error}"))?;
    let temporary = parent.join(format!(
        ".workspace-{}-{}.tmp",
        std::process::id(),
        workspace.revision
    ));

    let mut file = fs::File::create(&temporary)
        .map_err(|error| format!("임시 저장 파일 생성 실패: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("대화 기록 쓰기 실패: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("대화 기록 동기화 실패: {error}"))?;

    if read_valid_workspace(path).is_ok() {
        fs::copy(path, backup_path(path)).map_err(|error| format!("백업 생성 실패: {error}"))?;
    }

    if let Err(first_error) = fs::rename(&temporary, path) {
        if path.exists() {
            #[cfg(target_os = "windows")]
            {
                let previous = backup_path(path);
                fs::rename(path, &previous)
                    .map_err(|error| format!("기존 대화 기록 보존 실패: {error}"))?;
                if let Err(error) = fs::rename(&temporary, path) {
                    let _ = fs::rename(&previous, path);
                    return Err(format!(
                        "대화 기록 확정 실패: {error}; 최초 오류: {first_error}"
                    ));
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = fs::remove_file(&temporary);
                return Err(format!("대화 기록 확정 실패: {first_error}"));
            }
        } else {
            let _ = fs::remove_file(&temporary);
            return Err(format!("대화 기록 확정 실패: {first_error}"));
        }
    }
    let _ = fs::copy(path, backup_path(path));
    Ok(())
}

pub fn conversation_markdown(conversation: &Conversation) -> String {
    let mut output = format!("# {}\n\n", conversation.title.trim());
    for message in &conversation.messages {
        let heading = if message.role == "user" {
            "질문"
        } else {
            "답변"
        };
        output.push_str(&format!("## {heading}\n\n{}\n\n", message.content.trim()));
        if !message.sources.is_empty() {
            output.push_str("### 출처\n\n");
            for (index, source) in message.sources.iter().enumerate() {
                output.push_str(&format!(
                    "{}. [{}]({}) — {}\n",
                    index + 1,
                    source.title.replace(['[', ']'], ""),
                    source.url,
                    source.domain
                ));
            }
            output.push('\n');
        }
    }
    output.push_str("---\nSuisou에서 내보냄\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Message, Source};
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_workspace() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let workspace = Workspace {
            revision: 7,
            ..Workspace::default()
        };
        save_workspace(&path, &workspace).unwrap();

        let loaded = load_workspace(&path);
        assert_eq!(loaded.workspace.revision, 7);
        assert!(!loaded.recovered_from_backup);
    }

    #[test]
    fn later_saves_replace_the_workspace_and_refresh_the_backup() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let first = Workspace {
            revision: 1,
            conversations: vec![Conversation {
                id: "kept-first".into(),
                ..Conversation::default()
            }],
            ..Workspace::default()
        };
        save_workspace(&path, &first).unwrap();

        let second = Workspace {
            revision: 2,
            conversations: vec![Conversation {
                id: "kept-second".into(),
                ..Conversation::default()
            }],
            ..Workspace::default()
        };
        save_workspace(&path, &second).unwrap();

        let loaded = load_workspace(&path);
        assert_eq!(loaded.workspace, second);
        assert_eq!(read_valid_workspace(&backup_path(&path)).unwrap(), second);
    }

    #[test]
    fn recovers_valid_backup_when_primary_is_corrupt() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let backup = backup_path(&path);
        let workspace = Workspace {
            revision: 4,
            ..Workspace::default()
        };
        fs::write(&backup, serde_json::to_vec(&workspace).unwrap()).unwrap();
        fs::write(&path, b"not-json").unwrap();

        let loaded = load_workspace(&path);
        assert_eq!(loaded.workspace.revision, 4);
        assert!(loaded.recovered_from_backup);
        assert_eq!(read_valid_workspace(&path).unwrap().revision, 4);
    }

    #[test]
    fn markdown_export_contains_messages_and_sources() {
        let conversation = Conversation {
            title: "검증 기록".into(),
            messages: vec![Message {
                role: "assistant".into(),
                content: "확인된 답변".into(),
                sources: vec![Source {
                    title: "공식 문서".into(),
                    url: "https://example.com/docs".into(),
                    domain: "example.com".into(),
                    ..Source::default()
                }],
                ..Message::default()
            }],
            ..Conversation::default()
        };
        let markdown = conversation_markdown(&conversation);
        assert!(markdown.contains("확인된 답변"));
        assert!(markdown.contains("[공식 문서](https://example.com/docs)"));
    }
}
