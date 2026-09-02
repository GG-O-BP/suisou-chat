# Suisou

Suisou는 Sakana Fugu와 Z.ai GLM을 선택해 쓸 수 있는 로컬 우선 AI 리서치 워크스페이스입니다. Tauri 2와 Sycamore 0.9로 작성되어 데스크톱과 모바일 셸을 공유하며, 검색형 답변을 빠르게 읽고 출처를 다시 검토하는 흐름에 맞춘 한국어 UI를 제공합니다.

> API 접근 권한은 계정별로 달라질 수 있습니다. Sakana는 `/v1/models`, `/v1/responses`, `web_search`, Responses SSE 이벤트를 사용합니다. Z.ai는 GLM Coding Plan의 OpenAI Chat Completions 호환 endpoint `/api/coding/paas/v4/chat/completions`, `web_search`, Chat Completions SSE를 사용하며 초기 지원 모델은 `glm-5.3`입니다. 배포 전 두 provider의 실제 키로 모델·도구 권한과 과금·약관·지역 정책을 확인하세요.

## 구현 범위

- 빠른 답변, 웹 검색, 심층 연구의 3가지 모드
- Fugu/Fugu Ultra/GLM 모델과 provider별 추론 강도 선택, 대화 문맥 전송
- 네이티브 Rust HTTPS 클라이언트, SSE 델타·단계·출처 스트리밍
- 요청 취소, 부분 답변 보존, 오류 후 재시도, 인증 만료 안내
- 인용 출처 패널, HTTPS URL 검증, 외부 브라우저 열기
- 대화 검색, 고정, 삭제, Markdown 내보내기
- 원자적 JSON 저장, 백업 복구, 손상 파일 읽기 전용 보호, 오프라인 기록 검색
- 시스템·라이트·다크 테마, 반응형 레이아웃, 키보드·스크린리더 레이블
- 운영체제 보안 저장소에 provider별로 보관하고 앱 시작 시 자동 복원하는 API 키
- 외부 폰트/CDN 없는 오프라인 자산과 제한된 Tauri 권한/CSP

## 프라이버시 경계

대화 기록과 설정은 기기 앱 데이터 디렉터리의 `workspace.json`에 저장됩니다. Sakana 키와 Z.ai 키는 `workspace.json`, 브라우저 저장소, 로그가 아니라 운영체제 보안 자격 증명 저장소의 서로 다른 항목에 저장되며 앱 시작 시 자동 복원됩니다. Linux에서는 Secret Service, Windows에서는 Credential Manager, macOS·iOS에서는 Keychain, Android에서는 Keystore로 보호되는 저장소를 사용합니다. 질문과 선택된 이전 대화는 답변 생성을 위해 선택한 provider API로 전송됩니다. 웹 검색 모드에서는 해당 provider의 검색 서비스가 질의를 처리하므로 민감한 정보는 입력하지 마세요.

이 버전은 계정·클라우드 동기화를 의도적으로 포함하지 않습니다. 기록 열람·검색은 오프라인에서도 가능하지만 새 답변과 웹 출처 열기는 네트워크가 필요합니다.

## 개발 환경

필수 도구:

- 안정 Rust 툴체인과 `wasm32-unknown-unknown` 타깃
- Trunk 0.21 이상
- Tauri 2 CLI와 플랫폼별 시스템 의존성

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cargo install tauri-cli --version '^2' --locked
```

플랫폼별 사전 준비는 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)를 따릅니다. Android/iOS는 Tauri 2 툴체인과 SDK가 설치된 호스트에서 생성·빌드하세요.

## 실행과 빌드

```bash
cargo tauri dev
trunk build --release
cargo tauri build --no-bundle
cargo tauri build
```

### E2E 및 성능 테스트

```bash
npm ci
npm run e2e:doctor
npm run e2e
```

결정론적 브라우저 기능 테스트, 렌더링 성능 예산, 실제 Tauri/Rust IPC
테스트를 실행합니다. 실제 provider 요청은 별도의 opt-in 명령입니다.

```bash
SUISOU_E2E_LIVE=1 npm run e2e:live
SUISOU_E2E_LIVE=1 npm run e2e:live:glm
```

`e2e:live`는 Sakana와 GLM을 모두 실행하고, `e2e:live:glm`은 GLM
Coding Plan 요청만 실행합니다. 상세 구조와 CI/보안 경계는
[`docs/e2e.md`](docs/e2e.md)를 참고하세요.

앱을 연 뒤 설정에서 사용할 provider의 API 키를 각각 입력하고 **연결**을
누르세요. Sakana는 모델 목록으로 키를 검증합니다. Z.ai는 키 형식과 정적
카탈로그를 확인한 뒤 첫 요청으로 Coding Plan 계정 권한을 검증합니다.
Coding Plan 구독은 공식 지원 도구/제품 환경으로 사용이 제한될 수 있으므로
Z.ai 약관과 계정 상태를 확인하세요.

### Android 빌드

이 저장소의 생성된 Android 프로젝트는 Android SDK 36, NDK `29.0.13846066`, JDK 21을 기준으로 합니다. 이 환경에서는 필요한 SDK와 Rust Android 타깃이 이미 설치되어 있으므로 다음 스크립트를 사용하면 됩니다.

```bash
# arm64 디버그 APK
./scripts/android-build-debug.sh

# arm64 릴리스 APK와 AAB
./scripts/android-build-release.sh
```

디버그 APK는 다음 경로에 생성됩니다.

```text
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

직접 명령을 실행하려면 먼저 환경을 불러옵니다.

```bash
source scripts/android-env.sh
cargo tauri android build --debug --apk --target aarch64 --ci
```

새 머신이라면 Android command-line tools를 `$HOME/Android/Sdk`에 설치한 뒤 다음 패키지와 Rust 타깃을 준비하세요.

```bash
"$HOME/Android/Sdk/cmdline-tools/latest/bin/sdkmanager" \
  --sdk_root="$HOME/Android/Sdk" \
  "platform-tools" \
  "platforms;android-36" \
  "build-tools;35.0.0" \
  "ndk;29.0.13846066"

rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

`src-tauri/gen/android`는 이미 초기화되어 있고 `MainActivity.kt`에 API 키 보안 저장소 초기화 코드가 포함되어 있습니다. 따라서 해당 코드를 보존하려면 `cargo tauri android init`을 다시 실행하지 마세요.

실기기 설치는 USB 디버깅을 활성화하고 다음처럼 진행합니다.

```bash
source scripts/android-env.sh
adb devices
adb install -r \
  src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

디버그 APK는 Android 디버그 키로 자동 서명됩니다. Play Store 배포용 AAB는 별도의 업로드 키스토어와 Gradle signing configuration을 설정해야 합니다. 키스토어와 비밀번호 파일은 저장소에 커밋하지 마세요.

## 품질 검사

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
trunk build --release
cargo tauri build --no-bundle
```

현재 검증 기준에서는 Clippy가 경고 없이 통과하고 53개 작업 공간 테스트가 통과합니다. provider 호환성, GLM Coding Plan 요청/스트리밍/출처/사용량 매핑, 요청 검증, UTF-8 분할/CRLF SSE, API 키 보안 저장·복원·삭제 오류, 작업 공간 저장·백업 복구, Markdown 내보내기를 검사합니다.

## 출시 전 체크리스트

- 실제 Sakana 및 GLM Coding Plan 키로 빠른/검색/심층 모드, 모델명, 웹 검색 스키마를 확인
- 취소·재시도·인증 실패·오프라인·재시작 복구를 대상 OS에서 확인
- Windows/macOS/Linux 번들 서명과 자동 업데이트 정책 결정
- Android/iOS 권한, 패키지 ID(`com.ggobp.suisou-chat`), 앱 링크·공유 UX 확인
- 접근성 도구, 작은 화면, 긴 다국어 답변, 매우 큰 대화 점검
- 계정 동기화를 추가한다면 암호화, 충돌 해결, 삭제/내보내기 정책을 먼저 설계

## 알려진 제한

- 운영체제 보안 저장소가 잠겨 있거나 제공되지 않는 환경에서는 API 키 저장·복원이 실패하며, 평문 파일 저장으로 자동 전환하지 않습니다.
- Markdown은 안전한 일반 텍스트로 표시됩니다. 완전한 서식 렌더링과 문장 단위 인용 매핑은 후속 항목입니다.
- 기기 간 동기화, 음성, 파일 첨부, 팀 공유, 예약 연구에는 백엔드와 별도 개인정보 설계가 필요합니다.
- 모바일 Markdown 내보내기는 앱 문서 디렉터리에 저장되며 네이티브 공유 시트는 아직 연결하지 않았습니다.
