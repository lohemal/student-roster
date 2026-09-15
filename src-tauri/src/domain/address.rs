//! 주소 뜯어보기와 분류 판정.
//!
//! 주소는 사람이 손으로 적는 자료 가운데 가장 제멋대로다. 같은 건물인데도
//!
//! ```text
//! ○○로 123, 101동 1001호
//! ○○로123 105동 804호
//! ○○로 123(○○마을5단지)
//! ```
//!
//! 처럼 적힌다. 그래서 **동·호수처럼 학생마다 달라지는 부분은 버리고**
//! 건물을 가리키는 `도로명 + 건물번호` 를 뽑아 규칙의 기준으로 삼는다.
//!
//! 지키는 것
//!   * 원본은 절대 건드리지 않는다. 여기서는 읽기만 한다.
//!   * **확실하지 않으면 정하지 않는다.** 잘못된 5단지 통계보다 미분류 한 명이 낫다.
//!   * 판정의 근거를 함께 돌려준다 — 화면이 왜 그렇게 됐는지 보여 줄 수 있어야 한다.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------
// 정규화
// ---------------------------------------------------------------

/// 주소를 뜯어본 결과. 원본은 들고 있지 않다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    /// 검색·규칙 맞춤에 쓰는 정리본. 사람이 읽을 수 있는 모양을 지킨다.
    pub norm: String,
    /// `○○로 123` — 건물을 가리키는 안정적인 부분. 뽑지 못하면 None
    pub road: Option<String>,
    /// 주소에 적힌 단지 이름들. 긴 것부터 (`가온마을5단지`, `5단지`)
    pub complexes: Vec<String>,
}

/// 전각 숫자·괄호를 반각으로, 이어진 공백을 하나로.
///
/// 지나치게 바꾸지 않는다 — 서로 다른 건물이 같은 주소가 되면 안 된다.
pub fn normalize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_space = true; // 앞쪽 공백을 버리려고 true 로 시작
    for ch in raw.chars() {
        let c = match ch {
            // 엑셀에서 붙여 넣으면 전각이 섞여 들어온다
            '０'..='９' => char::from_u32(ch as u32 - '０' as u32 + '0' as u32).unwrap_or(ch),
            'Ａ'..='Ｚ' => char::from_u32(ch as u32 - 'Ａ' as u32 + 'A' as u32).unwrap_or(ch),
            'ａ'..='ｚ' => char::from_u32(ch as u32 - 'ａ' as u32 + 'a' as u32).unwrap_or(ch),
            '（' => '(',
            '）' => ')',
            '［' | '【' => '[',
            '］' | '】' => ']',
            '－' | '–' | '—' | '‐' => '-',
            '，' => ',',
            '　' => ' ',
            other => other,
        };
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.trim_end().to_string()
}

/// 공백을 모두 뺀 값. `가온마을 5단지` 와 `가온마을5단지` 를 같게 보려고 쓴다.
pub fn squeeze(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 주소를 뜯어본다.
pub fn parse(raw: &str) -> Parsed {
    let norm = normalize(raw);
    if norm.is_empty() {
        return Parsed::default();
    }
    Parsed {
        road: extract_road(&norm),
        complexes: extract_complexes(&norm),
        norm,
    }
}

// ---------------------------------------------------------------
// 도로명 + 건물번호 뽑기
// ---------------------------------------------------------------

fn is_road_tail(c: char) -> bool {
    c == '로' || c == '길'
}

/// 토막 끝에 붙은 쉼표·마침표·괄호를 턴다.
fn trim_edges(tok: &str) -> &str {
    tok.trim_matches(|c: char| matches!(c, ',' | '.' | '(' | ')' | '[' | ']' | '·' | ';'))
}

/// `123`, `123-4` 처럼 건물번호로 볼 수 있는가.
fn as_building_no(tok: &str) -> Option<&str> {
    let t = trim_edges(tok);
    if t.is_empty() {
        return None;
    }
    let mut seen_digit = false;
    let mut seen_dash = false;
    for c in t.chars() {
        if c.is_ascii_digit() {
            seen_digit = true;
        } else if c == '-' && seen_digit && !seen_dash {
            seen_dash = true;
        } else {
            return None;
        }
    }
    // '123-' 처럼 끝나면 번호로 보지 않는다
    if t.ends_with('-') {
        return None;
    }
    seen_digit.then_some(t)
}

/// `○○로123` 처럼 번호가 붙어 있으면 (`○○로`, `123`) 으로 가른다.
fn split_glued(tok: &str) -> Option<(&str, &str)> {
    let t = trim_edges(tok);
    let cut = t.char_indices().filter(|(_, c)| is_road_tail(*c)).next_back()?;
    let (name_end, tail_char) = cut;
    let name = &t[..name_end + tail_char.len_utf8()];
    let rest = &t[name_end + tail_char.len_utf8()..];
    if name.chars().count() < 2 || rest.is_empty() {
        return None;
    }
    as_building_no(rest).map(|n| (name, n))
}

/// 정리본에서 `도로명 건물번호` 를 뽑는다.
///
/// `101동 1001호` 는 로·길로 끝나지 않으므로 걸리지 않는다.
/// `○○로3길 25` 처럼 로와 길이 겹치면 **뒤에 오는 것(더 좁은 길)** 을 쓴다.
pub fn extract_road(norm: &str) -> Option<String> {
    // 괄호·쉼표도 토막을 가르는 것으로 본다.
    // '123(가온마을5단지)' 를 한 덩어리로 보면 건물번호를 못 읽는다.
    let tokens: Vec<&str> = norm
        .split(|c: char| c.is_whitespace() || "(),[]".contains(c))
        .filter(|t| !t.is_empty())
        .collect();
    let mut found: Option<String> = None;

    for (i, tok) in tokens.iter().enumerate() {
        let t = trim_edges(tok);

        // ① `○○로123` — 번호가 붙어 있다
        if let Some((name, no)) = split_glued(t) {
            found = Some(join_road(&tokens, i, name, no));
            continue;
        }

        // ② `○○로` + 다음 토막이 번호
        if t.chars().count() >= 2 && t.chars().next_back().is_some_and(is_road_tail) {
            if let Some(no) = tokens.get(i + 1).and_then(|n| as_building_no(n)) {
                found = Some(join_road(&tokens, i, t, no));
            }
        }
    }
    found
}

/// `○○로` + `123번길` 처럼 앞 도로명이 따로 떨어져 있으면 붙여 준다.
///
/// 도로명이 숫자로 시작하면 그것만으로는 어느 길인지 알 수 없기 때문이다.
fn join_road(tokens: &[&str], i: usize, name: &str, no: &str) -> String {
    if name.starts_with(|c: char| c.is_ascii_digit()) && i > 0 {
        let prev = trim_edges(tokens[i - 1]);
        if prev.chars().next_back().is_some_and(is_road_tail) {
            return format!("{prev}{name} {no}");
        }
    }
    format!("{name} {no}")
}

// ---------------------------------------------------------------
// 단지 이름 뽑기
// ---------------------------------------------------------------

/// 주소에 적힌 단지 이름을 **긴 것부터** 모은다.
///
/// `가온마을5단지` 면 `["가온마을5단지", "5단지"]`.
/// 반드시 `단지` 라는 말이 있어야 한다 — 건물번호를 단지로 잘못 읽지 않기 위해서다.
pub fn extract_complexes(norm: &str) -> Vec<String> {
    let squeezed = squeeze(norm);
    let chars: Vec<char> = squeezed.chars().collect();
    let mut out: Vec<String> = Vec::new();

    let mut i = 0;
    // 앞선 단지 이름이 끝난 자리. 이름을 거꾸로 훑을 때 그 너머로 넘어가지 않는다.
    let mut last_end = 0;
    while i < chars.len() {
        // '단지' 를 찾는다
        if chars[i] == '단' && chars.get(i + 1) == Some(&'지') {
            // 바로 앞의 숫자들
            let mut num_start = i;
            while num_start > last_end && chars[num_start - 1].is_ascii_digit() {
                num_start -= 1;
            }
            if num_start < i {
                // `5단지`
                let short: String = chars[num_start..i + 2].iter().collect();

                // 그 앞에 붙은 이름 (`가온마을`)
                let mut name_start = num_start;
                while name_start > last_end && is_name_char(chars[name_start - 1]) {
                    name_start -= 1;
                }
                if name_start < num_start {
                    let long: String = chars[name_start..i + 2].iter().collect();
                    if long != short {
                        out.push(long);
                    }
                }
                out.push(short);
                last_end = i + 2;
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    // 긴 것부터, 겹치는 것은 지운다
    out.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    out.dedup();
    out
}

/// 단지 앞에 붙는 이름에 쓸 수 있는 글자인가 (한글과 영문).
fn is_name_char(c: char) -> bool {
    ('가'..='힣').contains(&c) || c.is_ascii_alphabetic()
}

// ---------------------------------------------------------------
// 규칙 맞추기
// ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RuleKind {
    /// 도로명 + 건물번호가 정확히 같을 때만. 가장 믿을 만하다
    Road,
    /// 단지 이름이 들어 있을 때 (공백 차이는 무시)
    Complex,
    /// 단순 포함. 넓게 걸리므로 가장 마지막
    Contains,
}

impl RuleKind {
    pub fn code(self) -> &'static str {
        match self {
            RuleKind::Road => "ROAD",
            RuleKind::Complex => "COMPLEX",
            RuleKind::Contains => "CONTAINS",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "ROAD" => Some(RuleKind::Road),
            "COMPLEX" => Some(RuleKind::Complex),
            "CONTAINS" => Some(RuleKind::Contains),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RuleKind::Road => "도로명",
            RuleKind::Complex => "단지명",
            RuleKind::Contains => "포함",
        }
    }

    /// 화면에서 이 규칙이 얼마나 넓게 걸리는지 알려 줄 문장.
    pub fn hint(self) -> &'static str {
        match self {
            RuleKind::Road => "도로명과 건물번호가 정확히 같은 주소에만 적용됩니다. 동·호수는 달라도 됩니다.",
            RuleKind::Complex => "주소에 이 단지 이름이 들어 있으면 적용됩니다. 띄어쓰기는 달라도 됩니다.",
            RuleKind::Contains => "주소에 이 글자가 들어 있기만 하면 적용됩니다. 넓게 걸리므로 짧은 글자는 피해 주세요.",
        }
    }
}

/// 규칙을 저장하기 좋은 모양으로 다듬는다.
///
/// ROAD 는 사용자가 주소를 통째로 붙여 넣어도 `도로명 건물번호` 만 남긴다.
pub fn normalize_pattern(kind: RuleKind, pattern: &str) -> String {
    let n = normalize(pattern);
    match kind {
        RuleKind::Road => extract_road(&n).unwrap_or(n),
        _ => n,
    }
}

/// 이 주소가 규칙에 걸리는가.
pub fn matches(kind: RuleKind, pattern: &str, parsed: &Parsed) -> bool {
    let p = squeeze(&normalize_pattern(kind, pattern));
    if p.is_empty() {
        return false;
    }
    match kind {
        // 정확히 같아야 한다. '○○로 12' 가 '○○로 123' 에 걸리면 안 된다
        RuleKind::Road => parsed.road.as_deref().map(squeeze) == Some(p),
        // 단지 이름은 주소 어디에 있어도 된다 (띄어쓰기 무시)
        RuleKind::Complex => squeeze(&parsed.norm).contains(&p),
        RuleKind::Contains => squeeze(&parsed.norm).contains(&p),
    }
}

// ---------------------------------------------------------------
// 판정
// ---------------------------------------------------------------

/// 판정에 쓰는 규칙 한 줄. DB 를 모르기 위해 값만 담는다.
#[derive(Debug, Clone)]
pub struct RuleRef {
    pub id: i64,
    pub kind: RuleKind,
    pub pattern: String,
    pub category_id: i64,
    pub category_name: String,
}

/// 주소 분류 한 줄.
#[derive(Debug, Clone)]
pub struct CategoryRef {
    pub id: i64,
    pub name: String,
}

/// 어떤 규칙이 어느 분류를 가리켰는지.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub rule_id: i64,
    pub kind: String,
    pub pattern: String,
    pub category_id: i64,
    pub category_name: String,
}

/// 판정 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// 주소가 없다
    NoAddress,
    /// 사용자가 만든 규칙으로 정했다
    Rule { category_id: i64, hit: Hit },
    /// 주소에 적힌 단지 이름으로 저절로 정했다
    Auto { category_id: i64, token: String },
    /// 같은 순위 규칙이 서로 다른 분류를 가리킨다 — 정하지 않는다
    Conflict { hits: Vec<Hit> },
    /// 걸리는 것이 없다
    Unclassified,
}

impl Decision {
    pub fn category_id(&self) -> Option<i64> {
        match self {
            Decision::Rule { category_id, .. } | Decision::Auto { category_id, .. } => {
                Some(*category_id)
            }
            _ => None,
        }
    }

    /// `students.address_source` 에 넣을 값.
    pub fn source(&self) -> &'static str {
        match self {
            Decision::Rule { .. } => "RULE",
            Decision::Auto { .. } => "AUTO",
            Decision::Conflict { .. } => "CONFLICT",
            Decision::NoAddress | Decision::Unclassified => "NONE",
        }
    }

    pub fn rule_id(&self) -> Option<i64> {
        match self {
            Decision::Rule { hit, .. } => Some(hit.rule_id),
            _ => None,
        }
    }

    /// 화면에 보여 줄 판정 근거.
    pub fn reason(&self) -> String {
        match self {
            Decision::NoAddress => "주소가 없습니다.".into(),
            Decision::Rule { hit, .. } => format!(
                "주소 규칙으로 분류 ({} · {})",
                RuleKind::parse(&hit.kind)
                    .map(|k| k.label())
                    .unwrap_or("규칙"),
                hit.pattern
            ),
            Decision::Auto { token, .. } => format!("주소에 적힌 '{token}' 으로 자동 분류"),
            Decision::Conflict { hits } => {
                let names: Vec<&str> = hits.iter().map(|h| h.category_name.as_str()).collect();
                format!("규칙이 서로 다른 분류를 가리킵니다 ({})", names.join(" / "))
            }
            Decision::Unclassified => "걸리는 주소 규칙이 없습니다.".into(),
        }
    }
}

/// 주소 하나를 판정한다. **MANUAL 인 학생은 여기까지 오지 않는다** — 부르는 쪽에서 거른다.
///
/// 순서: ROAD → COMPLEX → CONTAINS → 주소에 적힌 단지 이름 → 미분류.
/// 같은 순위에서 서로 다른 분류가 걸리면 **고르지 않고** 충돌로 둔다.
pub fn classify(parsed: &Parsed, rules: &[RuleRef], categories: &[CategoryRef]) -> Decision {
    if parsed.norm.is_empty() {
        return Decision::NoAddress;
    }

    for kind in [RuleKind::Road, RuleKind::Complex, RuleKind::Contains] {
        let hits: Vec<Hit> = rules
            .iter()
            .filter(|r| r.kind == kind && matches(r.kind, &r.pattern, parsed))
            .map(|r| Hit {
                rule_id: r.id,
                kind: r.kind.code().to_string(),
                pattern: r.pattern.clone(),
                category_id: r.category_id,
                category_name: r.category_name.clone(),
            })
            .collect();

        if hits.is_empty() {
            continue;
        }

        let mut distinct: Vec<i64> = hits.iter().map(|h| h.category_id).collect();
        distinct.sort_unstable();
        distinct.dedup();

        if distinct.len() == 1 {
            // 같은 분류를 가리키는 규칙이 여럿이면 가장 먼저 만든 것을 근거로 남긴다
            let hit = hits
                .iter()
                .min_by_key(|h| h.rule_id)
                .cloned()
                .expect("hits 가 비어 있지 않다");
            return Decision::Rule {
                category_id: hit.category_id,
                hit,
            };
        }
        return Decision::Conflict { hits };
    }

    auto_by_complex(parsed, categories)
}

/// 주소에 적힌 단지 이름으로 저절로 정한다.
///
/// 긴 이름부터 본다 — `가온마을5단지` 분류가 있으면 그것을 쓰고, 없을 때만 `5단지` 를 본다.
/// 같은 길이에서 둘 이상이 걸리면 고르지 않는다.
pub fn auto_by_complex(parsed: &Parsed, categories: &[CategoryRef]) -> Decision {
    // 이름 길이가 같은 것끼리 묶어 본다. `가온마을5단지` 분류가 있으면 그것이 먼저고,
    // 없을 때에야 `5단지` 를 본다. 같은 길이에서 둘 이상이 걸리면 고르지 않는다.
    let mut i = 0;
    while i < parsed.complexes.len() {
        let len = parsed.complexes[i].chars().count();
        let mut j = i;
        while j < parsed.complexes.len() && parsed.complexes[j].chars().count() == len {
            j += 1;
        }

        let mut hits: Vec<(i64, String)> = Vec::new();
        for token in &parsed.complexes[i..j] {
            let squeezed = squeeze(token);
            for c in categories.iter().filter(|c| squeeze(&c.name) == squeezed) {
                if !hits.iter().any(|(id, _)| *id == c.id) {
                    hits.push((c.id, token.clone()));
                }
            }
        }

        match hits.len() {
            0 => i = j, // 이 길이에서는 걸리는 분류가 없다 — 더 짧은 이름을 본다
            1 => {
                return Decision::Auto {
                    category_id: hits[0].0,
                    token: hits[0].1.clone(),
                }
            }
            // 한 주소에 서로 다른 단지가 둘 이상 적혀 있다 — 어느 쪽인지 알 수 없다
            _ => return Decision::Unclassified,
        }
    }
    Decision::Unclassified
}

// ---------------------------------------------------------------
// 규칙 제안
// ---------------------------------------------------------------

/// 사용자가 이 주소를 보고 규칙을 만들려 할 때 내놓을 후보.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub kind: String,
    pub kind_label: String,
    pub pattern: String,
    pub hint: String,
}

/// 이 주소에서 만들 수 있는 규칙 후보를 믿음직한 순서로 내놓는다.
///
/// **저절로 저장하지 않는다.** 사람이 보고 고른다.
pub fn suggest(parsed: &Parsed) -> Vec<Suggestion> {
    let mut out = Vec::new();

    if let Some(road) = &parsed.road {
        out.push(make_suggestion(RuleKind::Road, road));
    }
    for token in &parsed.complexes {
        out.push(make_suggestion(RuleKind::Complex, token));
    }
    out
}

fn make_suggestion(kind: RuleKind, pattern: &str) -> Suggestion {
    Suggestion {
        kind: kind.code().to_string(),
        kind_label: kind.label().to_string(),
        pattern: pattern.to_string(),
        hint: kind.hint().to_string(),
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod address_tests;
