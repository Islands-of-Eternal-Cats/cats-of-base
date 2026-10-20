//! Место сбора: отряд садится за стол и, когда сели все, идёт к ближайшему
//! шлюзу (§12.279).
//!
//! Остальные тесты вылазок ставят тайл, который одновременно и стол, и дверь
//! (`set_gate`), — то есть гараж до §12.279. Здесь они разведены: стол — штамп
//! 3×3 со столешницей посередине, а шлюзов два, на разном расстоянии от него.

use super::*;

const SEAT: i16 = 1;
const TABLE: i16 = 2;
const EXIT: i16 = 3;

/// Ближний к столу шлюз — `(1, 4)`, дальний — `(9, 1)`.
const NEAR: (i32, i32) = (1, 4);
const FAR: (i32, i32) = (9, 1);

fn squad(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

/// Комната с местом сбора якорем в `(2, 1)` и двумя шлюзами. Коты стоят у
/// дальнего края, чтобы рассадка была видна: до стола им идти, а не стоять.
fn hall() -> Sim {
    let mut sim = sim_from(&[
        "###########",
        "#.........#",
        "#.........#",
        "#.........#",
        "#......a.b#",
        "###########",
    ]);
    sim.set_muster(SEAT, true);
    sim.set_solid(TABLE, true);
    sim.set_exit(EXIT, true);
    let def = sim.set_structure(vec![
        vec![Some(SEAT), Some(SEAT), Some(SEAT)],
        vec![Some(SEAT), Some(TABLE), Some(SEAT)],
        vec![Some(SEAT), Some(SEAT), Some(SEAT)],
    ]);
    sim.force_structure(def, 2, 1);
    sim.force_tile(NEAR.0, NEAR.1, EXIT);
    sim.force_tile(FAR.0, FAR.1, EXIT);
    sim
}

/// Ушёл ли отряд целиком за `ticks` тиков. Ловится сам момент ухода:
/// вылазка короткая, и к концу ожидания отряд мог уже вернуться.
fn leaves_within(sim: &mut Sim, ticks: usize) -> bool {
    (0..ticks).any(|_| {
        sim.tick_n(1);
        sim.is_away("a") && sim.is_away("b")
    })
}

/// Места по `id`: первый садится на первую проходимую клетку стола по обходу
/// карты, второй — на вторую (`seat_of`).
const SEAT_A: (i32, i32) = (2, 1);
const SEAT_B: (i32, i32) = (3, 1);

/// Восемь стульев — это **один** отряд, а не восемь (§12.161): слот держит
/// объект целиком.
#[test]
fn a_table_with_eight_seats_is_one_squad() {
    let mut sim = hall();
    assert_eq!(
        sim.gate_count(),
        1,
        "мест сбора — одно, шлюзы в счёт не идут"
    );
}

/// Никто не уходит, пока не сели все: ушедший в момент, когда сосед ещё
/// шёл к стулу, — это отряд, который «дружно» не выходит.
#[test]
fn a_squad_sits_down_before_it_walks_to_the_door() {
    let mut sim = hall();
    let def = sim.set_mission(2, 30, &[]);
    assert!(sim.launch(def, squad(&["a", "b"])));

    let (mut a_sat, mut b_sat) = (false, false);
    for _ in 0..200 {
        sim.tick_n(1);
        a_sat |= sim.pos_of("a") == SEAT_A;
        b_sat |= sim.pos_of("b") == SEAT_B;
        if sim.is_away("a") || sim.is_away("b") {
            assert!(a_sat && b_sat, "ушли, не рассевшись: a {a_sat}, b {b_sat}");
            assert!(sim.is_away("a") && sim.is_away("b"), "уходят вместе");
            assert_eq!(sim.mission_exit(), Some(NEAR), "через ближний шлюз");
            return;
        }
    }
    panic!("отряд так и не ушёл");
}

/// Дверь не выбрана, пока отряд рассаживается: выбрать её раньше значило бы
/// звать к ней тех, кто ещё не сел.
#[test]
fn the_door_is_chosen_only_once_everyone_is_seated() {
    let mut sim = hall();
    let def = sim.set_mission(2, 30, &[]);
    assert!(sim.launch(def, squad(&["a", "b"])));
    sim.tick_n(1);
    assert_eq!(sim.mission_exit(), None, "все ещё в пути к столу");
}

/// Без единого шлюза севший отряд ждёт за столом: уйти ему некуда, а
/// распускать его не за что — стол на месте, и шлюз можно построить.
#[test]
fn without_a_door_the_squad_waits_at_the_table() {
    let mut sim = hall();
    sim.force_tile(NEAR.0, NEAR.1, 0);
    sim.force_tile(FAR.0, FAR.1, 0);
    let def = sim.set_mission(2, 30, &[]);
    assert!(sim.launch(def, squad(&["a", "b"])));

    sim.tick_n(200);
    assert!(!sim.is_away("a") && !sim.is_away("b"), "уйти некуда");
    assert_eq!(sim.pos_of("a"), SEAT_A);
    assert_eq!(sim.pos_of("b"), SEAT_B);
    assert_eq!(sim.mission_exit(), None);
    assert_eq!(sim.raid_count(), 1, "заявка жива");

    // Построили шлюз — отряд уходит сам, второго клика не нужно.
    sim.force_tile(FAR.0, FAR.1, EXIT);
    assert!(leaves_within(&mut sim, 100), "ушли через новый шлюз");
}

/// Снесённый по дороге шлюз заменяется следующим ближайшим: дверь не
/// приписана к отряду, и выбирать её снова — не отмена решения игрока.
#[test]
fn a_door_torn_down_on_the_way_is_replaced_by_the_next_nearest() {
    let mut sim = hall();
    let def = sim.set_mission(2, 30, &[]);
    assert!(sim.launch(def, squad(&["a", "b"])));
    for _ in 0..200 {
        sim.tick_n(1);
        if sim.mission_exit().is_some() {
            break;
        }
    }
    assert_eq!(sim.mission_exit(), Some(NEAR));

    sim.force_tile(NEAR.0, NEAR.1, 0);
    sim.tick_n(1);
    assert_eq!(sim.mission_exit(), Some(FAR), "взяли следующий");
    assert!(leaves_within(&mut sim, 100), "ушли через дальний");
}

/// Добыча ложится на тот шлюз, через который ушли, а не на стол: дверь —
/// единственное место контакта с миром (§12.22).
#[test]
fn loot_lands_on_the_door_the_squad_left_through() {
    let mut sim = hall();
    let def = sim.set_mission(2, 30, &[(0, 5)]);
    assert!(sim.launch(def, squad(&["a", "b"])));
    sim.tick_n(300);

    assert!(!sim.is_away("a"), "вернулись");
    assert_eq!(sim.raid_count(), 0);
    // Уборки в схеме нет — склада нет, — так что куча лежит, где упала.
    assert_eq!(sim.scrap_at(NEAR.0, NEAR.1), 5, "добыча у ближнего шлюза");
}

/// Снесённый стол распускает собирающийся отряд — как снесённый станок
/// уносит свой заказ (§12.96).
#[test]
fn a_torn_down_table_disbands_the_gathering_squad() {
    let mut sim = hall();
    let def = sim.set_mission(2, 30, &[]);
    assert!(sim.launch(def, squad(&["a", "b"])));
    sim.tick_n(1);

    for (x, y) in [
        (2, 1),
        (3, 1),
        (4, 1),
        (2, 2),
        (4, 2),
        (2, 3),
        (3, 3),
        (4, 3),
    ] {
        sim.force_tile(x, y, 0);
    }
    sim.tick_n(1);
    assert_eq!(sim.raid_count(), 0);
    assert!(!sim.in_squad("a") && !sim.in_squad("b"));
}

/// Брифинг (§12.280): севший отряд сидит за столом `brief` тиков и только
/// потом выбирает дверь. Дверь, выбранная раньше, звала бы к выходу посреди
/// разговора.
#[test]
fn a_seated_squad_sits_through_the_briefing() {
    let mut sim = hall();
    sim.set_brief(SEAT, 20);
    let def = sim.set_mission(2, 30, &[]);
    assert!(sim.launch(def, squad(&["a", "b"])));

    // Ждём, пока сядут все.
    let mut sat_at = None;
    for t in 0..200 {
        sim.tick_n(1);
        if sim.pos_of("a") == SEAT_A && sim.pos_of("b") == SEAT_B {
            sat_at = Some(t);
            break;
        }
    }
    assert!(sat_at.is_some(), "так и не сели");

    sim.tick_n(15);
    assert_eq!(sim.mission_exit(), None, "брифинг ещё идёт");
    assert_eq!(sim.pos_of("a"), SEAT_A, "и сидят на местах");

    sim.tick_n(10);
    assert_eq!(sim.mission_exit(), Some(NEAR), "отслушали — к шлюзу");
}
