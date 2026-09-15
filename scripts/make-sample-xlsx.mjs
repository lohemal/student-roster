/**
 * 가져오기를 시험할 엑셀 명단을 만든다.
 *
 *   npm run sample                  # 1,200명, scratch/시험-학생명단.xlsx
 *   npm run sample -- 300 out.xlsx  # 인원과 파일 이름을 정해서
 *   npm run sample -- 1200 out.xlsx --changed   # 연락처를 바꾼 판 (재가져오기 시험용)
 *
 * 학교에서 실제로 쓰는 파일과 비슷하게 일부러 지저분하게 만든다.
 *   * 맨 윗줄은 제목 — 헤더가 둘째 줄에 있다
 *   * 열 이름이 프로그램 항목과 다르다 (학급·성명·출석번호·보호자 연락처)
 *   * 번호·생년월일은 숫자 칸, 부 연락처는 앞자리 0이 빠진 숫자 칸
 *   * 달력에 없는 날짜, 빈 성별·연락처가 섞여 있다
 *
 * 외부 묶음을 쓰지 않고 zip(xlsx)을 직접 쓴다.
 */
import { deflateRawSync } from 'node:zlib'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'

const args = process.argv.slice(2)
const CHANGED = args.includes('--changed')
const rest = args.filter((a) => !a.startsWith('--'))
const COUNT = Number(rest[0] ?? 1200)
const OUT = resolve(rest[1] ?? 'scratch/시험-학생명단.xlsx')

// ---- 자료 만들기 -----------------------------------------------------------

let seed = 20260915
function rnd() {
  seed = (seed + 0x6d2b79f5) | 0
  let t = seed
  t = Math.imul(t ^ (t >>> 15), t | 1)
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296
}
const pick = (a) => a[Math.floor(rnd() * a.length)]
const chance = (p) => rnd() < p

const SURNAMES = [...'김이박최정강조윤장임한오서신권황안송류전홍고문양손배백허유남심노하곽성차주우구']
const GIVEN = [...'민서지예수윤하은도준우현연아린채원빈솔결온새']

const CLASSES = ['가람', '나리', '다솜']
const HEADERS = [
  '학년', '학급', '출석번호', '성명', '성별', '생년월일',
  '주소', '아버지', '어머니', '부 연락처', '모 연락처', '보호자 연락처', '비고',
]

function makeRows() {
  const rows = []
  const counters = new Map()

  for (let i = 0; i < COUNT; i++) {
    const grade = 1 + Math.floor(rnd() * 6)
    const cls = pick(CLASSES)
    const key = `${grade}-${cls}`
    const no = (counters.get(key) ?? 0) + 1
    counters.set(key, no)

    const name = pick(SURNAMES) + pick(GIVEN) + pick(GIVEN)
    const birthYear = 2026 - grade - 6
    const mm = 1 + Math.floor(rnd() * 12)
    const dd = 1 + Math.floor(rnd() * 28)
    const pad = (n) => String(n).padStart(2, '0')

    // 1% 는 달력에 없는 날짜 — 등록은 되고 확인 필요가 붙어야 한다
    const birth = chance(0.01)
      ? Number(`${String(birthYear).slice(2)}${pad(mm)}32`)
      : Number(`${String(birthYear).slice(2)}${pad(mm)}${pad(dd)}`)

    const tail = CHANGED ? 9999 : (i * 7) % 10000
    rows.push([
      grade,                                   // 숫자 칸
      cls,
      no,                                      // 숫자 칸
      name,
      chance(0.02) ? '' : chance(0.5) ? '남' : '여',
      birth,                                   // 숫자 칸
      chance(0.02) ? '' : `○○시 ○○로 ${1 + Math.floor(rnd() * 400)}`,
      `${pick(SURNAMES)}○○`,
      `${pick(SURNAMES)}○○`,
      // 앞자리 0이 빠진 숫자 칸 (엑셀에서 흔한 일)
      Number(`10${pad(Math.floor(rnd() * 100))}${String(1000 + Math.floor(rnd() * 9000))}${pad(Math.floor(rnd() * 100))}`.slice(0, 10)),
      chance(0.02) ? '' : `010-${String(1000 + Math.floor(rnd() * 9000))}-${String(tail).padStart(4, '0')}`,
      `010-${String(1000 + Math.floor(rnd() * 9000))}-${String(1000 + Math.floor(rnd() * 9000))}`,
      chance(0.05) ? '알레르기 있음' : '',
    ])
  }
  return rows
}

// ---- xlsx 쓰기 -------------------------------------------------------------

const esc = (s) =>
  String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

function colName(n) {
  let s = ''
  n += 1
  while (n > 0) {
    const r = (n - 1) % 26
    s = String.fromCharCode(65 + r) + s
    n = Math.floor((n - 1) / 26)
  }
  return s
}

function sheetXml(rows) {
  const out = []
  out.push(
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>',
  )
  rows.forEach((cells, r) => {
    const n = r + 1
    const parts = cells
      .map((v, c) => {
        if (v === '' || v === null || v === undefined) return ''
        const ref = `${colName(c)}${n}`
        return typeof v === 'number'
          ? `<c r="${ref}"><v>${v}</v></c>`
          : `<c r="${ref}" t="inlineStr"><is><t xml:space="preserve">${esc(v)}</t></is></c>`
      })
      .join('')
    out.push(`<row r="${n}">${parts}</row>`)
  })
  out.push('</sheetData></worksheet>')
  return out.join('')
}

// 최소한의 zip (deflate) — xlsx 는 zip 이다
function crc32(buf) {
  let table = crc32.table
  if (!table) {
    table = crc32.table = new Int32Array(256)
    for (let n = 0; n < 256; n++) {
      let c = n
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
      table[n] = c
    }
  }
  let crc = -1
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buf[i]) & 0xff]
  return (crc ^ -1) >>> 0
}

function zip(files) {
  const chunks = []
  const central = []
  let offset = 0

  for (const { name, data } of files) {
    const nameBuf = Buffer.from(name, 'utf8')
    const body = deflateRawSync(data, { level: 9 })
    const crc = crc32(data)

    const local = Buffer.alloc(30)
    local.writeUInt32LE(0x04034b50, 0)
    local.writeUInt16LE(20, 4)
    local.writeUInt16LE(0x800, 6) // UTF-8 이름
    local.writeUInt16LE(8, 8) // deflate
    local.writeUInt32LE(crc, 14)
    local.writeUInt32LE(body.length, 18)
    local.writeUInt32LE(data.length, 22)
    local.writeUInt16LE(nameBuf.length, 26)
    chunks.push(local, nameBuf, body)

    const cd = Buffer.alloc(46)
    cd.writeUInt32LE(0x02014b50, 0)
    cd.writeUInt16LE(20, 4)
    cd.writeUInt16LE(20, 6)
    cd.writeUInt16LE(0x800, 8)
    cd.writeUInt16LE(8, 10)
    cd.writeUInt32LE(crc, 16)
    cd.writeUInt32LE(body.length, 20)
    cd.writeUInt32LE(data.length, 24)
    cd.writeUInt16LE(nameBuf.length, 28)
    cd.writeUInt32LE(offset, 42)
    central.push(cd, nameBuf)

    offset += local.length + nameBuf.length + body.length
  }

  const centralBuf = Buffer.concat(central)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(files.length, 8)
  end.writeUInt16LE(files.length, 10)
  end.writeUInt32LE(centralBuf.length, 12)
  end.writeUInt32LE(offset, 16)

  return Buffer.concat([...chunks, centralBuf, end])
}

const rows = [
  ['2026학년도 학생명단'],
  HEADERS,
  ...makeRows(),
]

const files = [
  {
    name: '[Content_Types].xml',
    data: Buffer.from(
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>',
      'utf8',
    ),
  },
  {
    name: '_rels/.rels',
    data: Buffer.from(
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>',
      'utf8',
    ),
  },
  {
    name: 'xl/workbook.xml',
    data: Buffer.from(
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="학생명단" sheetId="1" r:id="rId1"/></sheets></workbook>',
      'utf8',
    ),
  },
  {
    name: 'xl/_rels/workbook.xml.rels',
    data: Buffer.from(
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>',
      'utf8',
    ),
  },
  { name: 'xl/worksheets/sheet1.xml', data: Buffer.from(sheetXml(rows), 'utf8') },
]

mkdirSync(dirname(OUT), { recursive: true })
writeFileSync(OUT, zip(files))

console.log(`만들었습니다: ${OUT}`)
console.log(`  학생 ${COUNT}명 · 시트 '학생명단' · 헤더는 둘째 줄`)
if (CHANGED) console.log('  (모 연락처 끝자리를 9999 로 바꾼 판입니다)')
