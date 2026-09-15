/**
 * 화면 쪽 순수 함수 검사.  `npm run check:model`
 *
 * node --experimental-strip-types 로 실행되므로 import 경로에 확장자를 쓴다.
 * 저장 규칙의 원본은 Rust다 — 여기서는 '보여 주기'만 검사한다.
 */
import assert from 'node:assert/strict'
import { guessSchoolYear, yearLabel } from '../src/lib/schoolYear.ts'
import {
  birthCell,
  birthDisplay,
  countLabel,
  digitsOnly,
  genderLabel,
  issueBadge,
  rangeLabel,
  statusBadge,
} from '../src/lib/format.ts'

// ---------- 학년도 ----------

assert.equal(guessSchoolYear(new Date(2026, 8, 15)), 2026, '9월은 그해 학년도')
assert.equal(guessSchoolYear(new Date(2027, 0, 20)), 2026, '1월은 이전 학년도')
assert.equal(guessSchoolYear(new Date(2027, 1, 28)), 2026, '2월도 이전 학년도')
assert.equal(guessSchoolYear(new Date(2027, 2, 1)), 2027, '3월 1일부터 새 학년도')

assert.equal(yearLabel(2026), '2026학년도')
assert.equal(yearLabel(null), '학년도 미설정')

// ---------- 생년월일 표시 ----------

assert.equal(birthDisplay('2017-03-15'), '17.03.15.', '요구사항의 표시 형식')
assert.equal(birthDisplay('2009-12-31'), '09.12.31.')
assert.equal(birthDisplay('2000-01-01'), '00.01.01.')
assert.equal(birthDisplay(null), '')
assert.equal(birthDisplay(''), '')
assert.equal(birthDisplay('이상한값'), '이상한값', '못 읽으면 그대로 보여 준다')

// 날짜로 읽지 못한 학생은 원본을 보여 줘야 무엇을 고칠지 안다
assert.equal(birthCell('2017-03-15', '170315'), '17.03.15.')
assert.equal(birthCell(null, '20170230'), '20170230')
assert.equal(birthCell(null, null), '')

// ---------- 성별 ----------

assert.equal(genderLabel('M'), '남')
assert.equal(genderLabel('F'), '여')
assert.equal(genderLabel(null), '')

// ---------- 숫자만 ----------

assert.equal(digitsOnly('010-1234-5678'), '01012345678')
assert.equal(digitsOnly('가나다'), '')

// ---------- 상태 배지 ----------

assert.equal(statusBadge({ status: 'ENROLLED', graduated: false }).label, '재학')
assert.equal(statusBadge({ status: 'TRANSFER_IN', graduated: false }).label, '전입')
assert.equal(statusBadge({ status: 'TRANSFER_OUT', graduated: false }).label, '전출')
// 졸업은 학적 상태가 아니라 따로 저장한 사건이다. 표시할 때 합친다.
assert.equal(
  statusBadge({ status: 'ENROLLED', graduated: true }).label,
  '졸업',
  '그 해에 졸업했으면 졸업으로 보여 준다',
)
assert.equal(statusBadge({ status: 'ENROLLED', graduated: false }).tone, 'success')
assert.equal(statusBadge({ status: 'TRANSFER_OUT', graduated: false }).tone, 'error')

// ---------- 확인 필요 배지 ----------

assert.equal(issueBadge({ issueCount: 0 }), null, '없으면 아무것도 보여 주지 않는다')
assert.equal(issueBadge({ issueCount: 3 })?.label, '확인 3')
assert.equal(issueBadge({ issueCount: 3 })?.tone, 'warn')

// ---------- 개수 · 쪽 안내 ----------

assert.equal(countLabel(1011), '1,011명')
assert.equal(rangeLabel(0, 0, 0), '0명')
assert.equal(rangeLabel(1011, 0, 100), '1,011명 중 1–100')
assert.equal(rangeLabel(1011, 1000, 11), '1,011명 중 1,001–1,011')

console.log('check-format: ok')
