//! Вещи (§12.317): штуки со своим состоянием — прочностью и зарядами.
//!
//! Счёт сходится с вещами сам — это стережёт проверка в `Sim::tick` на каждом
//! тике каждого теста. Здесь проверяется другое: что **состояние переживает
//! дорогу**. Пропущенный переход не роняет ничего, он молча рождает на новом
//! месте свежую вещь вместо поношенной, поэтому у каждого перехода свой тест.

use super::*;

const CORE: &str = include_str!("../../assets/rulesets/core.yaml");

/// Комбинезон в схеме: индекс палитры с силой и прочностью.
const SUIT: usize = 1;

/// Коридор: склад в (5,1), шлюз в (6,1), один кот `a`. Комбинезон с силой и
/// прочностью 6, как в рулсете.
fn corridor() -> Sim {
    let mut sim = sim_from(&["########", "#a.....#", "########"]);
    sim.set_items(2);
    sim.set_capacity(1, 100);
    sim.force_tile(5, 1, 1);
    sim.set_gate(2, true);
    sim.force_tile(6, 1, 2);
    sim.set_force(SUIT, 1);
    sim.set_wear(SUIT, 6);
    sim
}

// --- рождение и трата -------------------------------------------------------

/// Куча вещи рождает вещи с полной прочностью; ресурс вещей не имеет.
#[test]
fn a_pile_of_things_is_born_fresh_and_a_resource_has_none() {
    let mut sim = corridor();
    sim.put_item(5, 1, SUIT, 2);
    sim.put_item(5, 1, 0, 10);
    sim.tick_n(1);
    assert_eq!(sim.wears_at(5, 1, SUIT), vec![6, 6]);
    assert!(sim.wears_of(0).is_empty(), "лом — ресурс, штук у него нет");
}

// --- переходы ---------------------------------------------------------------

/// Снятый комбинезон ложится со своей прочностью и надевается снова таким же —
/// лазейки §12.268 «снять и надеть» больше нет.
#[test]
fn a_suit_taken_off_and_put_on_again_keeps_its_wear() {
    let mut sim = corridor();
    sim.put_item(5, 1, SUIT, 1);
    sim.set_wears_at(5, 1, SUIT, &[2]);
    sim.tick_n(1);
    assert!(sim.set_outfit("a", SUIT, true), "приписка принята");
    sim.tick_n(20);
    assert_eq!(sim.wear_left("a", SUIT), Some(2), "надет поношенный");

    assert!(sim.set_outfit("a", SUIT, false));
    let (x, y) = sim.pos_of("a");
    assert_eq!(
        sim.wears_at(x, y, SUIT),
        vec![2],
        "снятый лёг со своей прочностью"
    );

    // Уборка увозит его на склад — через лапы, — и прочность едет с ним.
    sim.tick_n(30);
    assert_eq!(sim.wears_of(SUIT), vec![2], "после уборки он всё тот же");

    assert!(sim.set_outfit("a", SUIT, true));
    sim.tick_n(30);
    assert_eq!(sim.wear_left("a", SUIT), Some(2), "и надет снова таким же");
}

/// Из двух на складе надевают самый свежий, потрёпанный остаётся лежать.
#[test]
fn the_freshest_suit_is_put_on_first() {
    let mut sim = corridor();
    sim.put_item(5, 1, SUIT, 2);
    sim.set_wears_at(5, 1, SUIT, &[1, 4]);
    sim.tick_n(1);
    assert!(sim.set_outfit("a", SUIT, true));
    sim.tick_n(20);
    assert_eq!(sim.wear_left("a", SUIT), Some(4));
    assert_eq!(sim.wears_at(5, 1, SUIT), vec![1]);
}

/// Куча, выдавленная из пустоты, переезжает вместе со своими вещами.
#[test]
fn a_pile_pushed_out_of_a_void_keeps_its_wear() {
    let mut sim = corridor();
    sim.put_item(3, 1, SUIT, 2);
    sim.set_wears_at(3, 1, SUIT, &[1, 3]);
    sim.force_tile(3, 1, -1);
    sim.tick_n(1);
    assert_eq!(sim.item_at(3, 1, SUIT), 0, "из пустоты куча съехала");
    assert_eq!(sim.wears_of(SUIT), vec![1, 3], "вещи те же");
}

/// Мастерская в (3,1) и склад в (5,1), на станок уже довезли комбинезон с
/// двумя выходами под долгий разбор — отмена успеет до конца штуки.
fn shop_with_a_worn_suit() -> Sim {
    let mut sim = sim_from(&["########", "#a.....#", "########"]);
    sim.set_items(3);
    sim.set_capacity(1, 100);
    sim.force_tile(5, 1, 1);
    sim.set_shop(2, true);
    sim.force_tile(3, 1, 2);
    sim.set_wear(SUIT, 6);
    sim.put_item(5, 1, SUIT, 1);
    sim.set_wears_at(5, 1, SUIT, &[2]);
    let recipe = sim.set_recipe(10_000, &[(SUIT, 1)], &[(2, 1)], &[]);
    assert!(sim.start_craft(recipe, 1));
    for _ in 0..200 {
        sim.tick();
        if sim.craft_delivered().is_some_and(|n| n > 0) {
            return sim;
        }
    }
    panic!("комбинезон до станка не довезли");
}

/// Отменённый заказ мастерской возвращает завезённое таким, каким привезли.
#[test]
fn a_cancelled_order_returns_its_things_as_they_came() {
    let mut sim = shop_with_a_worn_suit();
    assert!(sim.cancel_craft(3, 1));
    assert_eq!(sim.wears_at(3, 1, SUIT), vec![2], "вернулся поношенным");
    sim.tick_n(1);
    assert_eq!(sim.wears_of(SUIT), vec![2], "и сверка его не подменила");
}

/// Снесённый станок роняет завезённое таким, каким привезли: это путь
/// системы (`spill_delivered`), а не фасада.
#[test]
fn a_razed_shop_drops_its_things_as_they_came() {
    let mut sim = shop_with_a_worn_suit();
    sim.force_tile(3, 1, 0);
    sim.tick_n(1);
    assert_eq!(sim.wears_of(SUIT), vec![2], "уронил поношенным");
}

/// Заявка на вылазку роняет груз под ноги (§12.38) — вместе с его состоянием.
#[test]
fn a_load_dropped_for_a_raid_keeps_its_wear() {
    let mut sim = corridor();
    sim.put_item(2, 1, SUIT, 1);
    sim.set_wears_at(2, 1, SUIT, &[3]);
    let mut carried = false;
    for _ in 0..60 {
        sim.tick();
        if sim.carrying_item_of("a") == Some(SUIT) {
            carried = true;
            break;
        }
    }
    assert!(carried, "уборка подняла комбинезон");
    let m = sim.set_risky_mission(1, 10, 0, 0, &[]);
    assert!(sim.launch(m, vec!["a".to_string()]));
    let (x, y) = sim.pos_of("a");
    assert_eq!(sim.wears_at(x, y, SUIT), vec![3], "лёг под ноги поношенным");
}

/// Износ снимается с самой вещи: после вылазки на коте и в мире одна и та же
/// прочность.
#[test]
fn a_raid_wears_the_thing_itself() {
    let mut sim = corridor();
    sim.put_item(5, 1, SUIT, 1);
    sim.tick_n(1);
    assert!(sim.set_outfit("a", SUIT, true));
    sim.tick_n(20);
    let m = sim.set_risky_mission(1, 10, 0, 0, &[]);
    assert!(sim.launch(m, vec!["a".to_string()]));
    sim.tick_n(40);
    assert_eq!(sim.wear_left("a", SUIT), Some(5));
    assert_eq!(sim.wears_of(SUIT), vec![5]);
}

// --- заряды аптечки ---------------------------------------------------------

/// Аптечка с зарядами: лечение снимает заряд, сама она остаётся на складе.
#[test]
fn a_medkit_spends_one_charge_per_patient() {
    const KIT: usize = 0;
    let mut sim = sim_from(&["#########", "#a.....b#", "#########"]);
    sim.set_health_rules(1000, 600, 1);
    sim.set_health("a", 100);
    sim.set_capacity(1, 20);
    sim.force_tile(4, 1, 1);
    sim.set_mends(KIT, 8);
    sim.set_uses(KIT, 3);
    sim.put_item(4, 1, KIT, 1);

    sim.tick_n(70);
    assert!(sim.health_of("a") > 100, "лечение идёт");
    assert_eq!(sim.item_total(KIT), 1, "аптечка на месте");
    assert_eq!(sim.charges_of(KIT), vec![2], "минус один заряд");
}

/// Заряд снимают с самой потраченной: початые не копятся, а опустевшая
/// исчезает.
#[test]
fn the_most_spent_medkit_is_used_first_and_vanishes_when_empty() {
    const KIT: usize = 0;
    let mut sim = sim_from(&["#########", "#a.....b#", "#########"]);
    sim.set_health_rules(1000, 600, 1);
    sim.set_health("a", 100);
    sim.set_capacity(1, 20);
    sim.force_tile(4, 1, 1);
    sim.set_mends(KIT, 8);
    sim.set_uses(KIT, 3);
    sim.put_item(4, 1, KIT, 2);
    sim.set_charges_at(4, 1, KIT, &[1, 3]);

    sim.tick_n(70);
    assert_eq!(sim.item_total(KIT), 1, "пустая ушла со склада");
    assert_eq!(sim.charges_of(KIT), vec![3], "полная не тронута");
}

// --- сохранение -------------------------------------------------------------

/// Прочность и заряды переживают снимок: вещи едут своими сущностями.
#[test]
fn wear_and_charges_survive_a_save() {
    let mut live = Sim::new(CORE).expect("рулсет");
    live.add_storage();
    let suit = live.item_index("suit").expect("комбинезон");
    let kit = live.item_index("medkit").expect("аптечка");
    live.put_item(4, 3, suit, 2);
    live.set_wears_at(4, 3, suit, &[1, 5]);
    live.put_item(5, 3, kit, 2);
    live.set_charges_at(5, 3, kit, &[1, 2]);

    let json = live.save().expect("снимок");
    let mut loaded = Sim::load_from(CORE, &json).expect("загрузка");
    assert_eq!(loaded.wears_of(suit), vec![1, 5]);
    assert_eq!(loaded.charges_of(kit), vec![1, 2]);
}

// --- боевой рулсет ----------------------------------------------------------

/// Заряды бывают только у того, что тратится применением: у аптечки они
/// есть, а у вещи без `mends` заряды не значили бы ничего.
#[test]
fn the_shipped_ruleset_charges_its_medkit() {
    let rs: crate::ruleset::Ruleset = serde_yaml::from_str(CORE).expect("рулсет");
    let kit = rs.items.iter().find(|i| i.id == "medkit").expect("аптечка");
    assert!(kit.uses > 1, "аптечка лечит больше одного раненого");
    for item in rs.items.iter().filter(|i| i.uses > 0) {
        assert!(
            item.mends > 0,
            "заряды у «{}», которое ничем не тратится",
            item.id
        );
    }
}
