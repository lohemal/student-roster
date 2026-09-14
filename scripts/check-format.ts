/**
 * 화면 쪽 순수 함수 검사.  `npm run check:model`
 *
 * node --experimental-strip-types 로 실행되므로 import 경로에 확장자를 쓴다.
 */
import assert from 'node:assert/strict'
import { guessSchoolYear, yearLabel } from '../src/lib/schoolYear.ts'

assert.equal(guessSchoolYear(new Date(2026, 8, 15)), 2026, '9월은 그해 학년도')
assert.equal(guessSchoolYear(new Date(2027, 0, 20)), 2026, '1월은 이전 학년도')
assert.equal(guessSchoolYear(new Date(2027, 1, 28)), 2026, '2월도 이전 학년도')
assert.equal(guessSchoolYear(new Date(2027, 2, 1)), 2027, '3월 1일부터 새 학년도')

assert.equal(yearLabel(2026), '2026학년도')
assert.equal(yearLabel(null), '학년도 미설정')

console.log('check-format: ok')
