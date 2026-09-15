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
    family = {
      fatherName: pick([...SURNAMES]) + '철수',
      motherName: pick([...SURNAMES]) + '영희',
      fatherPhone: chance(0.85) ? fp : null,
      fatherDigits: chance(0.85) ? fd : null,
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

  const primary = chance(0.7) ? family.motherPhone : family.fatherPhone
  const primaryDigits = chance(0.7) ? family.motherDigits : family.fatherDigits

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

db.close()
