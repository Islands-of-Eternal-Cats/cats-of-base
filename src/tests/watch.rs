//! Сторож (§12.462): тревоги справа — склад полон и лежанки нет.
//!
//! Сторож только смотрит, поэтому тесты гоняют полную цепочку и спрашивают
//! `Sim::watch` — то же, что уезжает в снимок.

use super::sim_from;
use crate::sim::Sim;

const CORRIDOR: [&str; 3] = ["#########", "#a......#", "#########"];
const PAIR: [&str; 3] = ["#########", "#ab.....#", "#########"];

/// Тревоги как их видит вид: тег, счёт и коты.
fn alarms(sim: &mut Sim) -> Vec<(String, i32, Vec<String>)> {
    sim.watch()
        .into_iter()
        .map(|w| (w.kind, w.count, w.units))
        .collect()
}

fn kinds(sim: &mut Sim) -> Vec<String> {
    alarms(sim).into_iter().map(|(k, ..)| k).collect()
}

// --- склад полон -----------------------------------------------------------

/// Склад набит, а на полу лежит то, что уборка хочет увезти: тревога со
/// счётом того, что ждёт места.
#[test]
fn a_full_store_raises_the_alarm_with_what_is_left() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_capacity(1, 3);
    sim.force_tile(7, 1, 1);
    sim.put_scrap(2, 1, 5);

    sim.tick_n(300);
    assert_eq!(sim.scrap_at(7, 1), 3, "склад набит");
    let got = alarms(&mut sim);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, "store_full");
    assert_eq!(got[0].1, 2, "ждут места две штуки");
}

/// Место на складе есть — коты увезут сами, и сторож молчит с первого тика.
#[test]
fn a_store_with_room_keeps_quiet() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_capacity(1, 20);
    sim.force_tile(7, 1, 1);
    sim.put_scrap(2, 1, 5);

    sim.tick_n(1);
    assert!(kinds(&mut sim).is_empty());
}

/// Склада нет вовсе — это начало партии, а не беда: сторож не зовёт строить.
#[test]
fn no_store_at_all_is_not_a_full_store() {
    let mut sim = sim_from(&CORRIDOR);
    sim.put_scrap(2, 1, 5);

    sim.tick_n(5);
    assert!(kinds(&mut sim).is_empty());
}

/// Выключенная автоуборка ничего не помечает: игрок не просил убирать, и
/// склад, полный под завязку, ничьего решения не держит.
#[test]
fn a_full_store_without_tidying_is_not_an_alarm() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_auto_tidy(false);
    sim.set_capacity(1, 3);
    sim.force_tile(7, 1, 1);
    sim.put_scrap(7, 1, 3);
    sim.put_scrap(2, 1, 5);

    sim.tick_n(5);
    assert!(kinds(&mut sim).is_empty());
}

// --- лежанки нет -----------------------------------------------------------

/// Устал, а лечь некуда: на базе нет ни одной лежанки.
#[test]
fn a_tired_cat_without_beds_raises_no_bed() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_needs(1000, 300, 1);
    sim.set_energy("a", 200);

    sim.tick_n(2);
    let got = alarms(&mut sim);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, "no_bed");
    assert_eq!(got[0].2, vec!["a".to_string()]);
}

/// Свалившийся на пол числится тоже — до самого пробуждения.
#[test]
fn a_cat_collapsed_on_the_floor_is_named() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_needs(1000, 300, 1);
    sim.set_energy("a", 1);

    sim.tick_n(3);
    assert!(sim.is_resting("a"), "упал и спит");
    let got = alarms(&mut sim);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].2, vec!["a".to_string()]);
}

/// Лежанка есть, но её держит сосед: тревога другая — чинится она тоже
/// стройкой, но говорит «мало», а не «нет».
#[test]
fn a_taken_bed_raises_beds_taken() {
    let mut sim = sim_from(&PAIR);
    sim.set_rest(1, 5);
    sim.force_tile(2, 1, 1); // лежанка под «b»
    sim.set_needs(1000, 300, 1);
    sim.set_energy("a", 900);
    sim.set_energy("b", 200);

    sim.tick_n(2);
    assert!(sim.is_resting("b"), "«b» лёг");
    assert!(kinds(&mut sim).is_empty(), "«a» бодр, тревоги нет");

    sim.set_energy("a", 200);
    sim.tick_n(2);
    let got = alarms(&mut sim);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, "beds_taken");
    assert_eq!(got[0].2, vec!["a".to_string()]);
}

/// Свободная лежанка есть — раздатчик уложит сам, сторож молчит.
#[test]
fn a_free_bed_keeps_quiet() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_rest(1, 5);
    sim.force_tile(7, 1, 1);
    sim.set_needs(1000, 300, 1);
    sim.set_energy("a", 200);

    sim.tick_n(1);
    assert!(kinds(&mut sim).is_empty());
}

/// Бодрому лежанка не нужна: «лежанок нет» без уставших — не тревога.
#[test]
fn a_rested_cat_needs_no_bed() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_needs(1000, 300, 1);
    sim.set_energy("a", 900);

    sim.tick_n(2);
    assert!(kinds(&mut sim).is_empty());
}

// --- лаборатории нет ---------------------------------------------------------

/// Лаборатория в палитре (тайл 1), но не построена; тема готова ко всему,
/// кроме места. Вернёт мир и индекс темы.
fn sim_waiting_lab() -> (Sim, usize) {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_lab(1, true);
    let topic = sim.set_topic("materials", 0, 200, &[], &[]);
    (sim, topic)
}

/// Темы ждут, а лаборатории нет: тревога называет, какие.
#[test]
fn a_waiting_topic_without_a_lab_raises_no_lab() {
    let (mut sim, topic) = sim_waiting_lab();
    sim.tick_n(1);
    let got: Vec<_> = sim.watch().into_iter().map(|w| (w.kind, w.defs)).collect();
    assert_eq!(got, vec![("no_lab".to_string(), vec![topic])]);
}

/// Лаборатория стоит — тема пойдёт обычным путём, сторож молчит.
#[test]
fn a_built_lab_keeps_quiet() {
    let (mut sim, _) = sim_waiting_lab();
    sim.force_tile(7, 1, 1);
    sim.tick_n(1);
    assert!(kinds(&mut sim).is_empty());
}

/// Лаборатория ещё закрыта наукой — сторож не называет постройку, которой
/// нет в палитре (§12.126).
#[test]
fn a_lab_not_yet_in_the_palette_is_not_named() {
    let (mut sim, _) = sim_waiting_lab();
    sim.set_tile_tech(1, "labs");
    sim.tick_n(1);
    assert!(kinds(&mut sim).is_empty());

    sim.set_tech("labs");
    sim.tick_n(1);
    assert_eq!(kinds(&mut sim), vec!["no_lab".to_string()]);
}

/// Тема, закрытая технологией, ещё не тема: ждать ей нечего.
#[test]
fn a_locked_topic_does_not_ask_for_a_lab() {
    let mut sim = sim_from(&CORRIDOR);
    sim.set_lab(1, true);
    sim.set_topic("deep", 0, 200, &[], &["basics"]);
    sim.tick_n(1);
    assert!(kinds(&mut sim).is_empty());
}

/// Изученная тема лабораторию не ждёт.
#[test]
fn a_learned_topic_does_not_ask_for_a_lab() {
    let (mut sim, _) = sim_waiting_lab();
    sim.set_tech("materials");
    sim.tick_n(1);
    assert!(kinds(&mut sim).is_empty());
}
