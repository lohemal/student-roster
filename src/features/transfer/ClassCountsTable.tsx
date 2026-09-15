import { Notice } from '@/components/ui'
import type { ClassCount, GradeCounts } from '@/ipc/transfer'
import type { Gender } from '@/ipc/student'
import s from './Transfer.module.css'

interface Props {
  counts: GradeCounts | undefined
  /** 사용자가 고른 반 이름. 고른 반만 '전입 후' 를 함께 보여 준다 */
  picked: string
  /** 전입할 학생의 성별. 미입력이면 '' */
  gender: Gender | ''
}

/**
 * 그 학년의 반별 인원.
 *
 * **프로그램은 반을 고르지 않는다.** 학생 수가 가장 적은 반을 권하지도, 강조하지도
 * 않는다. 실제 학교에서는 인원 말고도 따질 것이 많기 때문이다. 여기서는 지금 상황과,
 * 사용자가 반을 고른 뒤의 예상만 보여 준다.
 */
export function ClassCountsTable({ counts, picked, gender }: Props) {
  if (!counts) return null
  if (counts.classes.length === 0) {
    return (
      <Notice tone="info">
        {counts.grade}학년에 아직 학생이 없습니다. 반 이름을 직접 적어 주세요.
      </Notice>
    )
  }

  const chosen = counts.classes.find((c) => (c.className ?? '') === picked.trim())
  const hasUnknown = counts.classes.some((c) => c.unknown > 0) || counts.total.unknown > 0

  return (
    <div className={s.countsBox}>
      <div className={s.countsHead}>{counts.grade}학년 현재 학생 현황</div>
      <table className={s.counts}>
        <thead>
          <tr>
            <th>반</th>
            <th className={s.num}>남</th>
            <th className={s.num}>여</th>
            {hasUnknown && <th className={s.num}>미입력</th>}
            <th className={s.num}>합계</th>
          </tr>
        </thead>
        <tbody>
          {counts.classes.map((c) => (
            <Row
              key={c.classLabel}
              row={c}
              picked={(c.className ?? '') === picked.trim()}
              hasUnknown={hasUnknown}
            />
          ))}
          <tr className={s.countsTotal}>
            <td>합계</td>
            <td className={s.num}>{counts.total.male}</td>
            <td className={s.num}>{counts.total.female}</td>
            {hasUnknown && <td className={s.num}>{counts.total.unknown}</td>}
            <td className={s.num}>{counts.total.total}</td>
          </tr>
        </tbody>
      </table>

      {chosen && (
        <div className={s.afterBox}>
          <div className={s.afterTitle}>{chosen.classLabel}반</div>
          <div className={s.afterRow}>
            <span className={s.afterWhen}>현재</span>
            <span>
              남 {chosen.male} / 여 {chosen.female}
              {chosen.unknown > 0 && ` / 미입력 ${chosen.unknown}`} / 합계 {chosen.total}
            </span>
          </div>
          <div className={s.afterRow}>
            <span className={s.afterWhen}>전입 후</span>
            <span className={s.afterNext}>
              남 {chosen.male + (gender === 'M' ? 1 : 0)} / 여{' '}
              {chosen.female + (gender === 'F' ? 1 : 0)}
              {(chosen.unknown > 0 || gender === '') &&
                ` / 미입력 ${chosen.unknown + (gender === '' ? 1 : 0)}`}{' '}
              / 합계 {chosen.total + 1}
            </span>
          </div>
        </div>
      )}

      {hasUnknown && (
        <div className={s.countsHint}>
          성별이 비어 있는 학생은 남·여 어느 쪽에도 넣지 않고 따로 셉니다.
        </div>
      )}
    </div>
  )
}

function Row({
  row,
  picked,
  hasUnknown,
}: {
  row: ClassCount
  picked: boolean
  hasUnknown: boolean
}) {
  return (
    <tr className={picked ? s.countsPicked : undefined}>
      <td>{row.className ?? '미정'}</td>
      <td className={s.num}>{row.male}</td>
      <td className={s.num}>{row.female}</td>
      {hasUnknown && <td className={s.num}>{row.unknown || ''}</td>}
      <td className={s.num}>{row.total}</td>
    </tr>
  )
}
