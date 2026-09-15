import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { CalendarRange } from 'lucide-react'

import { Button, ErrorNotice, Field, Input } from '@/components/ui'
import { settingsApi } from '@/ipc/settings'
import { guessSchoolYear } from '@/lib/schoolYear'
import s from './WelcomePage.module.css'

/**
 * 첫 실행 화면. 학교 이름과 현재 학년도만 받는다.
 * 나머지 설정은 모두 기본값으로 시작할 수 있어야 한다.
 */
export function WelcomePage() {
  const qc = useQueryClient()
  const nav = useNavigate()
  const [schoolName, setSchoolName] = useState('')
  const [year, setYear] = useState(String(guessSchoolYear()))

  const start = useMutation({
    mutationFn: async () => {
      if (schoolName.trim()) await settingsApi.saveSchool(schoolName)
      await settingsApi.createYear(Number(year), true)
    },
    onSuccess: async () => {
      await qc.invalidateQueries()
      nav('/students', { replace: true })
    },
  })

  const yearOk = /^\d{4}$/.test(year)

  return (
    <div className={s.wrap}>
      <div className={s.card}>
        <div className={s.mark}>
          <CalendarRange size={26} strokeWidth={2.4} />
        </div>
        <div>
          <h1 className={s.title}>학생명단 관리 시스템</h1>
          <p className={s.desc}>
            학생정보를 한 번만 등록하면 전입·전출·진급·통계·명단 생성을 프로그램이 처리합니다.
            시작하려면 학교 이름과 현재 학년도를 알려 주세요.
          </p>
        </div>

        <ul className={s.principles}>
          <li>자료는 이 컴퓨터 안에만 저장되고 외부로 보내지 않습니다.</li>
          <li>확인이 필요한 자료가 있어도 입력을 막지 않고 나중에 모아서 처리할 수 있습니다.</li>
          <li>학년도가 바뀌어도 이전 자료는 그대로 남습니다.</li>
        </ul>

        <form
          className={s.form}
          onSubmit={(e) => {
            e.preventDefault()
            if (yearOk) start.mutate()
          }}
        >
          <Field label="학교 이름" hint="나중에 설정에서 바꿀 수 있습니다.">
            <Input
              autoFocus
              value={schoolName}
              onChange={(e) => setSchoolName(e.target.value)}
              placeholder="예: ○○초등학교"
              maxLength={50}
            />
          </Field>
          <Field label="현재 학년도" hint="3월에 시작하는 연도입니다. 1~2월은 이전 학년도로 봅니다.">
            <Input
              type="number"
              min={2000}
              max={2100}
              value={year}
              onChange={(e) => setYear(e.target.value)}
            />
          </Field>

          <ErrorNotice error={start.error} />

          <div className={s.foot}>
            <Button type="submit" variant="primary" disabled={!yearOk || start.isPending}>
              시작하기
            </Button>
          </div>
        </form>
      </div>
    </div>
  )
}
