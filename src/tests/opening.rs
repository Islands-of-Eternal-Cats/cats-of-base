//! Начало партии (§12.367): один кот у портала, шлюз за завалом, вылазки
//! открываются по одному глаголу за раз. Здесь — настоящий старт боевого
//! рулсета, без `open_base`.

use super::*;

const CORE: &str = include_str!("../../assets/rulesets/core.yaml");

fn start() -> Sim {
    let mut sim = Sim::new(CORE).expect("рулсет");
    sim.without_timeline();
    sim
}

/// Прокопать кратчайший путь к шлюзу: четыре клетки завала по `x = 5`.
fn dig(sim: &mut Sim) {
    assert!(sim.plan_demolish_rect(5, 6, 1, 4), "завал размечен на снос");
    let open = (0..3000).any(|_| {
        sim.tick_n(1);
        sim.exit_open()
    });
    assert!(open, "докопались до шлюза");
}

/// Дождаться, пока заказ выполнится и отряд вернётся.
fn done(sim: &mut Sim, def: usize, ticks: usize) -> bool {
    (0..ticks).any(|_| {
        sim.tick_n(1);
        sim.world.resource::<Raids>().0.contains(&def) && sim.mission_left().is_none()
    })
}

/// На старте — один кот, ни запаса, ни выхода: шлюз отрезан завалом.
#[test]
fn the_shipped_ruleset_starts_with_one_cat_behind_rubble() {
    let mut sim = start();
    assert_eq!(sim.unit_count(), 1, "кот один");
    assert!(sim.has_unit("excellent"));
    assert_eq!(sim.scrap_total(), 0, "запаса нет");
    assert!(!sim.exit_open(), "к шлюзу не дойти");
    let rubble = sim.tile_index("rubble").expect("завал в палитре");
    assert!(
        !sim.add_blueprint(8, 12, i32::from(rubble)),
        "завал не строится — его можно только разобрать"
    );
}

/// Завал даёт лом и открывает выход (§12.367).
#[test]
fn the_shipped_ruleset_digs_scrap_out_of_the_rubble() {
    let mut sim = start();
    dig(&mut sim);
    assert!(sim.scrap_total() > 0, "завал дал лом");
}

/// Сначала открыт только «Осмотреть вход»; «Пошарить» и «Исследовать …» —
/// после него, стол — после «Проходной».
#[test]
fn the_shipped_ruleset_opens_one_verb_at_a_time() {
    let mut sim = start();
    let id = |sim: &Sim, m: &str| sim.mission_index(m).unwrap_or_else(|| panic!("заказ {m}"));
    let (inspect, gatehouse, scavenge) = (
        id(&sim, "inspect"),
        id(&sim, "explore_gatehouse"),
        id(&sim, "scavenge"),
    );
    let muster = sim.structure_index("muster").expect("место брифинга");
    assert!(sim.raid_gates(inspect).unlocked);
    assert!(!sim.raid_gates(gatehouse).unlocked);
    assert!(!sim.raid_gates(scavenge).unlocked);
    assert!(!sim.structure_is_open(muster), "стола нет");

    dig(&mut sim);
    assert!(sim.scavenge(inspect, vec!["excellent".into()], -1));
    assert!(done(&mut sim, inspect, 2000), "вход осмотрен");
    assert!(sim.raid_gates(gatehouse).unlocked, "открылась проходная");
    assert!(sim.raid_gates(scavenge).unlocked, "и «Пошарить»");
    assert!(!sim.structure_is_open(muster), "стола ещё нет");

    assert!(sim.scavenge(gatehouse, vec!["excellent".into()], -1));
    assert!(done(&mut sim, gatehouse, 3000), "проходная исследована");
    assert!(sim.has_unit("sp2") && sim.has_unit("sp3"), "свои нашлись");
    assert_eq!(sim.unit_count(), 3);
    assert!(sim.structure_is_open(muster), "стол брифинга открылся");
}

/// Свободные коты ходят только рядом с базой (§12.367): дальний заказ — только
/// отрядом.
#[test]
fn the_shipped_ruleset_sends_free_cats_only_near_home() {
    let sim = start();
    let rules = sim.world.resource::<MissionRules>();
    for (i, r) in rules.0.iter().enumerate() {
        if !r.nearby && r.alone.is_none() {
            assert!(
                !r.free_may_go(&["excellent".into()]),
                "дальний заказ {i} пускает свободного кота"
            );
        }
    }
}

/// Разовые вылазки рядом с базой не ранят и не проваливаются: пока кот один,
/// лечить его некому (§12.37, §12.367).
#[test]
fn the_shipped_ruleset_keeps_the_first_raids_harmless() {
    let sim = start();
    let rules = sim.world.resource::<MissionRules>();
    let first: Vec<&MissionRule> = rules.0.iter().filter(|r| r.nearby && r.once).collect();
    assert!(!first.is_empty(), "разовых вылазок рядом нет");
    for r in first {
        assert_eq!(r.danger, 0, "разовая вылазка проваливается");
        assert_eq!(r.harm, 0, "разовая вылазка ранит");
        assert!(r.squad <= 1, "одному коту на разовую не пойти");
    }
}

/// Сквозной прогон начала: кот докапывается, осматривает вход и находит своих,
/// не упав от голода и усталости (§12.367, открытый вопрос «успевает ли»).
#[test]
fn the_shipped_ruleset_opening_fits_before_hunger() {
    let mut sim = start();
    let inspect = sim.mission_index("inspect").expect("осмотр");
    let gatehouse = sim.mission_index("explore_gatehouse").expect("проходная");
    dig(&mut sim);
    let dug = sim.world.resource::<SimTime>().tick;
    assert!(sim.scavenge(inspect, vec!["excellent".into()], -1));
    assert!(done(&mut sim, inspect, 3000));
    let inspected = sim.world.resource::<SimTime>().tick;
    assert!(sim.scavenge(gatehouse, vec!["excellent".into()], -1));
    assert!(done(&mut sim, gatehouse, 6000));
    let found = sim.world.resource::<SimTime>().tick;
    let (energy, fed) = (sim.energy_of("excellent"), sim.fed_of("excellent"));
    println!("прокоп {dug}, осмотр {inspected}, свои {found} · бодрость {energy} · сытость {fed}");
    assert!(sim.has_unit("sp2"), "свои нашлись");
}
