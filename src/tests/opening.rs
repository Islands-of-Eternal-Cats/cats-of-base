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

/// Разобрать путь к сломанному шлюзу (§12.372, §12.376): выход из правой
/// верхней комнатки и коридор по `y = 3`, затем шахта по `x = 11` до верха.
/// Кончается, когда верх шахты разобран и к руине можно подойти.
fn dig_to_ruin(sim: &mut Sim) {
    assert!(
        sim.plan_demolish_rect(12, 3, 3, 1),
        "коридор размечен на разбор"
    );
    assert!(
        sim.plan_demolish_rect(11, 1, 1, 3),
        "шахта размечена на разбор"
    );
    let rubble = sim.tile_index("rubble").expect("завал");
    let dug = (0..3000).any(|_| {
        sim.tick_n(1);
        sim.tile(11, 1) != rubble
    });
    assert!(dug, "докопались до сломанного шлюза");
}

/// Ждать, пока на клетке не встанет тайл `id`.
fn until_tile(sim: &mut Sim, x: i32, y: i32, id: &str, ticks: usize) -> bool {
    let t = sim.tile_index(id).expect("тайл в палитре");
    (0..ticks).any(|_| {
        sim.tick_n(1);
        sim.tile(x, y) == t
    })
}

/// Докопаться, обследовать руину и починить шлюз (§12.376, §12.379): выход
/// открыт, когда шлюз достроен.
fn dig(sim: &mut Sim) {
    dig_to_ruin(sim);
    assert!(sim.fix_at(11, 0), "руина размечена на обследование");
    assert!(
        until_tile(sim, 11, 0, "gate_ruin", 3000),
        "руину обследовали"
    );
    assert!(sim.fix_at(11, 0), "шлюз размечен на ремонт");
    let open = (0..3000).any(|_| {
        sim.tick_n(1);
        sim.exit_open()
    });
    assert!(open, "шлюз починен, выход открыт");
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
        !sim.add_blueprint(5, 4, i32::from(rubble)),
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

/// Сначала открыт только «Осмотреть вход»; «Исследовать …» — после него,
/// «Пошарить» и стол — после «Исследовать местность вокруг шлюза» (§12.377).
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
    assert!(
        sim.raid_gates(gatehouse).unlocked,
        "открылась местность вокруг шлюза"
    );
    assert!(!sim.raid_gates(scavenge).unlocked, "«Пошарить» ещё нет");
    assert!(!sim.structure_is_open(muster), "стола ещё нет");

    assert!(sim.scavenge(gatehouse, vec!["excellent".into()], -1));
    assert!(
        done(&mut sim, gatehouse, 3000),
        "местность вокруг шлюза исследована"
    );
    assert!(sim.has_unit("sp2") && sim.has_unit("sp3"), "свои нашлись");
    assert_eq!(sim.unit_count(), 3);
    assert!(sim.structure_is_open(muster), "стол брифинга открылся");
    assert!(sim.raid_gates(scavenge).unlocked, "открылось «Пошарить»");
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
    let gatehouse = sim
        .mission_index("explore_gatehouse")
        .expect("местность вокруг шлюза");
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

/// Стартовая база — шахта к шлюзу и шесть комнаток 3×3 по её сторонам, всё
/// под завалом (§12.372): свободна только клетка портала в правой верхней
/// комнатке, все восемь соседей кота завалены. Шагнуть ему некуда — он
/// «застрял», и это законное состояние (инвариант 6): разбирает он с места, и
/// первая же разобранная клетка его выпускает.
#[test]
fn the_shipped_ruleset_starts_in_one_rubble_room() {
    let mut sim = start();
    let rubble = sim.tile_index("rubble").expect("завал");
    let floor = sim.tile_index("floor").expect("пол");
    assert_eq!(
        sim.pos_of("excellent"),
        (15, 3),
        "кот в правой верхней комнатке"
    );
    sim.tick_n(1);
    assert!(sim.stuck_of("excellent"), "вокруг кота сплошной завал");
    assert_eq!(sim.tile(15, 3), floor, "клетка портала свободна");
    let mut cells: Vec<(i32, i32)> = (1..15).map(|y| (11, y)).collect();
    for left in [6, 14] {
        for top in [2, 7, 12] {
            cells.extend((left..left + 3).flat_map(|x| (top..top + 3).map(move |y| (x, y))));
        }
    }
    for row in [3, 8, 13] {
        cells.extend([(9, row), (10, row), (12, row), (13, row)]);
    }
    for (x, y) in cells {
        if (x, y) != (15, 3) {
            assert_eq!(sim.tile(x, y), rubble, "завалено: ({x}, {y})");
        }
    }
    let (w, h) = {
        let map = sim.world.resource::<BaseMap>();
        (map.width, map.height)
    };
    let piles = (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|&(x, y)| sim.tile(x, y) == rubble)
        .count();
    assert_eq!(piles, 79);
    let ruin = sim.tile_index("broken_gate").expect("сломанный шлюз");
    assert_eq!(sim.tile(11, 0), ruin, "над шахтой — сломанный шлюз");
    assert!(
        sim.tile_index("portal").is_none(),
        "портал — картинка, а не тайл"
    );

    // Разбирает он стоя на месте, и первая же клетка снимает «застрял».
    assert!(sim.plan_demolish_rect(14, 3, 1, 1));
    let freed = (0..500).any(|_| {
        sim.tick_n(1);
        !sim.stuck_of("excellent")
    });
    assert!(freed, "кот так и не выбрался из завала");
}

/// На старте не строится ничего (§12.372): шлюз открывает прокоп до руины
/// (§12.376), а пол, склад и лежанка — выполненный «Осмотреть вход».
#[test]
fn the_shipped_ruleset_builds_nothing_before_the_entrance_is_inspected() {
    let mut sim = start();
    let palette: Vec<usize> = {
        let rules = sim.world.resource::<TileRules>();
        (0..rules.0.len())
            .filter(|&t| !rules.is_internal(t as i16))
            .collect()
    };
    let open: Vec<usize> = palette
        .into_iter()
        .filter(|&t| sim.tile_is_open(t))
        .collect();
    assert!(open.is_empty(), "на старте открыты постройки {open:?}");

    let inspect = sim.mission_index("inspect").expect("осмотр");
    dig(&mut sim);
    assert!(sim.scavenge(inspect, vec!["excellent".into()], -1));
    assert!(done(&mut sim, inspect, 3000), "вход осмотрен");
    for id in ["floor", "storage", "bed"] {
        let t = sim.tile_index(id).expect("тайл в палитре");
        assert!(sim.tile_is_open(t as usize), "`{id}` не открылся осмотром");
    }
}

/// Что нельзя построить, того нельзя и разобрать (§12.372): на старте ластик
/// берёт только завал — пол комнаты и единственный шлюз ему не даются, иначе
/// партия запиралась бы одним мазком.
#[test]
fn the_shipped_ruleset_dismantles_only_rubble_at_the_start() {
    let mut sim = start();
    assert!(!sim.add_blueprint(15, 3, -1), "пол у портала разобран");
    assert!(!sim.add_blueprint(11, 0, -1), "сломанный шлюз разобран");
    assert!(sim.add_blueprint(6, 2, -1), "завал не разбирается");
    let mask = sim.buildable(-1, 0, 0, 0, 0);
    let w = sim.world.resource::<BaseMap>().width;
    let at = |x: i32, y: i32| mask[(y * w + x) as usize];
    assert_eq!(at(15, 3), 0, "маска зовёт разобрать пол");
    assert_eq!(at(11, 0), 0, "маска зовёт разобрать сломанный шлюз");
    assert_eq!(at(7, 2), 1, "маска не даёт разобрать завал");
}

/// Сломанный шлюз (§12.376, §12.379): руину сперва обследуют, потом чинят —
/// кнопкой в панели клетки, а не палитрой. Шлюза в палитре нет ни до, ни
/// после: его чинят, а не строят. Дверью руина не считается, выход открывает
/// достроенный шлюз.
#[test]
fn the_shipped_ruleset_examines_then_repairs_the_broken_gate() {
    let mut sim = start();
    let garage = sim.tile_index("garage").expect("шлюз");
    let ruin = sim.tile_index("gate_ruin").expect("обследованный шлюз");
    assert!(
        !sim.add_blueprint(11, 0, i32::from(garage)),
        "шлюз ставится палитрой"
    );
    assert!(
        !sim.add_blueprint(11, 0, i32::from(ruin)),
        "обследование ставится палитрой"
    );
    assert!(!sim.fix_at(10, 0), "чинится скала, а не руина");
    // Обследование размечается и за завалом — работа ждёт прокопа, как
    // любой чертёж (§12.379).
    assert!(sim.fix_at(11, 0), "руина за завалом не размечается");
    dig_to_ruin(&mut sim);
    assert!(!sim.exit_open(), "руина — не дверь");
    assert_eq!(sim.hand_tag("excellent", 11, 0), "examine");
    assert!(
        until_tile(&mut sim, 11, 0, "gate_ruin", 3000),
        "обследовали"
    );
    assert!(!sim.exit_open(), "обследованная руина — тоже не дверь");
    assert_eq!(sim.hand_tag("excellent", 11, 0), "repair");
    assert!(
        !sim.add_blueprint(11, 0, i32::from(garage)),
        "и после обследования шлюз не в палитре"
    );

    let dug = sim.scrap_total();
    assert!(dug >= 3, "прокоп дал лом на шлюз: {dug}");
    assert!(sim.fix_at(11, 0), "шлюз размечен на ремонт");
    let open = (0..3000).any(|_| {
        sim.tick_n(1);
        sim.exit_open()
    });
    assert!(open, "шлюз починен, выход открыт");
    assert_eq!(sim.tile(11, 0), garage);
    assert!(!sim.fix_at(11, 0), "целый шлюз чинить нечего");
    assert!(
        sim.world.resource::<TileRules>().is_internal(garage),
        "шлюз в палитру не идёт"
    );
}

/// Снесённый шлюз оставляет обследованную руину, а не яму и не «?» (§12.376,
/// §12.379): выход наверх потерять нельзя, а обследованное не забывается —
/// шлюз чинится заново сразу.
#[test]
fn the_shipped_ruleset_never_loses_its_way_up() {
    let mut sim = start();
    dig(&mut sim);
    let ruin = sim.tile_index("gate_ruin").expect("обследованный шлюз");
    assert!(sim.add_blueprint(11, 0, -1), "шлюз разбирается");
    let gone = (0..2000).any(|_| {
        sim.tick_n(1);
        sim.tile(11, 0) == ruin
    });
    assert!(gone, "снос оставил обследованный шлюз");
    assert!(!sim.add_blueprint(11, 0, -1), "руину не снести");
    assert_eq!(sim.hand_tag("excellent", 11, 0), "repair");
    assert!(sim.fix_at(11, 0), "и шлюз чинится снова");
}

/// Приказ коту по руине (§12.379): работу берёт **этот** кот, тем же кликом,
/// а не ближайший свободный.
#[test]
fn an_ordered_cat_examines_the_ruin_itself() {
    let mut sim = opened(Sim::new(CORE).expect("рулсет"));
    sim.without_timeline();
    let broken = sim.tile_index("broken_gate").expect("сломанный шлюз");
    sim.force_tile(5, 0, broken);
    let units: Vec<String> = {
        let mut q = sim.world.query::<&UnitId>();
        q.iter(&sim.world).map(|u| u.0.clone()).collect()
    };
    assert!(units.len() > 1, "выбирать есть из кого");
    // Самый дальний от руины — его раздача не взяла бы первым.
    let far = units
        .iter()
        .max_by_key(|id| {
            let (x, y) = sim.pos_of(id);
            ((x - 5).abs() + y, (*id).clone())
        })
        .unwrap()
        .clone();
    assert!(sim.command_at(&far, 5, 0), "приказ принят");
    assert!(sim.has_assignment(&far), "работу взял названный кот");
    for id in units.iter().filter(|id| **id != far) {
        assert!(!sim.has_assignment(id), "`{id}` перехватил руину");
    }
    assert!(until_tile(&mut sim, 5, 0, "gate_ruin", 3000), "обследовали");
}

/// Караван, пришедший раньше починки (§12.376), оставляет груз на руине, а
/// не теряет его: подарок не наказывает за то, что шлюз ещё сломан.
#[test]
fn the_caravan_leaves_its_cargo_at_the_broken_gate() {
    let mut sim = Sim::new(CORE).expect("рулсет");
    let part = sim.item_index("part").expect("деталь");
    let at = sim.world.resource::<crate::components::TimelineRules>().0[0].at;
    sim.tick_n(at as usize + 1);
    assert!(sim.item_at(11, 0, part) > 0, "груз каравана лежит на руине");
}
