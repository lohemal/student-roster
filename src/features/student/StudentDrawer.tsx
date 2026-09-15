import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { AlertTriangle, LogOut, Trash2 } from 'lucide-react'

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
import { renumberApi, type RenumberPreview } from '@/ipc/renumber'
import { studentApi, type StudentDetail, type StudentInput } from '@/ipc/student'
import { AddressPanel } from './AddressPanel'
import { birthCell, genderLabel, statusBadge } from '@/lib/format'
import { NumberMoveDialog } from './NumberMoveDialog'
import { TransferOutDialog } from '@/features/transfer/TransferOutDialog'
import { SiblingPanel } from './SiblingPanel'
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
  /** 처음 열 칸. 확인 필요 화면에서 종류에 맞는 자리로 바로 보낼 때 쓴다 */
  initialTab?: TabKey
  onClose: () => void
  /** 저장·삭제 뒤 명단을 다시 읽게 한다 */
  onSaved: () => void
}

export function StudentDrawer({
  studentId,
  schoolYear,
  classSuggestions,
  initialTab,
  onClose,
  onSaved,
}: Props) {
  const qc = useQueryClient()
  const nav = useNavigate()
  const isNew = studentId === null
  const [tab, setTab] = useState<TabKey>(initialTab ?? 'basic')
  const [form, setForm] = useState<StudentInput>(() => emptyStudent(schoolYear))
  const [confirmDelete, setConfirmDelete] = useState(false)
  /** 번호를 옮기면 같은 반이 어떻게 바뀌는지 — 사용자가 확인할 때까지 저장하지 않는다 */
  const [movePreview, setMovePreview] = useState<RenumberPreview | null>(null)
  /** 전출 확인 창 */
  const [confirmOut, setConfirmOut] = useState(false)

  const detail = useQuery({
    queryKey: ['student', studentId, schoolYear],
    queryFn: () => studentApi.get(studentId!, schoolYear),
    enabled: !isNew,
  })

  // 학생이 바뀌면 입력 내용을 그 학생 것으로 되돌린다
  useEffect(() => {
    setTab(initialTab ?? 'basic')
    setConfirmDelete(false)
    setMovePreview(null)
    setConfirmOut(false)
    if (isNew) setForm(emptyStudent(schoolYear))
  }, [studentId, isNew, schoolYear, initialTab])

  useEffect(() => {
    if (detail.data) setForm(toInput(detail.data, schoolYear))
  }, [detail.data, schoolYear])

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['student', studentId] })
    onSaved()
  }

  const enrollment = detail.data?.enrollment
  // 같은 반 안에서 번호만 바뀌었을 때만 자동 재정렬을 묻는다.
  // 반이나 학년까지 옮기는 것은 자리 이동이지 번호 순서 조정이 아니다.
  const sameClass =
    !!enrollment &&
    form.grade === enrollment.grade &&
    form.className.trim() === (enrollment.className ?? '')
  const numberChanged =
    !!enrollment && form.classNo != null && form.classNo !== enrollment.classNo

  const save = useMutation({
    mutationFn: async (): Promise<'saved' | 'ask'> => {
      if (isNew) {
        await studentApi.create(form)
        return 'saved'
      }
      // 번호를 바꾸면 같은 반 학생이 따라 움직일 수 있다. 먼저 보여 주고 나서 저장한다.
      if (numberChanged && sameClass) {
        const p = await renumberApi.preview(studentId!, schoolYear, form.classNo!)
        if (p.blocked != null || p.affected > 0) {
          setMovePreview(p)
          return 'ask'
        }
      }
      await studentApi.update(studentId!, form)
      return 'saved'
    },
    onSuccess: (what) => {
      if (what === 'ask') return
      invalidate()
      if (isNew) onClose()
    },
  })

  /**
   * 사용자가 [번호 이동]을 눌렀을 때.
   *
   * 여러 학생이 걸린 번호 이동을 먼저 끝내고, 그다음 이 학생의 나머지 수정을 저장한다.
   * 순서를 이렇게 두면 번호 이동이 실패했을 때 아무것도 바뀌지 않는다.
   */
  const applyMove = useMutation({
    mutationFn: async () => {
      const p = movePreview!
      await renumberApi.apply({
        studentId: studentId!,
        schoolYear,
        newNo: p.to,
        stateKey: p.stateKey,
      })
      await studentApi.update(studentId!, form)
    },
    onSuccess: () => {
      setMovePreview(null)
      qc.invalidateQueries({ queryKey: ['issues'] })
      invalidate()
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
          {!isNew && !confirmDelete && enrollment && enrollment.status !== 'TRANSFER_OUT' && (
            <Button
              variant="ghost"
              size="sm"
              icon={LogOut}
              onClick={() => setConfirmOut(true)}
              title="학교를 떠난 학생을 현재 명단에서 뺍니다"
            >
              전출 처리
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
      {save.isSuccess && save.data === 'saved' && !isNew && !save.isPending && (
        <div style={{ marginBottom: 12 }}>
          <Notice tone="success">저장했습니다.</Notice>
        </div>
      )}
      {applyMove.isSuccess && !applyMove.isPending && (
        <div style={{ marginBottom: 12 }}>
          <Notice tone="success">번호를 바꾸고 저장했습니다.</Notice>
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
        <SiblingPanel
          studentId={d.id}
          studentName={d.name}
          schoolYear={schoolYear}
          onChanged={invalidate}
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
              기본정보 탭에서 값을 고쳐 저장하면 해당 표시는 저절로 사라집니다. 학교 전체를
              종류별로 모아 보려면 왼쪽 <b>확인 필요</b> 화면을 열어 주세요.
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

      {confirmOut && d && enrollment && (
        <TransferOutDialog
          studentId={d.id}
          studentName={d.name}
          classLabel={enrollment.classLabel}
          classNo={enrollment.classNo}
          schoolYear={schoolYear}
          onCancel={() => setConfirmOut(false)}
          onDone={() => {
            setConfirmOut(false)
            qc.invalidateQueries({ queryKey: ['transfer'] })
            qc.invalidateQueries({ queryKey: ['issues'] })
            qc.invalidateQueries({ queryKey: ['issue-summary'] })
            invalidate()
            onClose()
          }}
        />
      )}
      {movePreview && (
        <NumberMoveDialog
          preview={movePreview}
          busy={applyMove.isPending}
          error={applyMove.error}
          onCancel={() => setMovePreview(null)}
          onApply={() => applyMove.mutate()}
          onSeeDuplicates={() => {
            setMovePreview(null)
            onClose()
            nav('/issues')
          }}
        />
      )}
    </Drawer>
  )
}
