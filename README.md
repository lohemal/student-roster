# 학생명단 관리 시스템

**초등학교** 학생명단을 한 곳에서 관리하는 Windows 프로그램입니다.
학생정보를 한 번만 등록하면 검색·전입·전출·형제 탐색·주소 분류·통계·진급·명단 생성을 프로그램이 처리합니다.

> 자료는 **이 컴퓨터 안에만** 저장됩니다. 외부 서버나 AI 서비스로 보내지 않습니다.

두 가지 원칙으로 만듭니다.

- 학생정보는 한 번만 입력하고, 필요한 명단은 프로그램이 만든다.
- 원본 데이터는 보존하고, 시스템이 정리한 값을 따로 둔다.

설계와 진행 계획은 [docs/01-설계안.md](docs/01-설계안.md)에 있습니다.

---

## 진행 상황

| Phase | 내용 | 상태 |
|---|---|---|
| 0 | 프로젝트 골격 · DB 스키마 · 화면 셸 · 설정/학년도 | ✅ |
| 1 | 학생 등록/수정 · 학생명단 검색 | |
| 2 | Excel 가져오기(진행률) | |
| 3 | 주소 분류 · 주소 규칙 | |
| 4 | 본교 형제 · 보호자 정보 보완 | |
| 5 | 확인 필요 · 번호 재정렬 | |
| 6 | 전입생 · 전출생 | |
| 7 | 통계 | |
| 8 | 파일 내보내기 · 외부 시스템 양식 | |
| 9 | 학년도 전환 · 졸업생 | |
| 10 | 백업/복원 · 업데이트 · 배포 | |

---

## 만드는 방법 (개발)

### 필요한 것

- [Node.js](https://nodejs.org) 20 이상
- [Rust](https://rustup.rs)
- Visual Studio Build Tools (C++ 데스크톱 개발)

### 실행

```bash
npm install
npm run app
```

### 실제 자료를 건드리지 않고 연습하기

```bash
npm run app:sandbox
```

자료를 `%APPDATA%\kr.school.studentroster.sandbox\` 에 따로 만들므로
실제 자료(`kr.school.studentroster`)는 손대지 않습니다. 창 제목에 **연습용**이라고 나옵니다.

### 검사

```bash
npm run build           # 타입 검사 + 화면 빌드
npm run check:model     # 화면 쪽 순수 함수 검사
cd src-tauri && cargo test --lib   # 서버 테스트
```

### 구조

```
src/                 React 화면
  components/        AppShell(사이드바) · ui(Page/Card/Button/Badge …)
  ipc/               Rust 명령 호출 래퍼 (Rust commands/ 와 1:1)
  lib/               화면 쪽 순수 함수
  pages/             화면
src-tauri/
  migrations/        SQL 마이그레이션 (번호순, 배포 후 수정 금지)
  src/commands/      Tauri 명령 (입력 검증만)
  src/repo/          SQL (저장소 계층)
  src/domain/        순수 계산 규칙 (정규화·판정·재정렬·진급)
  src/db/            연결 · 마이그레이션 러너 · 백업
docs/                설계안
```

### 버전 올리기

```bash
npm run version:set 0.2.0
```

`package.json` · `tauri.conf.json` · `Cargo.toml` 세 곳을 한 번에 맞춥니다.
