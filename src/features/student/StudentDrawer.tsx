import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { AlertTriangle, Trash2, Users } from 'lucide-react'

import {
  Badge,
  Button,
  Drawer,
  DrawerSpacer,
  Empty,
  ErrorNotice,
  Notice,
  Tabs,
  type TabDef,
} from '@/components/ui'
import { addressApi } from '@/ipc/address'
import { studentApi, type StudentDetail, type StudentInput } from '@/ipc/student'
import { AddressPanel } from './AddressPanel'
import { birthCell, genderLabel, statusBadge } from '@/lib/format'
import { StudentForm, emptyStudent } from './StudentForm'
import s from './StudentDrawer.module.css'

type TabKey = 'basic' | 'history' | 'sibling' | 'issues'

function toInput(d: StudentDetail, schoolYear: number): StudentInput {
  const e = d.enrollment
  return {
    name: d.name,
    gender: d.gender ?? '',
    birthRaw: d.birthRaw ?? '',
    addressRaw: d.addressRaw ?? '',
    fatherName: d.fatherName ?? '',
    motherName: d.motherName ?? '',
    fatherPhone: d.fatherPhone ?? '',
    motherPhone: d.motherPhone ?? '',
    primaryPhone: d.primaryPhone ?? '',
    note: d.note ?? '',
    schoolYear,
    grade: e?.grade ?? 1,
    className: e?.className ?? '',
    classNo: e?.classNo ?? null,
    keepManualAddress: false,
  }
}

interface Props {
  /** null 이면 새 학생 등록 */
  studentId: number | null
  schoolYear: number
  classSuggestions: string[]
  onClose: () => void
  /** 저장·삭제 뒤 명단을 다시 읽게 한다 */
  onSaved: () => void
}

export function StudentDrawer({
  studentId,
  schoolYear,
  classSuggestions,
  onClose,
  onSaved,
}: Props) {
  const qc = useQueryClient()
  const isNew = studentId === null
  const [tab, setTab] = useState<TabKey>('basic')
  const [form, setForm] = useState<StudentInput>(() => emptyStudent(schoolYear))
  const [confirmDelete, setConfirmDelete] = useState(false)

  const detail = useQuery({
    queryKey: ['student', studentId, schoolYear],
    queryFn: () => studentApi.get(studentId!, schoolYear),
    enabled: !isNew,
  })

  // 학생이 바뀌면 입력 내용을 그 학생 것으로 되돌린다
  useEffect(() => {
    setTab('basic')
    setConfirmDelete(false)
    if (isNew) setForm(emptyStudent(schoolYear))
  }, [studentId, isNew, schoolYear])

  useEffect(() => {
    if (detail.data) setForm(toInput(detail.data, schoolYear))
  }, [detail.data, schoolYear])

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['student', studentId] })
    onSaved()
  }

  const save = useMutation({
    mutationFn: async () => {
      if (isNew) return studentApi.create(form)
      return studentApi.update(studentId!, form)
    },
    onSuccess: () => {
      invalidate()
      if (isNew) onClose()
    },
  })

  const remove = useMutation({
    mutationFn: () => studentApi.remove(studentId!),
    onSuccess: () => {
      invalidate()
      onClose()
    },
  })

  const d = detail.data
  const issueCount = d?.issues.length ?? 0

  // 주소 판정 상태 — 주소를 고쳤을 때 직접 지정을 어떻게 할지 묻는 데 쓴다
  const addressStatus = useQuery({
    queryKey: ['address-status', studentId],
    queryFn: () => addressApi.status(studentId!),
    enabled: !isNew,
  })
  const addressChanged = (d?.addressRaw ?? '').trim() !== form.addressRaw.trim()

  const tabs: TabDef<TabKey>[] = useMemo(
    () => [
      { key: 'basic', label: '기본정보' },
      { key: 'history', label: '학적 이력' },
      { key: 'sibling', label: '형제' },
      { key: 'issues', label: '확인 필요', count: issueCount },
    ],
    [issueCount],
  )

  const title = isNew ? '학생 등록' : (d?.name ?? '학생')
  const badge = d?.enrollment ? statusBadge(d.enrollment) : null

  return (
    <Drawer
      title={title}
      subtitle={badge ? <Badge tone={badge.tone}>{badge.label}</Badge> : undefined}
      onClose={onClose}
      tabs={isNew ? undefined : <Tabs tabs={tabs} active={tab} onChange={setTab} />}
      footer={
        <>
          {!isNew && !confirmDelete && (
            <Button
              variant="ghost"
              size="sm"
              icon={Trash2}
              onClick={() => setConfirmDelete(true)}
              title="입력을 잘못했을 때만 사용하세요"
            >
              삭제
            </Button>
          )}
          {confirmDelete && (
            <span className={s.confirm}>
              <AlertTriangle size={15} />
              모든 학년도 기록이 지워집니다. 정말 삭제할까요?
            </span>
          )}
          <DrawerSpacer />
          {confirmDelete ? (
            <>
              <Button variant="outline" onClick={() => setConfirmDelete(false)}>
                취소
              </Button>
              <Button
                variant="danger"
                onClick={() => remove.mutate()}
                disabled={remove.isPending}
              >
                삭제
              </Button>
            </>
          ) : (
            <>
              <Button variant="outline" onClick={onClose}>
                닫기
              </Button>
              <Button
                variant="primary"
                onClick={() => save.mutate()}
                disabled={save.isPending || !form.name.trim() || (!isNew && !d)}
              >
                {isNew ? '등록' : '저장'}
              </Button>
            </>
          )}
        </>
      }
    >
      <ErrorNotice error={save.error ?? remove.error ?? detail.error} />
      {save.isSuccess && !isNew && !save.isPending && (
        <div style={{ marginBottom: 12 }}>
          <Notice tone="success">저장했습니다.</Notice>
        </div>
      )}

      {!isNew && detail.isLoading && <div className={s.loading}>불러오는 중…</div>}

      {(isNew || d) && tab === 'basic' && (
        <>
          <StudentForm value={form} onChange={setForm} classSuggestions={classSuggestions} />

          {/*
            주소를 고쳤는데 분류를 직접 지정해 둔 학생이면 한 번 묻는다.
            예전 분류가 새 주소에도 맞다고 볼 수 없기 때문이다.
          */}
          {d && addressChanged && addressStatus.data?.source === 'MANUAL' && (
            <div className={s.manualAsk}>
              <Notice tone="warn">
                이 학생의 주소 분류는 <b>{addressStatus.data.categoryName}</b>(으)로 직접
                지정되어 있습니다. 주소를 바꾸면 그 분류가 새 주소에도 맞는지 알 수 없습니다.
              </Notice>
              <div className={s.manualChoice}>
                <Button
                  size="sm"
                  variant={form.keepManualAddress ? 'outline' : 'primary'}
                  onClick={() => setForm({ ...form, keepManualAddress: false })}
                >
                  새 주소로 다시 판정
                </Button>
                <Button
                  size="sm"
                  variant={form.keepManualAddress ? 'primary' : 'outline'}
                  onClick={() => setForm({ ...form, keepManualAddress: true })}
                >
                  {addressStatus.data.categoryName} 그대로 두기
                </Button>
              </div>
            </div>
          )}

          {d && (
            <div className={s.addressSlot}>
              <AddressPanel
                studentId={d.id}
                schoolYear={schoolYear}
                currentAddress={form.addressRaw}
                savedAddress={d.addressRaw ?? ''}
                onChanged={invalidate}
              />
            </div>
          )}

          {d && (
            <div className={s.meta}>
              <span>등록 {d.createdAt}</span>
              <span>수정 {d.updatedAt}</span>
            </div>
          )}
        </>
      )}

      {d && tab === 'history' && (
        <div className={s.history}>
          <div>
            <div className={s.blockTitle}>학년도별 학적</div>
            <div className={s.yearList}>
              {d.enrollments.map((e) => {
                const b = statusBadge(e)
                return (
                  <div
                    key={e.schoolYear}
                    className={
                      e.schoolYear === schoolYear ? `${s.yearRow} ${s.yearRowNow}` : s.yearRow
                    }
                  >
                    <span className={s.yearName}>{e.schoolYear}학년도</span>
                    <span className={s.yearWhere}>
                      {e.classLabel}
                      {e.classNo != null ? ` ${e.classNo}번` : ' 번호 미정'}
                    </span>
                    <Badge tone={b.tone}>{b.label}</Badge>
                  </div>
                )
              })}
            </div>
          </div>

          <div>
            <div className={s.blockTitle}>이동 기록</div>
            {d.events.length === 0 ? (
              <div className={s.loading}>기록이 없습니다.</div>
            ) : (
              <div className={s.events}>
                {d.events.map((ev) => (
                  <div key={ev.id} className={s.event}>
                    <span className={s.eventKind}>{ev.kindLabel}</span>
                    <span className={s.eventWhere}>
                      {ev.schoolYear}학년도 {ev.classLabel ?? ''}
                      {ev.classNo != null ? ` ${ev.classNo}번` : ''}
                      {ev.note ? ` · ${ev.note}` : ''}
                    </span>
                    <span className={s.eventWhen}>{ev.eventDate ?? ev.createdAt.slice(0, 10)}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {d && tab === 'sibling' && (
        <Empty
          icon={Users}
          title="본교 형제는 Phase 4에서 연결합니다"
          description="보호자 성명과 연락처가 두 가지 이상 같은 학생을 찾아 형제 후보로 보여 주고, 확인하면 비어 있는 보호자 정보를 채울지 여쭤봅니다. 형제 관계는 학생끼리 연결해 두므로 진급해도 표시가 최신으로 유지됩니다."
        />
      )}

      {d && tab === 'issues' && (
        <>
          {d.issues.length === 0 ? (
            <Empty title="확인할 것이 없습니다" description="이 학생의 자료는 모두 채워져 있습니다." />
          ) : (
            <div className={s.issues}>
              {d.issues.map((i) => (
                <div key={i.id} className={s.issue}>
                  <AlertTriangle size={16} color="var(--warn-text)" style={{ marginTop: 2 }} />
                  <div className={s.issueBody}>
                    <div className={s.issueKind}>{i.kindLabel}</div>
                    <div className={s.issueMsg}>{i.message}</div>
                    {i.detail && <div className={s.issueDetail}>{i.detail}</div>}
                  </div>
                </div>
              ))}
            </div>
          )}
          <div style={{ marginTop: 16 }}>
            <Notice tone="info">
              기본정보 탭에서 값을 고쳐 저장하면 해당 표시는 저절로 사라집니다. 종류별로 모아 보고
              한 번에 처리하는 화면은 Phase 5에서 만듭니다.
            </Notice>
          </div>
        </>
      )}

      {d && tab === 'basic' && d.birthDate === null && d.birthRaw && (
        <div style={{ marginTop: 12 }}>
          <Notice tone="warn">
            생년월일을 날짜로 읽지 못해 원본({birthCell(null, d.birthRaw)})만 보관하고 있습니다.
            {d.gender ? ` 성별은 ${genderLabel(d.gender)}입니다.` : ''}
          </Notice>
        </div>
      )}
    </Drawer>
  )
}
