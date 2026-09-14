/**
 * 학년도 관련 순수 함수.
 *
 * 학년도는 3월에 시작한다. 1~2월은 아직 이전 학년도다.
 */
export function guessSchoolYear(now: Date = new Date()): number {
  const y = now.getFullYear()
  const m = now.getMonth() + 1
  return m <= 2 ? y - 1 : y
}

export function yearLabel(year: number | null | undefined): string {
  return year == null ? '학년도 미설정' : `${year}학년도`
}
