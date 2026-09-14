-- 학생명단 관리 시스템 — 초기 스키마 (설계안 §3)
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

CREATE TABLE address_rules (
  id          INTEGER PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('ROAD', 'COMPLEX', 'CONTAINS')),
  pattern     TEXT NOT NULL,                            -- ROAD: '한누리대로 123' / COMPLEX: '가온마을5단지' / CONTAINS: 임의 문자열
  category_id INTEGER NOT NULL REFERENCES address_categories(id) ON DELETE CASCADE,
  source      TEXT NOT NULL DEFAULT 'USER' CHECK (source IN ('USER', 'BUILTIN')),
  note        TEXT,
  hit_count   INTEGER NOT NULL DEFAULT 0,
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
  address_category_id   INTEGER REFERENCES address_categories(id) ON DELETE SET NULL,
  address_source        TEXT NOT NULL DEFAULT 'NONE' CHECK (address_source IN ('NONE', 'AUTO', 'RULE', 'MANUAL')),
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

-- ---------------------------------------------------------------
-- 학년도별 학적
-- ---------------------------------------------------------------

CREATE TABLE enrollments (
  id                 INTEGER PRIMARY KEY,
  student_id         INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  school_year        INTEGER NOT NULL REFERENCES school_years(year),
  grade              INTEGER NOT NULL CHECK (grade BETWEEN 1 AND 6),
  class_name         TEXT,                              -- '1', '가람' … NULL = 반배정 미정
  class_no           INTEGER CHECK (class_no IS NULL OR class_no >= 1),
  status             TEXT NOT NULL DEFAULT 'ENROLLED' CHECK (status IN ('ENROLLED', 'TRANSFER_IN', 'TRANSFER_OUT')),
  transfer_in_date   TEXT,
  transfer_out_date  TEXT,
  transfer_out_note  TEXT,
  created_at         TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  updated_at         TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  UNIQUE (student_id, school_year)
);

CREATE INDEX ix_enroll_year_class ON enrollments(school_year, grade, class_name, class_no);
CREATE INDEX ix_enroll_status     ON enrollments(school_year, status);

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
  matched_fields   TEXT NOT NULL DEFAULT '[]',          -- JSON: ["mother_name","mother_phone"]
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
                 'ADDRESS', 'BIRTH', 'GUARDIAN_CONFLICT', 'GUARDIAN_FILL',
                 'SIBLING_CANDIDATE', 'DUPLICATE', 'MISSING', 'CLASS_ASSIGN', 'OTHER')),
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

CREATE TABLE imports (
  id           INTEGER PRIMARY KEY,
  file_name    TEXT NOT NULL,
  school_year  INTEGER NOT NULL,
  imported_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  total        INTEGER NOT NULL DEFAULT 0,
  ok_count     INTEGER NOT NULL DEFAULT 0,
  flagged      INTEGER NOT NULL DEFAULT 0,
  summary      TEXT                                     -- JSON: 종류별 개수
);

CREATE TABLE year_transitions (
  id           INTEGER PRIMARY KEY,
  from_year    INTEGER NOT NULL,
  to_year      INTEGER NOT NULL,
  executed_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
  summary      TEXT                                     -- JSON: 진급/신입/졸업/제외 개수
);
