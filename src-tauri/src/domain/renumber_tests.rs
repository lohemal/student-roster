//! 번호 재정렬 계산 검사.
//!
//! 학생 번호는 사람이 눈으로 세는 값이라 한 칸만 어긋나도 바로 드러난다.
//! 그래서 경계(맨 앞·맨 뒤·바로 옆·같은 번호)를 모두 확인한다.

use super::*;

/// 1번부터 `n` 번까지 차례로 앉은 반. 학생 번호 = 학생 id 로 둬서 읽기 쉽게 한다.
fn straight(n: i32) -> Vec<Seat> {
    (1..=n).map(|i| Seat::new(i as i64, Some(i))).collect()
}

/// 계획을 `(학생, 이전, 이후)` 로 펴서 견주기 쉽게 만든다.
fn flat(p: &Plan) -> Vec<(i64, Option<i32>, i32)> {
    let mut v: Vec<(i64, Option<i32>, i32)> =
        p.moves.iter().map(|m| (m.student_id, m.from, m.to)).collect();
    v.sort_unstable();
    v
}

/// 계획대로 옮긴 뒤의 (학생, 번호) 표
fn applied(seats: &[Seat], p: &Plan) -> Vec<(i64, Option<i32>)> {
    let mut out: Vec<(i64, Option<i32>)> =
        seats.iter().map(|s| (s.student_id, s.class_no)).collect();
    for m in &p.moves {
        for row in out.iter_mut() {
            if row.0 == m.student_id {
                row.1 = Some(m.to);
            }
        }
    }
    out.sort_unstable();
    out
}

// ---------------------------------------------------------------
// 뒤 → 앞
// ---------------------------------------------------------------

#[test]
fn 스물세번을_칠번으로_옮기면_칠번부터_스물두번까지_한_칸씩_밀린다() {
    let seats = straight(23);
    let p = plan(&seats, 23, 7).unwrap();

    assert_eq!(p.kind, Kind::Reorder);
    // 대상 1명 + 밀린 16명
    assert_eq!(p.moves.len(), 17);
    assert_eq!(p.affected(23), 16);

    let after = applied(&seats, &p);
    assert_eq!(after[22], (23, Some(7)), "홍길동이 7번이 된다");
    for i in 7..=22i64 {
        assert_eq!(
            after[(i - 1) as usize],
            (i, Some(i as i32 + 1)),
            "{i}번 학생이 한 칸 밀린다"
        );
    }
    for i in 1..=6i64 {
        assert_eq!(after[(i - 1) as usize], (i, Some(i as i32)), "앞쪽은 그대로다");
    }
}

// ---------------------------------------------------------------
// 앞 → 뒤
// ---------------------------------------------------------------

#[test]
fn 칠번을_스물세번으로_옮기면_팔번부터_스물세번까지_한_칸씩_당겨진다() {
    let seats = straight(23);
    let p = plan(&seats, 7, 23).unwrap();

    assert_eq!(p.kind, Kind::Reorder);
    assert_eq!(p.affected(7), 16);

    let after = applied(&seats, &p);
    assert_eq!(after[6], (7, Some(23)));
    for i in 8..=23i64 {
        assert_eq!(after[(i - 1) as usize], (i, Some(i as i32 - 1)));
    }
    for i in 1..=6i64 {
        assert_eq!(after[(i - 1) as usize], (i, Some(i as i32)));
    }
}

// ---------------------------------------------------------------
// 바로 옆 · 같은 번호
// ---------------------------------------------------------------

#[test]
fn 바로_옆으로_옮기면_두_학생만_자리를_바꾼다() {
    let seats = straight(23);
    let p = plan(&seats, 7, 8).unwrap();
    assert_eq!(flat(&p), vec![(7, Some(7), 8), (8, Some(8), 7)]);

    // 반대로 눌러도 같은 결과여야 한다
    let q = plan(&seats, 8, 7).unwrap();
    assert_eq!(flat(&q), flat(&p));
}

#[test]
fn 같은_번호로_바꾸면_아무_일도_없다() {
    let p = plan(&straight(23), 7, 7).unwrap();
    assert_eq!(p.kind, Kind::None);
    assert!(p.moves.is_empty());
}

#[test]
fn 맨_앞과_맨_뒤로도_옮길_수_있다() {
    let seats = straight(5);

    let first = plan(&seats, 5, 1).unwrap();
    assert_eq!(
        flat(&first),
        vec![
            (1, Some(1), 2),
            (2, Some(2), 3),
            (3, Some(3), 4),
            (4, Some(4), 5),
            (5, Some(5), 1),
        ]
    );

    let last = plan(&seats, 1, 5).unwrap();
    assert_eq!(
        flat(&last),
        vec![
            (1, Some(1), 5),
            (2, Some(2), 1),
            (3, Some(3), 2),
            (4, Some(4), 3),
            (5, Some(5), 4),
        ]
    );
}

// ---------------------------------------------------------------
// 띄엄띄엄한 번호
// ---------------------------------------------------------------

#[test]
fn 띄엄띄엄한_번호를_일번부터_다시_매기지_않는다() {
    // 1,2,3,5,6,9 — 4번과 7·8번이 비어 있는 반
    let seats = vec![
        Seat::new(1, Some(1)),
        Seat::new(2, Some(2)),
        Seat::new(3, Some(3)),
        Seat::new(4, Some(5)),
        Seat::new(5, Some(6)),
        Seat::new(6, Some(9)),
    ];
    let p = plan(&seats, 6, 3).unwrap();

    let after = applied(&seats, &p);
    assert_eq!(
        after,
        vec![
            (1, Some(1)),
            (2, Some(2)),
            (3, Some(5)), // 3번이던 학생이 다음 번호인 5번으로
            (4, Some(6)),
            (5, Some(9)),
            (6, Some(3)), // 옮긴 학생이 3번
        ]
    );

    // 쓰이는 번호의 집합은 그대로다 — 없던 4·7·8번이 생기지 않는다
    let mut before: Vec<i32> = seats.iter().filter_map(|s| s.class_no).collect();
    let mut now: Vec<i32> = after.iter().filter_map(|s| s.1).collect();
    before.sort_unstable();
    now.sort_unstable();
    assert_eq!(before, now);
}

#[test]
fn 비어_있는_번호로_옮기면_그_학생만_바뀐다() {
    // 4번이 비어 있다
    let seats = vec![
        Seat::new(1, Some(1)),
        Seat::new(2, Some(2)),
        Seat::new(3, Some(3)),
        Seat::new(4, Some(5)),
    ];
    let p = plan(&seats, 4, 4).unwrap();

    assert_eq!(p.kind, Kind::Assign);
    assert_eq!(flat(&p), vec![(4, Some(5), 4)]);
    assert_eq!(p.affected(4), 0, "다른 학생은 건드리지 않는다");
}

// ---------------------------------------------------------------
// 번호 없는 학생
// ---------------------------------------------------------------

#[test]
fn 번호가_없는_학생은_밀리지_않는다() {
    let mut seats = straight(5);
    seats.push(Seat::new(99, None));

    let p = plan(&seats, 5, 2).unwrap();
    assert!(
        p.moves.iter().all(|m| m.student_id != 99),
        "번호가 없는 학생은 계획에 들어오지 않는다"
    );
}

#[test]
fn 번호가_없는_학생에게_빈_번호를_주면_그_학생만_바뀐다() {
    let mut seats = straight(3);
    seats.push(Seat::new(99, None));

    let p = plan(&seats, 99, 4).unwrap();
    assert_eq!(p.kind, Kind::Assign);
    assert_eq!(flat(&p), vec![(99, None, 4)]);
}

#[test]
fn 번호가_없는_학생을_쓰는_번호에_넣으면_뒤가_한_칸씩_밀린다() {
    let mut seats = straight(5);
    seats.push(Seat::new(99, None));

    let p = plan(&seats, 99, 3).unwrap();
    assert_eq!(p.kind, Kind::Insert);

    let after = applied(&seats, &p);
    assert_eq!(
        after,
        vec![
            (1, Some(1)),
            (2, Some(2)),
            (3, Some(4)),
            (4, Some(5)),
            (5, Some(6)), // 반에 번호 하나가 늘어난다
            (99, Some(3)),
        ]
    );
}

// ---------------------------------------------------------------
// 막아야 하는 것
// ---------------------------------------------------------------

#[test]
fn 번호가_겹치는_반에서는_자동_이동을_하지_않는다() {
    let seats = vec![
        Seat::new(1, Some(1)),
        Seat::new(2, Some(7)),
        Seat::new(3, Some(7)), // 중복
        Seat::new(4, Some(9)),
    ];
    let err = plan(&seats, 4, 1).unwrap_err();
    assert_eq!(err, Blocked::Duplicate(7));

    let msg = err.message("3-나리");
    assert!(msg.contains("3-나리"), "어느 반인지 알려 준다");
    assert!(msg.contains('7'), "몇 번이 겹치는지 알려 준다");
}

#[test]
fn 쓸_수_없는_번호는_거절한다() {
    let seats = straight(5);
    for bad in [0, -3, MAX_NO + 1, 9999] {
        assert_eq!(
            plan(&seats, 3, bad).unwrap_err(),
            Blocked::BadNumber(bad),
            "{bad} 는 번호로 쓸 수 없다"
        );
    }
}

#[test]
fn 그_반에_없는_학생은_옮기지_않는다() {
    assert_eq!(
        plan(&straight(5), 777, 2).unwrap_err(),
        Blocked::NotInClass
    );
}

#[test]
fn 혼자인_반도_계산된다() {
    let seats = vec![Seat::new(1, Some(5))];
    let p = plan(&seats, 1, 1).unwrap();
    assert_eq!(flat(&p), vec![(1, Some(5), 1)]);
}

// ---------------------------------------------------------------
// 계획이 지켜야 하는 성질
// ---------------------------------------------------------------

#[test]
fn 옮긴_뒤에도_번호가_겹치지_않는다() {
    let seats = straight(12);
    for target in 1..=12i64 {
        for to in 1..=12i32 {
            let p = plan(&seats, target, to).unwrap();
            let after = applied(&seats, &p);
            let mut nums: Vec<i32> = after.iter().filter_map(|r| r.1).collect();
            nums.sort_unstable();
            let before = nums.len();
            nums.dedup();
            assert_eq!(nums.len(), before, "{target} → {to} 에서 번호가 겹쳤다");

            let me = after.iter().find(|r| r.0 == target).unwrap();
            assert_eq!(me.1, Some(to), "{target} 은 {to} 번이 되어야 한다");
        }
    }
}

#[test]
fn 계획에는_실제로_바뀌는_학생만_들어간다() {
    let p = plan(&straight(20), 20, 18).unwrap();
    assert!(
        p.moves.iter().all(|m| m.from != Some(m.to)),
        "번호가 그대로인 학생은 계획에 넣지 않는다"
    );
    assert_eq!(p.affected(20), 2, "18·19번만 밀린다");
}

// ---------------------------------------------------------------
// 상태 확인용 글
// ---------------------------------------------------------------

#[test]
fn 같은_반이면_상태_글도_같다() {
    let a = straight(10);
    let mut b = straight(10);
    b.reverse(); // 순서만 다를 뿐 같은 반이다
    assert_eq!(state_key(&a), state_key(&b));
}

#[test]
fn 번호가_바뀌면_상태_글도_바뀐다() {
    let before = straight(10);
    let mut after = straight(10);
    after[3].class_no = Some(11);
    assert_ne!(state_key(&before), state_key(&after));

    // 학생이 늘거나 줄어도 달라진다
    let mut added = straight(10);
    added.push(Seat::new(99, None));
    assert_ne!(state_key(&before), state_key(&added));

    let mut removed = straight(10);
    removed.pop();
    assert_ne!(state_key(&before), state_key(&removed));
}

#[test]
fn 번호가_겹쳐도_빈_번호로_비켜서는_것은_막지_않는다() {
    // 7번이 둘인 반. 한쪽을 빈 번호로 옮기는 것이 곧 중복을 푸는 방법이다.
    let seats = vec![
        Seat::new(1, Some(1)),
        Seat::new(2, Some(7)),
        Seat::new(3, Some(7)),
        Seat::new(4, Some(9)),
    ];
    let p = plan(&seats, 3, 4).expect("빈 번호로 비켜서는 것은 막지 않는다");
    assert_eq!(p.kind, Kind::Assign);
    assert_eq!(flat(&p), vec![(3, Some(7), 4)]);
    assert_eq!(p.affected(3), 0, "다른 학생은 그대로다");

    // 반면 남이 쓰는 번호로 가려면 누구를 밀지 알 수 없어 막는다
    assert_eq!(plan(&seats, 4, 1).unwrap_err(), Blocked::Duplicate(7));
}
