import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { Search, UserPlus } from 'lucide-react'

import { Badge, Button, ErrorNotice, Field, Input, Notice } from '@/components/ui'
import { transferApi, type StudentMatch } from '@/ipc/transfer'
import { birthDisplay } from '@/lib/format'
import s from './Transfer.module.css'

interface Props {
  schoolYear: number
  /** 기존 학생을 골랐을 때 */
  onPick: (m: StudentMatch) => void
  /** 처음 오는 학생일 때 */
  onNew: (name: string, birth: string) => void
}

/**
 * 기존 학생 찾기.
 *
 * **이름이 같다고 프로그램이 같은 학생이라고 정하지 않는다.** 지난 학적을 나란히
 * 보여 주고 고르는 것은 사람이 한다. 같은 사람을 두 명으로 만들지 않기 위한 단계다.
 */
export function StudentLookup({ schoolYear, onPick, onNew }: Props) {
  const [name, setName] = useState('')
  const [birth, setBirth] = useState('')
  const [tried, setTried] = useState(false)

  const search = useMutation({
    mutationFn: () => transferApi.search(name.trim(), birth.trim() || null, schoolYear),
    onSuccess: () => setTried(true),
  })

  const found = search.data ?? []
  const canSearch = name.trim().length >= 2

  return (
    <div className={s.lookup}>
      <Notice tone="info">
        먼저 이미 등록된 학생인지 찾아 주세요. 지난 학년도에 다녔거나 올해 전출했던
        학생이면 <b>그 학생을 그대로</b> 써야 기록이 이어집니다.
      </Notice>

      <div className={s.lookupRow}>
        <Field label="이름">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="두 글자 이상"
            onKeyDown={(e) => {
              if (e.key === 'Enter' && canSearch) search.mutate()
            }}
          />
        </Field>
        <Field label="생년월일" hint="알면 더 정확합니다 (2017-03-15)">
          <Input
            value={birth}
            onChange={(e) => setBirth(e.target.value)}
            placeholder="2017-03-15"
          />
        </Field>
        <Button
          variant="outline"
          icon={Search}
          onClick={() => search.mutate()}
          disabled={!canSearch || search.isPending}
        >
          찾기
        </Button>
      </div>

      <ErrorNotice error={search.error} />

      {tried && found.length === 0 && !search.isPending && (
        <Notice tone="info">
          같은 이름의 학생이 없습니다. 아래 [처음 오는 학생으로 등록]을 눌러 주세요.
        </Notice>
      )}

      {found.length > 0 && (
        <div className={s.matches}>
          <div className={s.matchesHead}>찾은 학생 {found.length}명</div>
          {found.map((m) => (
            <button
              key={m.studentId}
              type="button"
              className={s.match}
              onClick={() => onPick(m)}
            >
              <div className={s.matchHead}>
                <span className={s.matchName}>{m.name}</span>
                <span className={s.matchBirth}>
                  {birthDisplay(m.birthDate) || m.birthRaw || '생년월일 없음'}
                </span>
                {m.currentStatus && <Badge tone="info">올해 {m.currentStatus}</Badge>}
              </div>
              {m.history.length === 0 ? (
                <div className={s.matchLine}>학적 기록이 없습니다.</div>
              ) : (
                m.history.map((h) => (
                  <div key={h.schoolYear} className={s.matchLine}>
                    <span className={s.matchYear}>{h.schoolYear}학년도</span>
                    <span>{h.whereAt}</span>
                    {h.extra && <span className={s.matchExtra}>{h.extra}</span>}
                  </div>
                ))
              )}
            </button>
          ))}
          <div className={s.countsHint}>
            같은 학생이 맞는지 지난 학적을 보고 정해 주세요. 이름이 같아도 다른 학생일 수
            있습니다.
          </div>
        </div>
      )}

      <div className={s.lookupFoot}>
        <Button
          variant="primary"
          icon={UserPlus}
          onClick={() => onNew(name.trim(), birth.trim())}
        >
          처음 오는 학생으로 등록
        </Button>
      </div>
    </div>
  )
}
