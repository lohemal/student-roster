//! 학생 번호 재정렬.
//!
//! 한 학생의 번호를 바꾸면 같은 반 학생들의 번호가 따라 움직여야 순서가 유지된다.
//! 23번을 7번으로 옮기면 7~22번이 한 칸씩 밀려 8~23번이 된다.
//!
//! 지키는 것
//!   * **같은 반 안에서만** 움직인다. 반이 다른 학생은 이 계산에 들어오지도 않는다.
//!   * **번호 집합을 바꾸지 않는다.** 누가 어느 번호를 갖는지만 바뀐다. 그래서
//!     `1,2,3,5,6,9` 처럼 띄엄띄엄한 반을 억지로 `1~6` 으로 다시 매기지 않는다.
//!   * **빈 번호로 옮기는 것은 그 학생만 바뀐다.** 아무도 밀 필요가 없다.
//!   * 번호가 겹치는 반에서는 계산하지 않는다 — 어느 쪽을 밀어야 할지 알 수 없다.
//!   * 여기는 계산만 한다. 저장은 `repo::renumber` 가 한다.

use serde::Serialize;

/// 번호로 쓸 수 있는 범위. `StudentInput::validate` 와 같은 값이다.
pub const MIN_NO: i32 = 1;
pub const MAX_NO: i32 = 200;

/// 한 반의 자리 하나. 번호가 없는 학생도 들어온다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    pub student_id: i64,
    pub class_no: Option<i32>,
}

impl Seat {
    pub fn new(student_id: i64, class_no: Option<i32>) -> Self {
        Self {
            student_id,
            class_no,
        }
    }
}

/// 학생 한 명의 번호가 바뀌는 것.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Move {
    pub student_id: i64,
    /// 바뀌기 전 번호. 번호가 없던 학생이면 None
    pub from: Option<i32>,
    pub to: i32,
}

/// 계산한 변경 계획의 성격. 화면이 무엇을 물어볼지 정하는 데 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 바뀌는 것이 없다
    None,
    /// 빈 번호를 준다 — 다른 학생은 그대로다
    Assign,
    /// 순서를 지키며 옮긴다 — 사이에 있는 학생들이 밀린다
    Reorder,
    /// 번호가 없던 학생을 사이에 끼워 넣는다 — 뒤 학생들이 한 칸씩 밀린다
    Insert,
}

impl Kind {
    pub fn code(self) -> &'static str {
        match self {
            Kind::None => "NONE",
            Kind::Assign => "ASSIGN",
            Kind::Reorder => "REORDER",
            Kind::Insert => "INSERT",
        }
    }
}

/// 자동으로 번호를 옮길 수 없는 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// 번호로 쓸 수 없는 값
    BadNumber(i32),
    /// 그 반에서 이 학생을 찾지 못했다
    NotInClass,
    /// 이 반에 이미 번호가 겹치는 학생이 있다
    Duplicate(i32),
}

impl Blocked {
    /// 사용자에게 보여 줄 문장. 왜 못 하는지와 무엇을 하면 되는지를 함께 적는다.
    pub fn message(&self, class_label: &str) -> String {
        match self {
            Blocked::BadNumber(_) => {
                format!("번호는 {MIN_NO}~{MAX_NO} 사이의 정수로 입력해 주세요.")
            }
            Blocked::NotInClass => {
                "이 학년도 학적을 찾지 못해 번호를 옮길 수 없습니다.".to_string()
            }
            Blocked::Duplicate(n) => format!(
                "현재 {class_label}반에 {n}번을 쓰는 학생이 둘 이상 있어 자동 번호 변경을 할 수 없습니다. \
                 먼저 중복된 번호를 정리해 주세요."
            ),
        }
    }
}

/// 번호 변경 계획.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub kind: Kind,
    /// 실제로 번호가 달라지는 학생만 담는다. 대상 학생도 여기 들어 있다.
    pub moves: Vec<Move>,
}

impl Plan {
    fn none() -> Self {
        Self {
            kind: Kind::None,
            moves: Vec::new(),
        }
    }

    /// 대상 학생을 뺀, 덩달아 번호가 바뀌는 인원.
    pub fn affected(&self, student_id: i64) -> usize {
        self.moves.iter().filter(|m| m.student_id != student_id).count()
    }
}

/// 한 학생의 번호를 `new_no` 로 바꾸는 계획을 세운다.
///
/// `seats` 는 **같은 학년도·학년·반** 학생 전부다. 번호가 없는 학생도 넣어 준다.
pub fn plan(seats: &[Seat], student_id: i64, new_no: i32) -> Result<Plan, Blocked> {
    if !(MIN_NO..=MAX_NO).contains(&new_no) {
        return Err(Blocked::BadNumber(new_no));
    }
    let target = seats
        .iter()
        .find(|s| s.student_id == student_id)
        .ok_or(Blocked::NotInClass)?;

    // 번호가 있는 학생만 줄을 세운다. 번호가 없는 학생은 밀 대상이 아니다.
    let mut numbered: Vec<(i32, i64)> = seats
        .iter()
        .filter_map(|s| s.class_no.map(|n| (n, s.student_id)))
        .collect();
    numbered.sort_unstable();

    if target.class_no == Some(new_no) {
        return Ok(Plan::none());
    }

    let taken = numbered
        .iter()
        .any(|(n, id)| *n == new_no && *id != student_id);

    // 빈 번호라면 이 학생만 바뀐다. 다른 학생을 건드릴 까닭이 없다.
    //
    // 번호가 겹치는 반에서도 이 길은 열어 둔다. 겹친 학생 하나를 빈 번호로 옮기는 것이
    // **중복을 푸는 방법**인데 여기서 막으면 빠져나갈 길이 없어진다.
    if !taken {
        return Ok(Plan {
            kind: Kind::Assign,
            moves: vec![Move {
                student_id,
                from: target.class_no,
                to: new_no,
            }],
        });
    }

    // 여기부터는 다른 학생을 밀어야 한다. 번호가 겹쳐 있으면 누구를 밀지 정할 수 없다.
    if let Some(w) = numbered.windows(2).find(|w| w[0].0 == w[1].0) {
        return Err(Blocked::Duplicate(w[0].0));
    }

    let mut numbers: Vec<i32> = numbered.iter().map(|(n, _)| *n).collect();
    let mut order: Vec<i64> = numbered.iter().map(|(_, id)| *id).collect();

    let kind = if target.class_no.is_some() {
        // 옮기기 — 사람 수도 번호 수도 그대로다. 줄에서 빼서 그 자리에 다시 넣는다.
        let pos = position_of(&numbers, new_no);
        order.retain(|id| *id != student_id);
        order.insert(pos, student_id);
        Kind::Reorder
    } else {
        // 끼워 넣기 — 사람이 하나 늘었으니 번호도 뒤에 하나 늘린다.
        let next = numbers.last().copied().unwrap_or(MIN_NO - 1) + 1;
        let pos = position_of(&numbers, new_no);
        numbers.push(next);
        order.insert(pos, student_id);
        Kind::Insert
    };

    let before: std::collections::HashMap<i64, Option<i32>> =
        seats.iter().map(|s| (s.student_id, s.class_no)).collect();

    let moves: Vec<Move> = order
        .iter()
        .zip(numbers.iter())
        .filter_map(|(id, to)| {
            let from = before.get(id).copied().flatten();
            (from != Some(*to)).then_some(Move {
                student_id: *id,
                from,
                to: *to,
            })
        })
        .collect();

    Ok(Plan { kind, moves })
}

fn position_of(numbers: &[i32], no: i32) -> usize {
    numbers.iter().position(|n| *n == no).unwrap_or(numbers.len())
}

/// 이 반의 현재 번호 상태를 나타내는 짧은 글.
///
/// 미리보기를 만든 뒤 적용하기 전에 명단이 바뀌지 않았는지 확인하는 데 쓴다.
/// 오래된 미리보기를 그대로 적용하면 엉뚱한 학생의 번호가 바뀐다.
pub fn state_key(seats: &[Seat]) -> String {
    let mut pairs: Vec<(i64, Option<i32>)> =
        seats.iter().map(|s| (s.student_id, s.class_no)).collect();
    pairs.sort_unstable();

    // FNV-1a 64. 자료를 되살릴 목적이 아니라 '달라졌는지'만 보는 것이므로 이만하면 된다.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for (id, no) in &pairs {
        for b in id.to_le_bytes() {
            eat(b);
        }
        for b in no.unwrap_or(0).to_le_bytes() {
            eat(b);
        }
        eat(b';');
    }
    format!("{}-{:016x}", pairs.len(), h)
}

#[cfg(test)]
#[path = "renumber_tests.rs"]
mod renumber_tests;
