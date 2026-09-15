-- 학생명단 관리 시스템 — 초기 스키마 (설계안 §3)
--
-- ⚠️ v0.1.0 릴리스로 **동결**. 이 파일은 더 이상 고치지 않는다.
--    이미 이 구조로 자료를 만든 사용자가 있으므로, 고치면 그 자료와 어긋난다.
--    바꿀 일이 생기면 002_*.sql 을 새로 만들고 db::migrate::MIGRATIONS 에 한 줄
--    더한다. 러너가 user_version 보다 높은 번호만 차례로 적용하고, 적용 직전에
--    자동으로 백업한다(before_migration_*.db).
--
-- 원칙
--   * 원본(raw)과 정리값(norm/digits)을 나란히 둔다.
--   * 삭제 대신 상태 변경. 학년도별 학적은 덮어쓰지 않고 행을 추가한다.
--   * 자동 판정의 근거(source / rule_id / detail)를 함께 저장한다.

-- ---------------------------------------------------------------
-- 설정 · 학년도
-- ---------------------------------------------------------------

CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE school_years (
  year       INTEGER PRIMARY KEY,                       -- 2026
  is_current INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
  note       TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
-- 현재 학년도는 하나만
CREATE UNIQUE INDEX ux_school_years_current ON school_years(is_current) WHERE is_current = 1;

-- ---------------------------------------------------------------
-- 주소 분류 · 규칙
-- ---------------------------------------------------------------

CREATE TABLE address_categories (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,                      -- '1단지', '주택', '기타'
  kind       TEXT NOT NULL DEFAULT 'COMPLEX' CHECK (kind IN ('COMPLEX', 'HOUSE', 'OTHER')),
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_builtin INTEGER NOT NULL DEFAULT 0 CHECK (is_builtin IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

INSERT INTO address_categories (name, kind, sort_order, is_builtin) VALUES
  ('주택', 'HOUSE', 900, 1),
  ('기타', 'OTHER', 999, 1);

-- 주소 → 분류 규칙. 믿음직한 순서대로 ROAD > COMPLEX > CONTAINS 로 본다.
--
--   ROAD     도로명 + 건물번호가 **정확히** 같을 때만. '○○로 12' 와 '○○로 123' 은 다르다
--   COMPLEX  단지 이름이 들어 있을 때 (공백 차이는 무시)
--   CONTAINS 단순 포함. 넓게 걸리므로 가장 마지막에 본다
--
-- 몇 명에게 적용됐는지는 세어 두지 않고 `students.address_rule_id` 를 그때그때 센다.
-- 세어 둔 값은 언젠가 실제와 어긋나지만, 세는 것은 어긋나지 않는다.
CREATE TABLE address_rules (
  id          INTEGER PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('ROAD', 'COMPLEX', 'CONTAINS')),
  pattern     TEXT NOT NULL,                            -- ROAD: '한누리대로 123' / COMPLEX: '가온마을5단지' / CONTAINS: 임의 문자열
  category_id INTEGER NOT NULL REFERENCES address_categories(id) ON DELETE CASCADE,
  source      TEXT NOT NULL DEFAULT 'USER' CHECK (source IN ('USER', 'BUILTIN')),
  is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  note        TEXT,
  created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  UNIQUE (kind, pattern)
);

-- ---------------------------------------------------------------
-- 학생 기본정보 (사람 한 명 = 한 행)
-- ---------------------------------------------------------------

CREATE TABLE students (
  id                    INTEGER PRIMARY KEY,
  name                  TEXT NOT NULL,
  gender                TEXT CHECK (gender IS NULL OR gender IN ('M', 'F')),

  birth_raw             TEXT,                           -- 입력 원문
  birth_date            TEXT,                           -- YYYY-MM-DD (정규화 실패면 NULL + BIRTH issue)

  address_raw           TEXT,                           -- 입력 원문 (절대 수정하지 않음)
  address_norm          TEXT,                           -- 공백·괄호 정리본 (검색·규칙 매칭용)
  address_road          TEXT,                           -- 뽑아낸 '도로명 건물번호' (동·호수는 뺀다)
  address_category_id   INTEGER REFERENCES address_categories(id) ON DELETE SET NULL,
  -- 분류를 **어떻게 정했는지**. 통계에서 네 가지를 갈라 봐야 한다.
  --   NONE     아직 정하지 못함 (주소가 없거나, 걸리는 규칙이 없음)
  --   AUTO     주소에 적힌 단지 이름으로 저절로
  --   RULE     사용자가 만든 주소 규칙으로
  --   MANUAL   사용자가 이 학생만 직접 지정 — 자동 재적용이 절대 건드리지 않는다
  --   CONFLICT 같은 순위 규칙이 서로 다른 분류를 가리켜 정하지 못함
  address_source        TEXT NOT NULL DEFAULT 'NONE' CHECK (address_source IN ('NONE', 'AUTO', 'RULE', 'MANUAL', 'CONFLICT')),
  address_rule_id       INTEGER REFERENCES address_rules(id) ON DELETE SET NULL,

  father_name           TEXT,
  mother_name           TEXT,
  father_phone          TEXT,                           -- 표시용 010-1234-5678
  father_phone_digits   TEXT,                           -- 검색·비교용 01012345678
  mother_phone          TEXT,
  mother_phone_digits   TEXT,
  primary_phone         TEXT,                           -- 주보호자 연락처
  primary_phone_digits  TEXT,

  note                  TEXT,
  created_at            TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  updated_at            TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX ix_students_name           ON students(name);
CREATE INDEX ix_students_birth          ON students(birth_date);
CREATE INDEX ix_students_father_digits  ON students(father_phone_digits);
CREATE INDEX ix_students_mother_digits  ON students(mother_phone_digits);
CREATE INDEX ix_students_primary_digits ON students(primary_phone_digits);
CREATE INDEX ix_students_addr_cat       ON students(address_category_id);
CREATE INDEX ix_students_addr_road      ON students(address_road);
CREATE INDEX ix_students_addr_rule      ON students(address_rule_id);

-- ---------------------------------------------------------------
-- 학년도별 학적
-- ---------------------------------------------------------------

-- 한 학년도에 한 행. **현재 상태**를 담는다(이력이 아니다).
-- 전입일·전출일은 가장 최근 값의 사본이며, 이동 이력의 원본은 enrollment_events 다.
CREATE TABLE enrollments (
  id                 INTEGER PRIMARY KEY,
  student_id         INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  school_year        INTEGER NOT NULL REFERENCES school_years(year),
  grade              INTEGER NOT NULL CHECK (grade BETWEEN 1 AND 6),
  class_name         TEXT,                              -- '1', '가람' … NULL = 반배정 미정
  class_no           INTEGER CHECK (class_no IS NULL OR class_no >= 1),
  status             TEXT NOT NULL DEFAULT 'ENROLLED' CHECK (status IN ('ENROLLED', 'TRANSFER_IN', 'TRANSFER_OUT')),
  transfer_in_date   TEXT,                              -- 가장 최근 전입일 (원본은 events)
  transfer_out_date  TEXT,                              -- 가장 최근 전출일 (원본은 events)
  transfer_out_to    TEXT,                              -- 전출 간 학교·지역 (아는 만큼만)
  transfer_out_note  TEXT,
  created_at         TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  updated_at         TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  UNIQUE (student_id, school_year)
);

CREATE INDEX ix_enroll_year_class ON enrollments(school_year, grade, class_name, class_no);
CREATE INDEX ix_enroll_status     ON enrollments(school_year, status);

-- ---------------------------------------------------------------
-- 학적 이동 이력 — 덧붙이기만 한다 (고치거나 지우지 않는다)
--
-- 같은 학년도에 전출했다가 다시 전입하는 일이 실제로 있다. enrollments 는
-- 한 학년도에 한 행이므로 그것만으로는 두 번째 이동이 첫 번째를 덮어쓴다.
-- 그래서 이동은 여기에 사건으로 남기고, enrollments 에는 현재 상태만 둔다.
-- 학년/반/번호는 그 시점 스냅샷이라 나중에 반이 바뀌어도 기록이 흐트러지지 않는다.
-- ---------------------------------------------------------------

CREATE TABLE enrollment_events (
  id           INTEGER PRIMARY KEY,
  student_id   INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  school_year  INTEGER NOT NULL,
  kind         TEXT NOT NULL CHECK (kind IN (
                 'ENROLL',        -- 최초 등록
                 'TRANSFER_IN',   -- 전입 (재전입 포함)
                 'TRANSFER_OUT',  -- 전출
                 'PROMOTE',       -- 학년도 전환으로 진급
                 'GRADUATE',      -- 졸업
                 'CANCEL')),      -- 앞선 처리 취소·정정
  event_date   TEXT,                                    -- 실제 발생일. 모르면 NULL
  grade        INTEGER,                                 -- 그 시점 스냅샷
  class_name   TEXT,
  class_no     INTEGER,
  note         TEXT,
  source       TEXT NOT NULL DEFAULT 'MANUAL' CHECK (source IN ('MANUAL', 'IMPORT', 'TRANSITION')),
  created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX ix_events_student ON enrollment_events(student_id, school_year, id);
CREATE INDEX ix_events_year    ON enrollment_events(school_year, kind);

-- ---------------------------------------------------------------
-- 졸업 기록
-- ---------------------------------------------------------------

CREATE TABLE graduations (
  student_id    INTEGER PRIMARY KEY REFERENCES students(id) ON DELETE CASCADE,
  school_year   INTEGER NOT NULL REFERENCES school_years(year),   -- 졸업한 학년도 (6학년이었던 해)
  graduated_at  TEXT,
  note          TEXT,
  created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX ix_graduations_year ON graduations(school_year);

-- ---------------------------------------------------------------
-- 형제 연결
-- ---------------------------------------------------------------

CREATE TABLE sibling_links (
  id               INTEGER PRIMARY KEY,
  student_a        INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  student_b        INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  status           TEXT NOT NULL CHECK (status IN ('CANDIDATE', 'CONFIRMED', 'REJECTED')),
  source           TEXT NOT NULL DEFAULT 'AUTO' CHECK (source IN ('AUTO', 'MANUAL')),
  matched_fields   TEXT NOT NULL DEFAULT '[]',          -- JSON: ["motherName","motherPhone"]
  conflict_fields  TEXT NOT NULL DEFAULT '[]',          -- JSON: 값이 둘 다 있는데 다른 항목
  created_at       TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  decided_at       TEXT,
  CHECK (student_a < student_b),
  UNIQUE (student_a, student_b)
);

CREATE INDEX ix_sibling_b ON sibling_links(student_b);

-- ---------------------------------------------------------------
-- 확인 필요
-- ---------------------------------------------------------------

CREATE TABLE issues (
  id           INTEGER PRIMARY KEY,
  student_id   INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN (
                 'ADDRESS',            -- 주소 분류를 정하지 못함
                 'BIRTH',              -- 생년월일을 날짜로 읽지 못하거나 범위를 벗어남
                 'GUARDIAN_CONFLICT',  -- 형제인데 보호자 정보가 서로 다름
                 'GUARDIAN_FILL',      -- 형제에게 있는 정보로 빈칸을 채울 수 있음
                 'SIBLING_CANDIDATE',  -- 형제 후보 확인 필요
                 'DUPLICATE',          -- 같은 학년도에 이름+생년월일이 같은 학생이 있음
                 'MISSING',            -- 성별·생년월일·주소·연락처 등이 비어 있음
                 'CLASS_ASSIGN',       -- 반 또는 번호가 정해지지 않음
                 'NUMBER_DUP',         -- 같은 반에 같은 번호가 둘 이상
                 'OTHER')),
  message      TEXT NOT NULL,                           -- 사용자에게 보여줄 문장
  detail       TEXT,                                    -- JSON 근거 (후보 목록, 비교값 등)
  ref_id       INTEGER,                                 -- sibling_links.id 등 관련 행
  status       TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'RESOLVED', 'DISMISSED')),
  created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  resolved_at  TEXT
);

CREATE INDEX ix_issues_student_open ON issues(student_id) WHERE status = 'OPEN';
CREATE INDEX ix_issues_kind_open    ON issues(kind) WHERE status = 'OPEN';
-- 같은 학생·같은 종류·같은 참조의 열린 항목은 하나만
CREATE UNIQUE INDEX ux_issues_open ON issues(student_id, kind, COALESCE(ref_id, 0)) WHERE status = 'OPEN';

-- ---------------------------------------------------------------
-- 작업 기록
-- ---------------------------------------------------------------

-- 가져오기는 '분석(미리보기) → 적용' 두 단계로 동작한다. 이 표에는 적용된 것만 남는다.
-- mode 로 '새로 추가만' / '기존 학생 갱신' / '둘 다' 를 구분하고, mapping 에 열 짝짓기를
-- 남겨 다음 가져오기에서 다시 쓴다.
CREATE TABLE imports (
  id            INTEGER PRIMARY KEY,
  file_name     TEXT NOT NULL,
  sheet_name    TEXT,
  school_year   INTEGER NOT NULL,
  mode          TEXT NOT NULL DEFAULT 'ADD' CHECK (mode IN ('ADD', 'UPDATE', 'BOTH')),
  imported_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  total         INTEGER NOT NULL DEFAULT 0,             -- 파일에서 읽은 행 수
  added         INTEGER NOT NULL DEFAULT 0,             -- 새로 등록한 학생
  updated       INTEGER NOT NULL DEFAULT 0,             -- 기존 학생을 고친 수
  skipped       INTEGER NOT NULL DEFAULT 0,             -- 사용자가 건너뛴 행
  flagged       INTEGER NOT NULL DEFAULT 0,             -- 확인 필요가 생긴 학생
  mapping       TEXT,                                   -- JSON: 엑셀 열 -> 항목 짝짓기
  summary       TEXT                                    -- JSON: 확인 필요 종류별 개수
);

CREATE TABLE year_transitions (
  id           INTEGER PRIMARY KEY,
  from_year    INTEGER NOT NULL,
  to_year      INTEGER NOT NULL,
  executed_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  summary      TEXT                                     -- JSON: 진급/신입/졸업/제외 개수
);

-- ---------------------------------------------------------------
-- 번호 재정렬 기록
--
-- 번호 이동 한 번에 반 학생 열몇 명의 번호가 함께 바뀐다. 나중에 "내 번호가 왜
-- 바뀌었지?" 를 되짚을 수 있어야 하므로 작업 단위로 한 줄만 남긴다.
--
-- 학생마다 enrollment_events 에 남기지 않는 까닭: 그 표는 전입·전출·진급 같은
-- '학적 이동' 이력이다. 한 번 정렬할 때마다 사건이 열몇 개씩 쌓이면 정작 봐야 할
-- 이동 기록이 묻힌다.
-- ---------------------------------------------------------------

CREATE TABLE renumber_ops (
  id           INTEGER PRIMARY KEY,
  school_year  INTEGER NOT NULL,
  grade        INTEGER NOT NULL,
  class_name   TEXT,
  student_id   INTEGER REFERENCES students(id) ON DELETE SET NULL,  -- 사용자가 고른 학생
  from_no      INTEGER,                                 -- 없던 번호를 준 경우 NULL
  to_no        INTEGER NOT NULL,
  kind         TEXT NOT NULL CHECK (kind IN ('ASSIGN', 'REORDER', 'INSERT')),
  moved        INTEGER NOT NULL DEFAULT 0,              -- 번호가 바뀐 학생 수 (대상 포함)
  plan         TEXT NOT NULL DEFAULT '[]',              -- JSON: [{studentId, from, to}]
  created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX ix_renumber_class ON renumber_ops(school_year, grade, class_name, id);
