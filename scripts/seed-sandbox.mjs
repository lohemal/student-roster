/**
 * 연습용 자료를 만든다 — 실제 학교 규모로 화면을 시험하기 위한 것.
 *
 *   npm run seed            # 연습용(sandbox) 자료에 1,000명
 *   npm run seed -- 300     # 인원 수를 정해서
 *
 * **연습용 폴더에만** 쓴다. 실제 자료(kr.school.studentroster)는 건드리지 않는다.
 * 자료 모양을 일부러 지저분하게 만든다 — 생년월일 오타, 빈 연락처, 이상한 날짜 등이
 * 섞여 있어야 '확인 필요' 흐름을 제대로 볼 수 있다.
 */
import { DatabaseSync } from 'node:sqlite'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

const args = process.argv.slice(2).filter((a) => a !== '--reset')
const RESET = process.argv.includes('--reset')
const COUNT = Number(args[0] ?? 1000)
const YEAR = Number(args[1] ?? new Date().getFullYear())

const dbPath = join(
  process.env.APPDATA ?? '',
  'kr.school.studentroster.sandbox',
  'studentroster.db',
)

if (!existsSync(dbPath)) {
  console.error(`연습용 자료가 없습니다: ${dbPath}`)
  console.error('먼저 `npm run app:sandbox` 로 한 번 켜서 학년도를 만들어 주세요.')
  process.exit(1)
}

// ---- 이름 만들기 ----------------------------------------------------------

const SURNAMES = '김이박최정강조윤장임한오서신권황안송류전홍고문양손배백허유남심노하곽성차주우구'
const GIVEN_1 = '민서지예수윤하은도서준하시우주원지호준우건우현우우진선우연준유준정우승현시윤준서'
const GIVEN_2 = '준서윤우진호연아린서아하윤지아채원수아지우다은예린소율지윤예은수빈서연민지'

const CLASSES = {
  1: ['가람', '나리', '다솜'],
  2: ['가람', '나리', '다솜'],
  3: ['가람', '나리', '다솜', '라온'],
  4: ['1', '2', '3'],
  5: ['1', '2', '3'],
  6: ['가람', '나리', '다솜'],
}

/**
 * 가상 주소. 주소 분류 시험을 위해 네 갈래를 섞는다.
 *
 *   complex  주소에 단지 이름이 적혀 있다 → 분류만 만들면 저절로 분류된다
 *   roadOnly 단지 이름이 없다 → 도로명 규칙을 만들어야 분류된다
 *   villa    빌라 이름만 있다 → 포함 규칙으로 '주택' 에 넣을 수 있다
 *   plain    아무 단서가 없다 → 미분류로 남는다
 */
const PLACES = [
  { kind: 'complex', road: '가온로', no: 101, complex: '가온마을5단지', weight: 22 },
  { kind: 'complex', road: '나온로', no: 202, complex: '나온마을1단지', weight: 18 },
  { kind: 'complex', road: '다온로', no: 303, complex: '다온마을2단지', weight: 15 },
  // 단지 이름이 안 적힌 같은 아파트 — 도로명 규칙이 필요하다
  { kind: 'roadOnly', road: '라온로', no: 404, complex: null, weight: 16 },
  { kind: 'roadOnly', road: '마온로', no: 505, complex: null, weight: 12 },
  { kind: 'villa', road: '바온로', no: 606, complex: null, weight: 10 },
  { kind: 'plain', road: '사온로', no: 707, complex: null, weight: 7 },
]

/** 가중치에 맞춰 하나 고른다 */
function pickPlace() {
  const total = PLACES.reduce((n, p) => n + p.weight, 0)
  let r = rnd() * total
  for (const p of PLACES) {
    r -= p.weight
    if (r <= 0) return p
  }
  return PLACES[PLACES.length - 1]
}

let seed = 20260915
/**
 * 돌릴 때마다 같은 자료가 나오도록 고정 난수를 쓴다 (mulberry32).
 *
 * 곱셈에 `Math.imul` 을 쓰는 이유: 자바스크립트 수는 2^53 까지만 정확해서
 * 그냥 곱하면 아래 자리가 뭉개지고 같은 값이 되풀이된다.
 */
function rnd() {
  seed = (seed + 0x6d2b79f5) | 0
  let t = seed
  t = Math.imul(t ^ (t >>> 15), t | 1)
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296
}
const pick = (arr) => arr[Math.floor(rnd() * arr.length)]
const chance = (p) => rnd() < p

function makeName(gender) {
  const given = gender === 'M' ? GIVEN_1 : GIVEN_2
  const a = given[Math.floor(rnd() * given.length)]
  const b = given[Math.floor(rnd() * given.length)]
  return pick([...SURNAMES]) + a + b
}

function phone() {
  const mid = String(Math.floor(1000 + rnd() * 9000))
  const last = String(Math.floor(1000 + rnd() * 9000))
  return [`010-${mid}-${last}`, `010${mid}${last}`]
}

/** 학년에 맞는 생년월일. 가끔 일부러 잘못된 값을 섞는다. */
function birth(grade) {
  const year = YEAR - grade - 6
  const m = 1 + Math.floor(rnd() * 12)
  const d = 1 + Math.floor(rnd() * 28)
  const pad = (n) => String(n).padStart(2, '0')

  if (chance(0.015)) return [`${year}${pad(m)}32`, null] // 달력에 없는 날
  if (chance(0.01)) return ['확인요망', null] // 날짜가 아닌 값

  const iso = `${year}-${pad(m)}-${pad(d)}`
  // 입력 모양을 일부러 섞는다
  const raw = pick([
    `${String(year).slice(2)}${pad(m)}${pad(d)}`,
    `${String(year).slice(2)}.${pad(m)}.${pad(d)}`,
    `${String(year).slice(2)}.${pad(m)}.${pad(d)}.`,
    `${year}${pad(m)}${pad(d)}`,
    `${year}.${pad(m)}.${pad(d)}`,
  ])
  return [raw, iso]
}

// ---- 넣기 -----------------------------------------------------------------

const db = new DatabaseSync(dbPath)
db.exec('PRAGMA foreign_keys = ON')

const hasYear = db
  .prepare('SELECT COUNT(*) AS n FROM school_years WHERE year = ?')
  .get(YEAR)
if (hasYear.n === 0) {
  db.prepare('INSERT INTO school_years(year) VALUES (?)').run(YEAR)
  db.prepare('UPDATE school_years SET is_current = 0').run()
  db.prepare('UPDATE school_years SET is_current = 1 WHERE year = ?').run(YEAR)
  console.log(`${YEAR}학년도를 만들었습니다.`)
}

if (RESET) {
  // students 를 지우면 학적·이력·확인 필요도 ON DELETE CASCADE 로 함께 사라진다
  const gone = db.prepare('SELECT COUNT(*) AS n FROM students').get().n
  db.prepare('DELETE FROM students').run()
  console.log(`기존 연습용 학생 ${gone}명을 지웠습니다.`)
}

const before = db.prepare('SELECT COUNT(*) AS n FROM students').get().n

const insStudent = db.prepare(`
  INSERT INTO students(
    name, gender, birth_raw, birth_date, address_raw, address_norm,
    father_name, mother_name,
    father_phone, father_phone_digits, mother_phone, mother_phone_digits,
    primary_phone, primary_phone_digits, note)
  VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)`)

const insEnroll = db.prepare(`
  INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
  VALUES (?,?,?,?,?,?)`)

const insEvent = db.prepare(`
  INSERT INTO enrollment_events(student_id, school_year, kind, grade, class_name, class_no, source)
  VALUES (?,?,?,?,?,?,'IMPORT')`)

// 반마다 번호를 1번부터 매기기 위한 세기
const counters = new Map()

db.exec('BEGIN')
let made = 0
/** 형제로 묶을 보호자 정보를 몇 집 만들어 둔다 */
const families = []

for (let i = 0; i < COUNT; i++) {
  const grade = 1 + Math.floor(rnd() * 6)
  const className = pick(CLASSES[grade])
  const key = `${grade}-${className}`
  const no = (counters.get(key) ?? 0) + 1
  counters.set(key, no)

  const gender = chance(0.5) ? 'M' : 'F'
  const name = makeName(gender)
  const [birthRaw, birthIso] = birth(grade)

  // 형제로 묶이도록 앞서 만든 집의 보호자를 다시 쓴다
  let family
  if (families.length > 0 && chance(0.12)) {
    family = pick(families)
  } else {
    const [fp, fd] = phone()
    const [mp, md] = phone()
    // 보호자 이름도 학생만큼 다양해야 한다. 이름이 몇 가지뿐이면 남남인 학생끼리
    // 부·모 성명이 우연히 겹쳐 형제 후보가 실제보다 훨씬 많이 나온다.
    const hasFatherPhone = chance(0.85)
    family = {
      fatherName: makeName('M'),
      motherName: makeName('F'),
      fatherPhone: hasFatherPhone ? fp : null,
      fatherDigits: hasFatherPhone ? fd : null,
      motherPhone: mp,
      motherDigits: md,
    }
    families.push(family)
  }

  const place = pickPlace()
  const dong = 101 + Math.floor(rnd() * 12)
  const ho = `${1 + Math.floor(rnd() * 15)}0${1 + Math.floor(rnd() * 4)}`
  let address
  switch (place.kind) {
    case 'complex':
      // 띄어쓰기를 일부러 들쭉날쭉하게 — 같은 단지로 읽혀야 한다
      address = chance(0.5)
        ? `○○시 ${place.road} ${place.no}, ${dong}동 ${ho}호(${place.complex})`
        : `○○시 ${place.road} ${place.no} ${dong}동 ${ho}호 (${place.complex.replace('마을', '마을 ')})`
      break
    case 'roadOnly':
      address = `○○시 ${place.road} ${place.no}, ${dong}동 ${ho}호`
      break
    case 'villa':
      address = `○○시 ${place.road} ${place.no} 가온빌라 ${ho}호`
      break
    default:
      address = `○○시 ${place.road} ${place.no}-${1 + Math.floor(rnd() * 20)}`
  }

  // 일부러 비워 두는 칸들 — '확인 필요'가 생기도록
  const noGender = chance(0.02)
  const noAddress = chance(0.02)
  const noPhone = chance(0.02)

  // 한 번만 뽑는다. 따로 뽑으면 보이는 번호와 찾기용 숫자가 서로 다른 사람 것이 된다.
  const primaryIsMother = chance(0.7)
  const primary = primaryIsMother ? family.motherPhone : family.fatherPhone
  const primaryDigits = primaryIsMother ? family.motherDigits : family.fatherDigits

  insStudent.run(
    name,
    noGender ? null : gender,
    birthRaw,
    birthIso,
    noAddress ? null : address,
    noAddress ? null : address,
    family.fatherName,
    family.motherName,
    noPhone ? null : family.fatherPhone,
    noPhone ? null : family.fatherDigits,
    noPhone ? null : family.motherPhone,
    noPhone ? null : family.motherDigits,
    noPhone ? null : primary,
    noPhone ? null : primaryDigits,
    chance(0.05) ? '알레르기 있음' : null,
  )
  const id = db.prepare('SELECT last_insert_rowid() AS id').get().id

  // 몇 명은 반·번호를 일부러 비운다
  const unassigned = chance(0.01)
  insEnroll.run(
    id,
    YEAR,
    grade,
    unassigned ? null : className,
    unassigned ? null : no,
    chance(0.03) ? 'TRANSFER_IN' : 'ENROLLED',
  )
  insEvent.run(id, YEAR, 'ENROLL', grade, unassigned ? null : className, unassigned ? null : no)
  made++
}
db.exec('COMMIT')

// 찾기 시험용으로 번호를 딱 정해 둔 학생 하나
db.exec(`
  UPDATE students
     SET father_phone = '010-1234-5678', father_phone_digits = '01012345678',
         primary_phone = '010-1234-5678', primary_phone_digits = '01012345678'
   WHERE id = (SELECT MIN(id) FROM students)`)

// ---- 형제 탐색 시험용 -----------------------------------------------------
//
// 형제 판정의 경계를 눈으로 확인하려고 일부러 짜 둔 학생들.
// 이름·연락처·주소 모두 가상의 값이다. 위 무작위 자료와 겹치지 않도록
// 연락처 끝자리를 0 으로 시작하게 두었다 — 무작위로는 나올 수 없는 모양이다.

const digitsOf = (v) => (v ? v.replace(/\D/g, '') : null)

/** 보호자 네 항목. 빈칸은 null 로 둔다. */
const G = (fatherName, motherName, fatherPhone, motherPhone) => ({
  fatherName,
  motherName,
  fatherPhone,
  motherPhone,
})

function addFixture(name, gender, grade, g, note) {
  const className = pick(CLASSES[grade])
  const key = `${grade}-${className}`
  const no = (counters.get(key) ?? 0) + 1
  counters.set(key, no)

  const [birthRaw, birthIso] = birth(grade)
  const address = `○○시 가온로 101, 105동 ${100 + (no % 15)}호(가온마을5단지)`
  const primary = g.motherPhone ?? g.fatherPhone

  insStudent.run(
    name,
    gender,
    birthRaw,
    birthIso,
    address,
    address,
    g.fatherName,
    g.motherName,
    g.fatherPhone,
    digitsOf(g.fatherPhone),
    g.motherPhone,
    digitsOf(g.motherPhone),
    primary,
    digitsOf(primary),
    note,
  )
  const id = db.prepare('SELECT last_insert_rowid() AS id').get().id
  insEnroll.run(id, YEAR, grade, className, no, 'ENROLLED')
  insEvent.run(id, YEAR, 'ENROLL', grade, className, no)
  made++
  return id
}

const insLink = db.prepare(`
  INSERT INTO sibling_links(
    student_a, student_b, status, source, matched_fields, conflict_fields, decided_at)
  VALUES (?,?,?,'AUTO',?,'[]',datetime('now','localtime'))`)

/** 사용자가 이미 판단해 둔 관계를 미리 만들어 둔다 */
function link(a, b, status, matched) {
  insLink.run(Math.min(a, b), Math.max(a, b), status, JSON.stringify(matched))
}

db.exec('BEGIN')

// A — 네 항목 모두 같은 확실한 형제 2명
const gA = G('강바다', '임소라', '010-9901-0001', '010-9901-0002')
addFixture('강한별', 'F', 3, gA, '형제 시험 A — 네 항목 모두 같음')
addFixture('강두별', 'M', 5, gA, '형제 시험 A — 네 항목 모두 같음')

// B — 형제 3명. 쌍으로 세면 3쌍이 나와야 한다.
//     가운데 학생은 부 연락처가 비어 있어 가져오기 시험에도 쓴다.
const gB = G('문산들', '배가온', '010-9902-0001', '010-9902-0002')
addFixture('문한별', 'M', 1, gB, '형제 시험 B — 형제 3명(3쌍)')
addFixture('문두별', 'F', 4, G(gB.fatherName, gB.motherName, null, gB.motherPhone),
  '형제 시험 B — 형제 3명, 부 연락처 빈칸')
addFixture('문세별', 'M', 6, gB, '형제 시험 B — 형제 3명(3쌍)')

// C — 부 성명 하나만 같다. 한 항목만으로는 후보가 되면 안 된다.
addFixture('오한별', 'M', 2, G('노가람', '서미르', '010-9903-0001', '010-9903-0002'),
  '형제 시험 C — 한 항목만 같음(후보가 되면 안 됨)')
addFixture('정두별', 'F', 5, G('노가람', '채봄이', '010-9903-0003', '010-9903-0004'),
  '형제 시험 C — 한 항목만 같음(후보가 되면 안 됨)')

// D — 모 연락처 하나만 같다. 역시 후보가 되면 안 된다.
addFixture('한한별', 'F', 1, G('우다온', '신여울', '010-9904-0001', '010-9904-0009'),
  '형제 시험 D — 연락처 하나만 같음(후보가 되면 안 됨)')
addFixture('심두별', 'M', 3, G('곽나루', '차슬기', '010-9904-0002', '010-9904-0009'),
  '형제 시험 D — 연락처 하나만 같음(후보가 되면 안 됨)')

// E — 값이 있는 항목이 딱 2개고 둘 다 같다. 빈칸은 세지 않는다.
const gE = G('백벼리', null, '010-9905-0001', null)
addFixture('백한별', 'M', 2, gE, '형제 시험 E — 정확히 2개 일치(빈칸은 세지 않음)')
addFixture('백두별', 'F', 6, gE, '형제 시험 E — 정확히 2개 일치(빈칸은 세지 않음)')

// F — 3개가 같고 모 연락처만 다르다. 후보이면서 불일치가 함께 보여야 한다.
addFixture('구한별', 'F', 1, G('구나래', '유하람', '010-9906-0001', '010-9906-0002'),
  '형제 시험 F — 3개 일치 1개 불일치')
addFixture('구두별', 'M', 5, G('구나래', '유하람', '010-9906-0001', '010-9906-0008'),
  '형제 시험 F — 3개 일치 1개 불일치')

// G — 보호자 정보가 하나도 없다. 빈칸끼리 묶이면 온 학교가 형제가 된다.
const gNone = G(null, null, null, null)
addFixture('남한별', 'M', 4, gNone, '형제 시험 G — 보호자 정보 없음(후보가 되면 안 됨)')
addFixture('하두별', 'F', 2, gNone, '형제 시험 G — 보호자 정보 없음(후보가 되면 안 됨)')

// H — 네 항목이 같지만 사용자가 '형제 아님'으로 정해 둔 쌍.
//     다시 찾아도 후보로 되살아나면 안 된다.
const gH = G('전미르', '손보라', '010-9907-0001', '010-9907-0002')
const h1 = addFixture('전한별', 'F', 3, gH, '형제 시험 H — 형제 아님으로 정해 둠')
const h2 = addFixture('전두별', 'M', 6, gH, '형제 시험 H — 형제 아님으로 정해 둠')
link(h1, h2, 'REJECTED', ['fatherName', 'motherName', 'fatherPhone', 'motherPhone'])

// I — 이미 확정한 형제. 한쪽에 빈칸이 있어 [가져오기]를 눌러 볼 수 있다.
const i1 = addFixture('권한별', 'M', 2, G('권여울', '황초롱', '010-9908-0001', '010-9908-0002'),
  '형제 시험 I — 확정 + 가져올 보호자 정보 있음')
const i2 = addFixture('권두별', 'F', 4, G('권여울', null, '010-9908-0001', null),
  '형제 시험 I — 확정, 모 성명·모 연락처가 빈칸')
link(i1, i2, 'CONFIRMED', ['fatherName', 'fatherPhone'])

// ---- 번호 재정렬 시험용 -----------------------------------------------------
//
// 번호를 옮기면 같은 반 학생이 어떻게 밀리는지 눈으로 보려면 번호가 가지런한 반이
// 하나 있어야 한다. 6학년 라온반에 1~23번을 차례로 채워 둔다.
// 자료는 모두 채워 넣어 '확인 필요' 표시가 생기지 않게 한다 — 번호만 보기 위해서다.

/** 번호를 직접 정해 학생을 넣는다. */
function addNumbered(name, grade, className, classNo, note, gender = 'M') {
  const [fp, fd] = phone()
  const [mp, md] = phone()
  const address = `○○시 나온로 202, 101동 ${100 + ((classNo ?? 0) % 15)}호(나온마을1단지)`
  // 번호만 보기 위한 학생이므로 생년월일은 일부러 깨끗하게 둔다
  const year = YEAR - grade - 6
  const day = String(1 + ((classNo ?? 0) % 28)).padStart(2, '0')
  const birthIso = `${year}-03-${day}`
  const birthRaw = `${String(year).slice(2)}03${day}`
  insStudent.run(
    name,
    gender,
    birthRaw,
    birthIso,
    address,
    address,
    makeName('M'),
    makeName('F'),
    fp,
    fd,
    mp,
    md,
    mp,
    md,
    note,
  )
  const id = db.prepare('SELECT last_insert_rowid() AS id').get().id
  insEnroll.run(id, YEAR, grade, className, classNo, 'ENROLLED')
  insEvent.run(id, YEAR, 'ENROLL', grade, className, classNo)
  made++
  return id
}

// 번호가 가지런한 반 — 23번을 7번으로, 7번을 23번으로 옮겨 보는 곳
for (let i = 1; i <= 23; i++) {
  addNumbered(
    `번호시험${String(i).padStart(2, '0')}`,
    6,
    '라온',
    i,
    '번호 시험 — 1~23번이 가지런한 반',
    i % 2 === 0 ? 'F' : 'M',
  )
}

// 번호가 겹치는 반 — 여기서는 자동 번호 변경이 막혀야 한다
addNumbered('중복시험가', 2, '라온', 1, '번호 중복 시험 — 1번')
addNumbered('중복시험나', 2, '라온', 2, '번호 중복 시험 — 2번')
addNumbered('중복시험다', 2, '라온', 5, '번호 중복 시험 — 5번이 둘 가운데 하나', 'F')
addNumbered('중복시험라', 2, '라온', 5, '번호 중복 시험 — 5번이 둘 가운데 하나', 'F')
addNumbered('중복시험마', 2, '라온', 6, '번호 중복 시험 — 6번')

// 번호가 띄엄띄엄한 반 — 1,2,3,5,6,9. 옮겨도 4·7·8번이 생기면 안 된다
for (const [i, no] of [1, 2, 3, 5, 6, 9].entries()) {
  addNumbered(
    `띄엄시험${String(i + 1).padStart(2, '0')}`,
    4,
    '라온',
    no,
    '번호 시험 — 띄엄띄엄한 번호(1,2,3,5,6,9)',
    'F',
  )
}

// 번호가 없는 학생 — 자동 이동 대상이 아니다
addNumbered('번호없음가', 4, '라온', null, '번호 시험 — 번호가 없는 학생')

db.exec('COMMIT')

const after = db.prepare('SELECT COUNT(*) AS n FROM students').get().n
const enrolled = db
  .prepare(
    `SELECT COUNT(*) AS n FROM enrollments WHERE school_year = ? AND status IN ('ENROLLED','TRANSFER_IN')`,
  )
  .get(YEAR).n

console.log(`연습용 자료에 ${made}명을 넣었습니다. (${before} → ${after})`)
console.log(`${YEAR}학년도 재학생 ${enrolled}명`)
console.log(`자료 파일: ${dbPath}`)
console.log('')
console.log('확인 필요 표시는 앱에서 학생을 한 번 저장하면 계산됩니다.')
console.log('')
console.log('주소 분류 시험:')
console.log("  · '가온마을5단지' '나온마을1단지' '다온마을2단지' 분류를 만들면 저절로 분류됩니다")
console.log("  · '라온로 404' '마온로 505' 는 도로명 규칙이 있어야 분류됩니다")
console.log("  · '가온빌라' 는 포함 규칙으로 '주택' 에 넣을 수 있습니다")
console.log("  · '사온로' 는 단서가 없어 미분류로 남습니다")
console.log('연락처 검색 시험: 1234 / 5678 (첫 번째 학생)')
console.log('')
console.log('형제 탐색 시험 (이름이 ○한별·○두별·○세별 인 학생 19명):')
console.log('  · A 네 항목 모두 같음 · B 형제 3명(3쌍) · E 정확히 2개 일치 → 후보로 올라와야 합니다')
console.log('  · C 한 항목만 같음 · D 연락처만 같음 · G 보호자 정보 없음 → 후보가 되면 안 됩니다')
console.log('  · F 3개 일치 1개 불일치 → 후보이면서 불일치가 함께 보여야 합니다')
console.log("  · H 는 '형제 아님', I 는 '확정'으로 미리 정해 두었습니다 (I 는 가져오기 시험용)")
console.log("  · 설정 화면의 [형제 후보 다시 찾기] 를 눌러야 후보가 만들어집니다")
console.log('')
console.log('번호 재정렬 시험:')
console.log('  · 6학년 라온반 — 1~23번이 가지런합니다. 23→7 과 7→23 을 눌러 보세요')
console.log('  · 4학년 라온반 — 번호가 1,2,3,5,6,9 로 띄엄띄엄합니다 (없던 4·7·8번이 생기면 안 됩니다)')
console.log('  · 4학년 라온반 번호없음가 — 번호가 없는 학생입니다')
console.log('  · 2학년 라온반 — 5번이 둘입니다. 자동 번호 변경이 막혀야 합니다')

db.close()
