//! 본교 형제 판정.
//!
//! 보호자 정보 네 가지를 **같은 항목끼리만** 견준다.
//!
//! ```text
//! A 부 성명   ↔ B 부 성명
//! A 모 성명   ↔ B 모 성명
//! A 부 연락처 ↔ B 부 연락처
//! A 모 연락처 ↔ B 모 연락처
//! ```
//!
//! A 의 부 연락처와 B 의 모 연락처처럼 **엇갈려 견주지 않는다.**
//!
//! 지키는 것
//!   * **빈칸끼리는 일치가 아니다.** 빈칸 두 개를 일치로 세면 온 학교가 형제가 된다.
//!   * 값이 있는 항목이 **둘 이상** 같아야 후보다.
//!   * 이름을 비슷하다고 같게 보지 않는다. `김영희` 와 `김영이` 는 다른 사람이다.
//!   * 판정은 후보까지다. 형제인지 아닌지는 사람이 정한다.

use serde::{Deserialize, Serialize};

use super::phone;

/// 견주는 네 항목.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    FatherName,
    MotherName,
    FatherPhone,
    MotherPhone,
}

impl Field {
    pub const ALL: [Field; 4] = [
        Field::FatherName,
        Field::MotherName,
        Field::FatherPhone,
        Field::MotherPhone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Field::FatherName => "부 성명",
            Field::MotherName => "모 성명",
            Field::FatherPhone => "부 연락처",
            Field::MotherPhone => "모 연락처",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Field::FatherName => "fatherName",
            Field::MotherName => "motherName",
            Field::FatherPhone => "fatherPhone",
            Field::MotherPhone => "motherPhone",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Field::ALL.into_iter().find(|f| f.code() == code)
    }

    fn is_phone(self) -> bool {
        matches!(self, Field::FatherPhone | Field::MotherPhone)
    }
}

/// 한 학생의 보호자 정보. 저장된 원문 그대로 담고, 견주는 값은 여기서 만든다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guardians {
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
}

impl Guardians {
    pub fn raw(&self, f: Field) -> Option<&str> {
        let v = match f {
            Field::FatherName => &self.father_name,
            Field::MotherName => &self.mother_name,
            Field::FatherPhone => &self.father_phone,
            Field::MotherPhone => &self.mother_phone,
        };
        v.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }

    /// 값이 적혀 있는가. **빈칸 판단은 이것 하나로 한다.**
    pub fn has(&self, f: Field) -> bool {
        self.raw(f).is_some()
    }

    /// 견주기 좋게 다듬은 값. 이름은 공백을 없애고, 연락처는 숫자만 남긴다.
    ///
    /// 빈칸이면 None.
    pub fn compare_value(&self, f: Field) -> Option<String> {
        let raw = self.raw(f)?;
        Some(if f.is_phone() {
            phone::digits(raw)
        } else {
            normalize_name(raw)
        })
    }

    /// **일치로 셀 수 있는** 값인가.
    ///
    /// 연락처는 온전한 번호일 때만 센다 — 네 자리쯤 되는 값은 우연히 겹친다.
    /// 이름은 한 글자짜리를 빼면 그대로 쓴다.
    pub fn match_key(&self, f: Field) -> Option<String> {
        let v = self.compare_value(f)?;
        if v.is_empty() {
            return None;
        }
        if f.is_phone() {
            phone::is_comparable(&Some(v.clone())).then_some(v)
        } else {
            (v.chars().count() >= 2).then_some(v)
        }
    }
}

/// 이름 견주기용 정리 — 앞뒤와 가운데 공백을 없앤다.
///
/// `김 영희` 와 `김영희` 는 같은 사람이다. 하지만 `김영희` 와 `김영이` 는
/// **다른 사람**이므로 비슷하다고 묶지 않는다.
pub fn normalize_name(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 두 학생을 견준 결과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comparison {
    /// 값이 있고 서로 같은 항목
    pub matched: Vec<Field>,
    /// 양쪽 다 값이 있는데 서로 다른 항목
    pub conflicts: Vec<Field>,
}

impl Comparison {
    /// 형제 후보인가 — 값이 있는 항목이 둘 이상 같을 때.
    pub fn is_candidate(&self) -> bool {
        self.matched.len() >= MIN_MATCHES
    }
}

/// 후보로 보는 최소 일치 수.
pub const MIN_MATCHES: usize = 2;

/// 두 학생의 보호자 정보를 견준다.
pub fn compare(a: &Guardians, b: &Guardians) -> Comparison {
    let mut out = Comparison::default();
    for f in Field::ALL {
        // 일치: 양쪽 다 셀 수 있는 값이고 같을 때
        match (a.match_key(f), b.match_key(f)) {
            (Some(x), Some(y)) if x == y => {
                out.matched.push(f);
                continue;
            }
            _ => {}
        }
        // 불일치: 양쪽 다 적혀 있는데 다를 때. 빈칸은 불일치가 아니다.
        if a.has(f) && b.has(f) && a.compare_value(f) != b.compare_value(f) {
            out.conflicts.push(f);
        }
    }
    out
}

/// 한쪽이 비어 있어 형제에게서 가져올 수 있는 항목.
///
/// `from` 에는 있고 `to` 에는 없는 것만 고른다. **있는 값은 절대 건드리지 않는다.**
pub fn fillable(to: &Guardians, from: &Guardians) -> Vec<Field> {
    Field::ALL
        .into_iter()
        .filter(|f| !to.has(*f) && from.has(*f))
        .collect()
}

/// 항목 목록을 JSON 배열 글로. `sibling_links` 에 저장한다.
pub fn to_json(fields: &[Field]) -> String {
    serde_json::to_string(fields).unwrap_or_else(|_| "[]".into())
}

#[allow(dead_code)] // 저장된 값을 되읽을 때 쓴다 (검사에서 확인)
pub fn from_json(s: &str) -> Vec<Field> {
    serde_json::from_str(s).unwrap_or_default()
}

// ---------------------------------------------------------------
// 후보 쌍 찾기
// ---------------------------------------------------------------

/// 학생 한 명의 견줄 값. 훑기(scan)에 넘긴다.
#[derive(Debug, Clone)]
pub struct Entry {
    pub student_id: i64,
    pub guardians: Guardians,
}

/// 같은 값을 너무 많이 나눠 가진 항목은 사람을 가려내지 못한다.
///
/// 학교 대표번호를 모든 학생에게 적어 둔 자료 같은 경우인데, 그대로 두면
/// 쌍이 수만 개로 불어난다. 이런 값은 후보를 만드는 데 쓰지 않고 몇 건인지만 알린다.
pub const BUCKET_LIMIT: usize = 30;

#[derive(Debug, Clone, Default)]
pub struct PairSearch {
    /// 견줘 볼 만한 쌍 (작은 id 가 앞)
    pub pairs: Vec<(i64, i64)>,
    /// 너무 흔해서 쓰지 않은 값의 수
    pub skipped_values: usize,
}

/// 같은 값을 가진 학생끼리만 묶어 견줄 쌍을 만든다.
///
/// 모든 조합을 견주면 1,000명에 50만 번이다. 네 항목의 값으로 묶어 두면
/// 실제로 견줄 쌍은 몇백 개에 그친다.
pub fn find_pairs(entries: &[Entry]) -> PairSearch {
    use std::collections::{HashMap, HashSet};

    let mut buckets: HashMap<(Field, String), Vec<i64>> = HashMap::new();
    for e in entries {
        for f in Field::ALL {
            if let Some(key) = e.guardians.match_key(f) {
                buckets.entry((f, key)).or_default().push(e.student_id);
            }
        }
    }

    let mut seen: HashSet<(i64, i64)> = HashSet::new();
    let mut skipped = 0;
    for ids in buckets.values() {
        if ids.len() < 2 {
            continue;
        }
        if ids.len() > BUCKET_LIMIT {
            skipped += 1;
            continue;
        }
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                seen.insert(if a < b { (*a, *b) } else { (*b, *a) });
            }
        }
    }

    let mut pairs: Vec<(i64, i64)> = seen.into_iter().collect();
    pairs.sort_unstable();
    PairSearch {
        pairs,
        skipped_values: skipped,
    }
}

#[cfg(test)]
#[path = "sibling_tests.rs"]
mod sibling_tests;
