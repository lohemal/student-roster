import { Field, Input, Select, Textarea } from '@/components/ui'
import type { Gender, StudentInput } from '@/ipc/student'
import { birthDisplay } from '@/lib/format'
import s from './StudentForm.module.css'

export function emptyStudent(schoolYear: number, grade = 1): StudentInput {
  return {
    name: '',
    gender: '',
    birthRaw: '',
    addressRaw: '',
    fatherName: '',
    motherName: '',
    fatherPhone: '',
    motherPhone: '',
    primaryPhone: '',
    note: '',
    schoolYear,
    grade,
    className: '',
    classNo: null,
  }
}

/**
 * 생년월일 미리보기.
 *
 * 저장 규칙의 원본은 Rust다. 여기서는 사람이 적은 값이 어떻게 읽힐지
 * 미리 보여 주기만 한다 — 틀렸으면 저장하기 전에 알아차리라고.
 */
function previewBirth(raw: string): { text: string; warn: boolean } {
  const t = raw.trim()
  if (!t) return { text: '', warn: false }

  const parts = t.split(/\D+/).filter(Boolean)
  let y: number, m: number, d: number

  if (parts.length === 3 && (parts[0].length === 4 || parts[0].length === 2)) {
    y = Number(parts[0])
    m = Number(parts[1])
    d = Number(parts[2])
    if (parts[0].length === 2) y = guessCentury(y, m, d)
  } else {
    const digits = t.replace(/\D/g, '')
    if (digits.length === 8) {
      y = Number(digits.slice(0, 4))
      m = Number(digits.slice(4, 6))
      d = Number(digits.slice(6, 8))
    } else if (digits.length === 6) {
      y = guessCentury(Number(digits.slice(0, 2)), Number(digits.slice(2, 4)), Number(digits.slice(4, 6)))
      m = Number(digits.slice(2, 4))
      d = Number(digits.slice(4, 6))
    } else {
      return { text: '날짜로 읽을 수 없습니다. 그래도 저장은 됩니다.', warn: true }
    }
  }

  const date = new Date(y, m - 1, d)
  const real = date.getFullYear() === y && date.getMonth() === m - 1 && date.getDate() === d
  if (!real) return { text: '달력에 없는 날짜입니다. 그래도 저장은 됩니다.', warn: true }

  const iso = `${String(y).padStart(4, '0')}-${String(m).padStart(2, '0')}-${String(d).padStart(2, '0')}`
  const age = ageOn(date)
  const warn = age < 4 || age > 15
  return {
    text: `${birthDisplay(iso)} · 만 ${age}세${warn ? ' — 초등학생 범위를 벗어납니다' : ''}`,
    warn,
  }
}

function guessCentury(yy: number, m: number, d: number): number {
  const asTwoThousand = new Date(2000 + yy, m - 1, d)
  const age = ageOn(asTwoThousand)
  if (age >= 4 && age <= 15) return 2000 + yy
  const asNineteen = ageOn(new Date(1900 + yy, m - 1, d))
  if (asNineteen >= 4 && asNineteen <= 15) return 1900 + yy
  // 아직 오지 않은 날이면 1900년대로 읽는다
  return asTwoThousand > new Date() ? 1900 + yy : 2000 + yy
}

function ageOn(birth: Date, today = new Date()): number {
  let age = today.getFullYear() - birth.getFullYear()
  const beforeBirthday =
    today.getMonth() < birth.getMonth() ||
    (today.getMonth() === birth.getMonth() && today.getDate() < birth.getDate())
  if (beforeBirthday) age -= 1
  return age
}

interface Props {
  value: StudentInput
  onChange: (next: StudentInput) => void
  /** 그 학년도에 이미 있는 반 이름 — 입력 도우미로 보여 준다 */
  classSuggestions?: string[]
}

export function StudentForm({ value, onChange, classSuggestions = [] }: Props) {
  const set = <K extends keyof StudentInput>(key: K, v: StudentInput[K]) =>
    onChange({ ...value, [key]: v })

  const birth = previewBirth(value.birthRaw)

  return (
    <div className={s.form}>
      <section className={s.section}>
        <div className={s.sectionTitle}>학적</div>
        <div className={s.grid3}>
          <Field label="학년">
            <Select value={value.grade} onChange={(e) => set('grade', Number(e.target.value))}>
              {[1, 2, 3, 4, 5, 6].map((g) => (
                <option key={g} value={g}>
                  {g}학년
                </option>
              ))}
            </Select>
          </Field>
          <Field label="반" hint="숫자도 이름도 됩니다">
            <Input
              list="class-suggestions"
              value={value.className}
              onChange={(e) => set('className', e.target.value)}
              placeholder="예: 1 또는 가람"
              maxLength={20}
            />
            <datalist id="class-suggestions">
              {classSuggestions.map((c) => (
                <option key={c} value={c} />
              ))}
            </datalist>
          </Field>
          <Field label="번호" hint="나중에 정해도 됩니다">
            <Input
              type="number"
              min={1}
              max={200}
              value={value.classNo ?? ''}
              onChange={(e) => set('classNo', e.target.value === '' ? null : Number(e.target.value))}
              placeholder="비워 둘 수 있음"
            />
          </Field>
        </div>
      </section>

      <section className={s.section}>
        <div className={s.sectionTitle}>학생</div>
        <div className={s.grid2}>
          <Field label="이름">
            <Input
              value={value.name}
              onChange={(e) => set('name', e.target.value)}
              placeholder="예: 홍길동"
              maxLength={30}
              autoFocus
            />
          </Field>
          <Field label="성별">
            <div className={s.choice}>
              {(
                [
                  ['M', '남'],
                  ['F', '여'],
                  ['', '미정'],
                ] as [Gender | '', string][]
              ).map(([v, label]) => (
                <button
                  key={label}
                  type="button"
                  className={value.gender === v ? `${s.choiceBtn} ${s.choiceOn}` : s.choiceBtn}
                  onClick={() => set('gender', v)}
                >
                  {label}
                </button>
              ))}
            </div>
          </Field>
          <Field
            label="생년월일"
            hint={
              birth.text ? (
                <span className={birth.warn ? s.birthWarn : undefined}>{birth.text}</span>
              ) : (
                '170315 · 17.03.15 · 2017-03-15 모두 됩니다'
              )
            }
          >
            <Input
              value={value.birthRaw}
              onChange={(e) => set('birthRaw', e.target.value)}
              placeholder="예: 170315"
              maxLength={20}
            />
          </Field>
          <div />
          <Field className={s.full} label="주소" hint="입력한 그대로 보관합니다">
            <Input
              value={value.addressRaw}
              onChange={(e) => set('addressRaw', e.target.value)}
              placeholder="예: 세종특별자치시 한누리대로 123, 101동 1001호(가온마을5단지)"
              maxLength={200}
            />
          </Field>
        </div>
      </section>

      <section className={s.section}>
        <div className={s.sectionTitle}>보호자</div>
        <div className={s.grid2}>
          <Field label="부 성명">
            <Input
              value={value.fatherName}
              onChange={(e) => set('fatherName', e.target.value)}
              maxLength={30}
            />
          </Field>
          <Field label="부 연락처">
            <Input
              value={value.fatherPhone}
              onChange={(e) => set('fatherPhone', e.target.value)}
              placeholder="010-0000-0000"
              maxLength={40}
            />
          </Field>
          <Field label="모 성명">
            <Input
              value={value.motherName}
              onChange={(e) => set('motherName', e.target.value)}
              maxLength={30}
            />
          </Field>
          <Field label="모 연락처">
            <Input
              value={value.motherPhone}
              onChange={(e) => set('motherPhone', e.target.value)}
              placeholder="010-0000-0000"
              maxLength={40}
            />
          </Field>
          <Field
            className={s.full}
            label="주보호자 연락처"
            hint="명단에 대표로 나가는 번호입니다"
          >
            <Input
              value={value.primaryPhone}
              onChange={(e) => set('primaryPhone', e.target.value)}
              placeholder="010-0000-0000"
              maxLength={40}
            />
          </Field>
        </div>
      </section>

      <section className={s.section}>
        <div className={s.sectionTitle}>비고</div>
        <Textarea
          value={value.note}
          onChange={(e) => set('note', e.target.value)}
          placeholder="특이사항을 적어 두세요"
          maxLength={500}
        />
      </section>
    </div>
  )
}
