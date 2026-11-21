//! Общая вылазка нескольких отрядов (§12.364): выбранные в штабе отряды уходят
//! одной вылазкой — состав общий, слоты держатся у каждого стола.

use super::*;

/// Два стола-шлюза и три кота: `a` и `b` — первый отряд, `c` — второй.
fn two_squads() -> Sim {
    let mut sim = sim_from(&["#######", "#a.b.c#", "#.....#", "#######"]);
    sim.set_gate(1, true);
    sim.force_tile(1, 2, 1);
    sim.force_tile(4, 2, 1);
    assert!(sim.enlist("a", 1, 2) && sim.enlist("b", 1, 2));
    assert!(sim.enlist("c", 4, 2));
    sim
}

/// Два отряда — одна вылазка: все трое в одном составе, слоты заняты оба.
#[test]
fn two_squads_leave_as_one_raid() {
    let mut sim = two_squads();
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    let other = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 3);
    assert!(
        sim.launch_nodes(def, vec![1, 2, 4, 2], vec![], -1),
        "заявка принята"
    );
    assert_eq!(sim.raid_count(), 1, "вылазка одна");
    assert!(!sim.launch_node(other, 4, 2, -1), "второй стол тоже занят");
    let gone = (0..200).any(|_| {
        sim.tick_n(1);
        sim.is_away("a") && sim.is_away("b") && sim.is_away("c")
    });
    assert!(gone, "ушли все трое разом");
}

/// Предел заказа меряет объединённый состав.
#[test]
fn the_limit_counts_both_squads() {
    let mut sim = two_squads();
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 2);
    assert!(
        !sim.launch_nodes(def, vec![1, 2, 4, 2], vec![], -1),
        "трое при пределе в двоих не уходят"
    );
    assert_eq!(sim.raid_count(), 0);
}

/// Отозвать общую вылазку можно с любого её стола — отзывается вся.
#[test]
fn a_joined_table_recalls_the_whole_raid() {
    let mut sim = two_squads();
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 3);
    assert!(sim.launch_nodes(def, vec![1, 2, 4, 2], vec![], -1));
    assert!(sim.cancel_mission_at(4, 2), "отзыв со второго стола");
    assert_eq!(sim.raid_count(), 0);
    assert!(!sim.in_squad("a") && !sim.in_squad("c"));
}

/// Вернулись — свободны оба стола.
#[test]
fn coming_home_frees_both_tables() {
    let mut sim = two_squads();
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    let other = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 3);
    assert!(sim.launch_nodes(def, vec![1, 2, 4, 2], vec![], -1));
    let home = (0..400).any(|_| {
        sim.tick_n(1);
        sim.raid_count() == 0
    });
    assert!(home, "вылазка вернулась");
    assert!(
        sim.launch_node(other, 4, 2, -1),
        "второй стол снова свободен"
    );
}

/// Прогноз общей вылазки — по объединённому составу.
#[test]
fn the_forecast_sums_the_picked_squads() {
    let mut sim = two_squads();
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 3);
    assert!(sim.combo_snap().is_none(), "ничего не выбрано");
    sim.set_squad_pick(vec![1, 2, 4, 2]);
    let combo = sim.combo_snap().expect("выбрано два отряда");
    assert_eq!(combo.crew, vec!["a", "b", "c"]);
    assert_eq!(combo.spans[def], sim.mission_span_of(def, 3));
}

/// Отряд и свободные коты — одной вылазкой (§12.365): свободный идёт с
/// отрядом, но к столу не приписывается.
#[test]
fn a_squad_takes_free_cats_along() {
    let mut sim = two_squads();
    assert!(sim.dismiss("c"), "c свободен");
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 3);
    sim.set_squad_pick(vec![1, 2]);
    sim.set_nearby_pick(vec!["c".into()]);
    let combo = sim.combo_snap().expect("отряд и свободный");
    assert_eq!(combo.crew, vec!["a", "b", "c"]);
    assert!(sim.launch_nodes(def, vec![1, 2], vec!["c".into()], -1));
    assert_eq!(sim.raid_count(), 1);
    assert!(sim.in_squad("c"), "свободный в составе");
    let home = (0..400).any(|_| {
        sim.tick_n(1);
        sim.raid_count() == 0
    });
    assert!(home, "вернулись");
    assert_eq!(sim.enlisted_at("c"), None, "и остался свободным");
}

/// Предел заказа считает и свободных; занятого свободным не назовёшь.
#[test]
fn free_cats_count_toward_the_limit() {
    let mut sim = two_squads();
    assert!(sim.dismiss("c"));
    let def = sim.set_mission(1, 30, &[(0, 5)]);
    sim.set_squad_range(def, 1, 2);
    assert!(
        !sim.launch_nodes(def, vec![1, 2], vec!["c".into()], -1),
        "двое отряда и свободный при пределе в двоих"
    );
    assert!(
        !sim.launch_nodes(def, vec![1, 2], vec!["a".into()], -1),
        "кот из отряда — не свободный"
    );
}
