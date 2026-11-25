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

/// Откопать ящик под завалом (§12.415, §12.417): шахта вниз до среднего ряда,
/// коридор влево и клетка под ящиком, потом сам завал. В ящике детали, без
/// которых шлюз не чинится.
fn dig_to_bin(sim: &mut Sim) {
    assert!(sim.plan_demolish_rect(11, 4, 1, 5), "шахта вниз размечена");
    assert!(sim.plan_demolish_rect(9, 8, 2, 1), "коридор размечен");
    assert!(
        sim.plan_demolish_rect(8, 8, 1, 1),
        "подход к ящику размечен"
    );
    assert!(
        sim.plan_demolish_rect(8, 7, 1, 1),
        "завал над ящиком размечен"
    );
}

/// Ждать, пока на клетке не встанет тайл `id`.
fn until_tile(sim: &mut Sim, x: i32, y: i32, id: &str, ticks: usize) -> bool {
    let t = sim.tile_index(id).expect("тайл в палитре");
    (0..ticks).any(|_| {
        sim.tick_n(1);
        sim.tile(x, y) == t
    })
}

/// Докопаться, исследовать руину и починить шлюз (§12.376, §12.426):
/// выход открыт, когда шлюз достроен.
fn dig(sim: &mut Sim) {
    dig_to_ruin(sim);
    dig_to_bin(sim);
    assert!(sim.fix_at(11, 0), "шлюз размечен на исследование");
    assert!(
        until_tile(sim, 11, 0, "gate_ruin", 3000),
        "исследовали — сломанный шлюз"
    );
    // Без детали ремонт не размечается (§12.419): ждём, пока ящик откопают.
    assert!(
        (0..6000).any(|_| {
            sim.tick_n(1);
            sim.fix_why(11, 0).is_empty()
        }),
        "детали на ремонт достали"
    );
    assert!(sim.fix_at(11, 0), "шлюз размечен на ремонт");
    let open = (0..6000).any(|_| {
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
/// «Пошарить» — после «Исследовать местность вокруг шлюза» (§12.377). Стол
/// брифинга с §12.418 ждёт ещё и починки пыльного стола картой.
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
    assert!(!sim.home_met(), "база на карте — «?» до осмотра (§12.422)");

    dig(&mut sim);
    assert!(sim.scavenge(inspect, vec!["excellent".into()], -1));
    assert!(done(&mut sim, inspect, 2000), "вход осмотрен");
    assert!(sim.home_met(), "осмотр ставит базу на карту (§12.422)");
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
    assert!(
        !sim.structure_is_open(muster),
        "стол брифинга открылся до починки пыльного"
    );
    assert!(sim.raid_gates(scavenge).unlocked, "открылось «Пошарить»");
}

/// Первый отряд — пыльный стол, починенный картой (§12.418): «Исследовать
/// местность» приносит карту, комнатку со столом откапывают, стол чинят, и
/// только тогда он становится отрядом, а место брифинга — палитрой. Карта
/// уходит в стол целиком.
#[test]
fn the_shipped_ruleset_repairs_the_first_squad_with_a_map() {
    let mut sim = start();
    let gatehouse = sim.mission_index("explore_gatehouse").expect("местность");
    let inspect = sim.mission_index("inspect").expect("осмотр");
    let muster = sim.structure_index("muster").expect("место брифинга");
    let map = sim.item_index("map").expect("карта");
    let hidden = sim.tile_index("rubble_table").expect("завал над столом");
    assert_eq!(sim.tile(7, 3), hidden, "посреди левой верхней — завал");
    assert!(
        sim.node_cells_of().is_empty(),
        "пыльный стол — ещё не отряд"
    );

    dig(&mut sim);
    assert!(sim.scavenge(inspect, vec!["excellent".into()], -1));
    assert!(done(&mut sim, inspect, 3000), "вход осмотрен");
    assert!(sim.scavenge(gatehouse, vec!["excellent".into()], -1));
    assert!(done(&mut sim, gatehouse, 6000), "местность исследована");
    assert_eq!(sim.item_total(map), 1, "принесли карту");

    // Комнатка под завалом: стулья вокруг стола — слоты, и откапывают их тем
    // же ластиком (§12.418), а дорога к ней — коридор от шахты.
    assert!(sim.plan_demolish_rect(9, 3, 2, 1), "коридор размечен");
    assert!(sim.plan_demolish_rect(6, 2, 3, 3), "комнатка размечена");
    assert!(
        until_tile(&mut sim, 7, 3, "dusty_table", 20000),
        "под завалом нашли пыльный стол"
    );
    assert!(!sim.add_blueprint(7, 3, -1), "пыльный стол не сносится");
    assert!(
        sim.node_cells_of().is_empty(),
        "обследованный стол без карты — ещё не отряд"
    );
    assert!(sim.fix_at(7, 3), "пыльный стол размечен на ремонт");
    assert!(
        until_tile(&mut sim, 7, 3, "briefing_table", 20000),
        "стол починили"
    );
    assert_eq!(sim.item_total(map), 0, "карта ушла в стол");
    assert_eq!(sim.node_cells_of(), vec![(7, 3)], "стол стал отрядом");
    assert!(sim.structure_is_open(muster), "место брифинга в палитре");
    let junkyard = sim.mission_index("junkyard").expect("свалка");
    assert!(
        sim.raid_gates(junkyard).unlocked,
        "починенный стол открывает Свалку (§12.421)"
    );
    assert!(!sim.add_blueprint(7, 3, -1), "починенный стол не сносится");

    // Раскопанный и починенный стол — всё тот же объект (§12.420): разбор
    // завала и ремонт растят клетку штампа, а не разбирают его. Иначе стол
    // остаётся без мест вокруг, и вылазка распускается тем же тиком.
    for cat in ["excellent", "sp2", "sp3"] {
        assert!(sim.enlist(cat, 7, 3), "{cat} записан в отряд");
    }
    let scavenge = sim.mission_index("scavenge").expect("пошарить");
    assert!(sim.launch_node(scavenge, 7, 3, -1), "заявка принята");
    sim.tick_n(5);
    assert!(sim.in_squad("excellent"), "отряд не распустился");
    assert!(
        (0..3000).any(|_| {
            sim.tick_n(1);
            sim.is_away("excellent")
        }),
        "отряд сел за стол и ушёл"
    );
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
    // Под одной клеткой завала лежит малый ящик (§12.404).
    let hidden = sim.tile_index("rubble_bin").expect("завал над ящиком");
    assert_eq!(sim.tile(8, 7), hidden, "над входом в левую среднюю — ящик");
    // Посреди левой верхней — завал над пыльным столом (§12.418).
    let dusty = sim.tile_index("rubble_table").expect("завал над столом");
    assert_eq!(
        sim.tile(7, 3),
        dusty,
        "посреди левой верхней — завал над столом"
    );
    // В комнатке портала — завалы над сломанной лежанкой и старым складом
    // (§12.423).
    let bed = sim.tile_index("rubble_bed").expect("завал над лежанкой");
    let store = sim.tile_index("rubble_storage").expect("завал над складом");
    assert_eq!(sim.tile(16, 2), bed, "в углу комнатки портала — лежанка");
    assert_eq!(sim.tile(14, 4), store, "в другом углу — склад");
    let finds = [(15, 3), (8, 7), (7, 3), (16, 2), (14, 4)];
    for (x, y) in cells {
        if !finds.contains(&(x, y)) {
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
    assert_eq!(
        piles, 75,
        "ещё четыре клетки — завалы над ящиком, столом, лежанкой и складом"
    );
    let ruin = sim.tile_index("broken_gate").expect("неисследованный шлюз");
    assert_eq!(
        sim.tile(11, 0),
        ruin,
        "над шахтой — неисследованный шлюз без завала"
    );
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
/// (§12.376), пол — выполненный «Осмотреть вход», а склад и лежанку с
/// §12.424 — находка: склад откопан, сломанная лежанка починена.
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
    let open_of = |sim: &mut Sim, id: &str| {
        let t = sim.tile_index(id).expect("тайл в палитре");
        sim.tile_is_open(t as usize)
    };
    assert!(open_of(&mut sim, "floor"), "пол не открылся осмотром");
    for id in ["storage", "bed"] {
        assert!(
            !open_of(&mut sim, id),
            "`{id}` открылся осмотром, без находки"
        );
    }

    assert!(
        sim.plan_demolish_rect(14, 2, 3, 3),
        "комнатка портала размечена"
    );
    assert!(
        until_tile(&mut sim, 14, 4, "storage", 3000),
        "склад откопан"
    );
    assert!(open_of(&mut sim, "storage"), "склад не открылся находкой");
    assert!(!open_of(&mut sim, "bed"), "лежанка открылась до починки");
    assert!(
        until_tile(&mut sim, 16, 2, "broken_bed", 3000),
        "сломанная лежанка откопана"
    );
    assert!(sim.fix_at(16, 2), "лежанка размечена на ремонт");
    assert!(until_tile(&mut sim, 16, 2, "bed", 3000), "лежанка починена");
    assert!(open_of(&mut sim, "bed"), "лежанка не открылась починкой");
}

/// Деталь, потраченная до шлюза, возвращается сносом (§12.424): лежанка в
/// палитре с починки старой, и единственную деталь дебюта можно поставить в
/// новую раньше шлюза. Тупика нет — снос возвращает цену, — а отказ у шлюза
/// говорит об этом словом: `built`, а не голое `lacks`.
#[test]
fn the_shipped_ruleset_returns_the_part_spent_before_the_gate() {
    let mut sim = start();
    assert!(
        sim.plan_demolish_rect(14, 2, 3, 3),
        "комнатка портала размечена"
    );
    assert!(
        until_tile(&mut sim, 16, 2, "broken_bed", 3000),
        "сломанная лежанка откопана"
    );
    assert!(sim.fix_at(16, 2), "лежанка размечена на ремонт");
    assert!(until_tile(&mut sim, 16, 2, "bed", 3000), "лежанка починена");
    dig_to_ruin(&mut sim);
    dig_to_bin(&mut sim);
    assert!(sim.fix_at(11, 0), "шлюз размечен на исследование");
    assert!(
        until_tile(&mut sim, 11, 0, "gate_ruin", 3000),
        "под завалом нашли сломанный шлюз"
    );
    let part = sim.item_index("part").expect("деталь");
    assert!(
        (0..6000).any(|_| {
            sim.tick_n(1);
            sim.item_total(part) > 0
        }),
        "деталь из ящика достали"
    );

    // Сторож обратимости (§12.424): всё, что до шлюза открыто в палитре и
    // просит деталь, сносимо — снос возвращает цену, и деталь не теряется.
    // Неснимаемая постройка с деталью до выхода заперла бы партию: детали
    // взять больше негде («Пошарить рядом» ждёт выхода).
    let n = sim.world.resource::<TileRules>().0.len();
    let mut sinks = Vec::new();
    for t in 0..n {
        let (spends, fixed) = {
            let rules = sim.world.resource::<TileRules>();
            let spends = !rules.is_internal(t as i16)
                && rules.cost_of(t as i16).iter().any(|&(i, _)| i == part);
            (spends, rules.is_fixed(t as i16))
        };
        if spends && fixed && sim.tile_is_open(t) {
            sinks.push(t);
        }
    }
    assert!(
        sinks.is_empty(),
        "до шлюза деталь уходит безвозвратно в {sinks:?}"
    );

    // Деталь — в новую лежанку, мимо шлюза.
    let bed = sim.tile_index("bed").expect("лежанка");
    assert!(
        until_tile(&mut sim, 16, 4, "floor", 6000),
        "угол комнатки портала откопан"
    );
    assert!(
        sim.add_blueprint(16, 4, i32::from(bed)),
        "лежанка в палитре после починки старой"
    );
    assert!(
        until_tile(&mut sim, 16, 4, "bed", 6000),
        "новую лежанку построили"
    );
    assert_eq!(sim.item_total(part), 0, "деталь ушла в лежанку");
    assert_eq!(
        sim.fix_why(11, 0),
        "built",
        "отказ говорит, что деталь в постройке"
    );
    assert_eq!(sim.hand_tag("excellent", 11, 0), "built");
    assert!(!sim.fix_at(11, 0), "без детали ремонт не размечается");

    // Снос возвращает деталь, и шлюз чинится.
    assert!(sim.add_blueprint(16, 4, -1), "лежанку сносят");
    assert!(
        (0..6000).any(|_| {
            sim.tick_n(1);
            sim.fix_why(11, 0).is_empty()
        }),
        "снос вернул деталь на ремонт"
    );
    assert!(sim.fix_at(11, 0), "шлюз размечен на ремонт");
    assert!(
        (0..6000).any(|_| {
            sim.tick_n(1);
            sim.exit_open()
        }),
        "шлюз починен"
    );
}

/// Что нельзя построить, того нельзя и разобрать (§12.372): на старте ластик
/// берёт только завал — пол комнаты и единственный шлюз ему не даются, иначе
/// партия запиралась бы одним мазком.
#[test]
fn the_shipped_ruleset_dismantles_only_rubble_at_the_start() {
    let mut sim = start();
    assert!(!sim.add_blueprint(15, 3, -1), "пол у портала разобран");
    assert!(sim.add_blueprint(6, 2, -1), "завал не разбирается");
    assert!(
        !sim.add_blueprint(11, 0, -1),
        "неисследованный шлюз разбирается (§12.426)"
    );
    let mask = sim.buildable(-1, 0, 0, 0, 0);
    let w = sim.world.resource::<BaseMap>().width;
    let at = |x: i32, y: i32| mask[(y * w + x) as usize];
    assert_eq!(at(15, 3), 0, "маска зовёт разобрать пол");
    assert_eq!(at(7, 2), 1, "маска не даёт разобрать завал");
}

/// Сломанный шлюз (§12.376, §12.418): верх шахты засыпан, и шлюз
/// показывается разбором завала, а потом чинится кнопкой в панели клетки, а
/// не палитрой. Шлюза в палитре нет ни до, ни после. Дверью руина не
/// считается, выход открывает достроенный шлюз.
#[test]
fn the_shipped_ruleset_uncovers_then_repairs_the_broken_gate() {
    let mut sim = start();
    let garage = sim.tile_index("garage").expect("шлюз");
    let ruin = sim.tile_index("gate_ruin").expect("сломанный шлюз");
    assert!(
        !sim.add_blueprint(11, 0, i32::from(garage)),
        "шлюз ставится палитрой"
    );
    // Исследование размечается и за завалом шахты — работа ждёт прокопа
    // (§12.426: над самим шлюзом завала нет).
    assert!(
        !sim.plan_demolish(11, 0),
        "неисследованный шлюз разбирается"
    );
    assert!(sim.fix_at(11, 0), "шлюз не размечается на исследование");
    dig_to_ruin(&mut sim);
    assert!(
        until_tile(&mut sim, 11, 0, "gate_ruin", 3000),
        "исследовали — сломанный шлюз"
    );
    assert!(!sim.exit_open(), "руина — не дверь");
    assert!(
        !sim.add_blueprint(11, 0, i32::from(garage)),
        "и после разбора шлюз не в палитре"
    );

    let dug = sim.scrap_total();
    assert!(dug >= 3, "прокоп дал лом на шлюз: {dug}");
    // Ремонт стоит детали (§12.417), а она только в ящике под завалом: пока
    // его не откопали, «Починить» не размечается вовсе (§12.419) — ни кнопкой,
    // ни приказом коту, и причина едет тегом. Детали база ещё не видела, а
    // лом видела (§12.426): кнопка погашена, деталь в цене — «??».
    assert_eq!(sim.fix_why(11, 0), "unknown");
    assert_eq!(sim.hand_tag("excellent", 11, 0), "unknown");
    assert!(!sim.fix_at(11, 0), "шлюз размечен на ремонт без детали");
    sim.tick_n(300);
    assert_eq!(sim.tile(11, 0), ruin, "руина стоит, пока ящик не откопан");
    dig_to_bin(&mut sim);
    let part = sim.item_index("part").expect("деталь");
    assert!(
        (0..3000).any(|_| {
            sim.tick_n(1);
            sim.item_total(part) > 0
        }),
        "из-под завала достали детали"
    );
    assert_eq!(sim.fix_why(11, 0), "", "деталь есть — чинить можно");
    assert_eq!(sim.hand_tag("excellent", 11, 0), "repair");
    assert!(sim.fix_at(11, 0), "шлюз размечен на ремонт");
    let open = (0..6000).any(|_| {
        sim.tick_n(1);
        sim.exit_open()
    });
    assert!(open, "шлюз починен, выход открыт");
    assert_eq!(sim.tile(11, 0), garage);
    sim.tick_n(300);
    assert_eq!(
        sim.item_total(part),
        0,
        "на шлюз ушла единственная деталь из ящика"
    );
    assert!(!sim.fix_at(11, 0), "целый шлюз чинить нечего");
    assert!(
        sim.world.resource::<TileRules>().is_internal(garage),
        "шлюз в палитру не идёт"
    );
}

/// Починенный шлюз не сносится вовсе (§12.417): ремонт стоит детали, и снос,
/// вернувший её кучей, позволил бы потратить её на другое — база осталась бы
/// без выхода и без вылазок, то есть без единого способа добыть деталь. До
/// §12.417 снос оставлял обследованную руину (§12.376, §12.379).
#[test]
fn the_shipped_ruleset_never_loses_its_way_up() {
    let mut sim = start();
    dig(&mut sim);
    let garage = sim.tile_index("garage").expect("шлюз");
    assert!(!sim.add_blueprint(11, 0, -1), "шлюз разбирается");
    assert!(!sim.plan_demolish(11, 0), "шлюз сносится кликом");
    sim.tick_n(500);
    assert_eq!(sim.tile(11, 0), garage, "шлюз на месте");
    assert!(sim.exit_open(), "выход открыт");
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
    // Каравана в боевом рулсете сейчас нет (§12.409) — событие заводится
    // тестом: механизм груза на руине жив и ждёт возвращения каравана.
    let mut sim = Sim::new(CORE).expect("рулсет");
    let part = sim.item_index("part").expect("деталь");
    let at = 600;
    sim.set_event(at, &[], &[(part, 4)], 0, 0);
    sim.tick_n(at as usize + 1);
    assert!(sim.item_at(11, 0, part) > 0, "груз каравана лежит на руине");
}

/// Приказ выбранному коту по завалу (§12.395): второй клик по клетке завала
/// размечает разбор и отдаёт его **этому** коту — как работа над руиной.
#[test]
fn an_ordered_cat_clears_the_rubble_itself() {
    let mut sim = start();
    let rubble = sim.tile_index("rubble").expect("завал");
    assert_eq!(sim.tile(14, 3), rubble, "рядом с котом завал");
    assert_eq!(sim.hand_tag("excellent", 14, 3), "dismantle");
    assert_eq!(
        sim.hand_tag("excellent", 15, 3),
        "",
        "пол — обычное «иди туда»"
    );
    assert!(sim.command_at("excellent", 14, 3), "приказ принят");
    assert!(sim.has_assignment("excellent"), "разбор взял названный кот");
    let cleared = (0..3000).any(|_| {
        sim.tick_n(1);
        sim.tile(14, 3) != rubble
    });
    assert!(cleared, "завал разобран");
    assert!(sim.scrap_total() > 0, "разбор дал лом");
}

/// Повторный приказ по уже размеченному разбору его не отменяет, как ластик,
/// а отдаёт коту (§12.395).
#[test]
fn an_order_keeps_a_planned_rubble_clearing() {
    let mut sim = start();
    assert!(sim.plan_demolish(14, 3), "разбор размечен кнопкой");
    assert!(sim.command_at("excellent", 14, 3), "приказ принят");
    assert!(sim.has_assignment("excellent"), "кот при деле");
    assert!(sim.command_at("excellent", 14, 3), "второй приказ тоже");
    assert!(sim.has_assignment("excellent"), "разбор не снят");
}

/// **Под завалом найдётся малый ящик** (§12.404): разбор этой клетки оставляет
/// не пол, а ящик — с двумя деталями и назначенный на них (§12.415),
/// переназначаемый с первой минуты и до темы «Малый ящик» неразбираемый
/// (что нельзя построить, того нельзя и разобрать, §12.372).
#[test]
fn the_shipped_ruleset_hides_a_small_bin_under_the_rubble() {
    let mut sim = start();
    let rubble = sim.tile_index("rubble").expect("завал");
    let floor = sim.tile_index("floor").expect("пол");
    let bin = sim.tile_index("small_bin").expect("малый ящик");
    // Остальной завал снимаем мимо котов: проверяем находку, а не раскопки.
    let (w, h) = {
        let map = sim.world.resource::<BaseMap>();
        (map.width, map.height)
    };
    for x in 0..w {
        for y in 0..h {
            if sim.tile(x, y) == rubble {
                sim.force_tile(x, y, floor);
            }
        }
    }
    assert!(
        sim.plan_demolish_rect(8, 7, 1, 1),
        "завал над ящиком разбирается"
    );
    let found = (0..2000).any(|_| {
        sim.tick_n(1);
        sim.tile(8, 7) == bin
    });
    assert!(found, "под завалом так и не нашёлся ящик");

    // Ящик найден не пустым (§12.415): деталь внутри (§12.417, §12.423), и
    // назначен он на них — без назначения коты вынесли бы находку на пол
    // (§12.402).
    let part = sim.item_index("part").expect("деталь");
    sim.tick_n(1);
    assert_eq!(sim.item_at(8, 7, part), 1, "в найденном ящике деталь");
    assert_eq!(sim.bin_of(8, 7), vec![part], "ящик назначен на детали");
    assert!(sim.seen(part), "находка знакомит базу с деталью");
    sim.tick_n(300);
    assert_eq!(
        sim.item_at(8, 7, part),
        1,
        "деталь из ящика никто не выносит"
    );

    assert!(
        !sim.add_blueprint(8, 7, -1),
        "найденный ящик до темы разобран ластиком"
    );
    let scrap = sim.item_index("scrap").expect("лом");
    assert!(
        sim.set_bin(8, 7, scrap, true),
        "а назначить его можно сразу"
    );
}

/// Рамка «Разобрать» по всей стартовой базе метит **весь** завал (§12.408),
/// а не одну клетку над ящиком, которая на слой выше прочих.
#[test]
fn the_shipped_ruleset_erases_all_rubble_with_one_rect() {
    let mut sim = start();
    let rubble = sim.tile_index("rubble").expect("завал");
    let (w, h) = {
        let map = sim.world.resource::<BaseMap>();
        (map.width, map.height)
    };
    assert!(sim.plan_demolish_rect(0, 0, w, h), "рамка что-то разметила");
    let missed: Vec<(i32, i32)> = (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|&(x, y)| sim.tile(x, y) == rubble && sim.planned_tile(x, y) != Some(-1))
        .collect();
    assert!(missed.is_empty(), "завал без пометки: {missed:?}");
    assert_eq!(sim.planned_tile(8, 7), Some(-1), "и завал над ящиком тоже");
    assert_eq!(sim.planned_tile(7, 3), Some(-1), "и над пыльным столом");
    assert_eq!(
        sim.planned_tile(11, 0),
        None,
        "шлюз рамка не трогает (§12.426: завала над ним нет)"
    );
}

/// Обследование руины растит «Науку», а не «Стройку» (§12.410): навык у
/// ставящегося тайла (`skill` у «Сломанного шлюза»), а прокоп — по-прежнему
/// «Стройка».
#[test]
fn examining_the_ruin_trains_science() {
    let mut sim = start();
    let science = sim.skill_index("science").expect("наука");
    let build = sim.skill_index("build").expect("стройка");
    // С §12.426 боевой старт снова ставит руину сам; ставим явно, чтобы тест
    // не зависел от стартовой застройки.
    let broken = sim
        .tile_index("broken_gate")
        .expect("необследованная руина");
    sim.force_tile(11, 0, broken);
    assert!(sim.fix_at(11, 0));
    dig_to_ruin(&mut sim);
    let (sci0, build0) = (
        sim.xp_of("excellent", science),
        sim.xp_of("excellent", build),
    );
    assert!(build0 > 0, "прокоп растил стройку");
    assert!(
        until_tile(&mut sim, 11, 0, "gate_ruin", 3000),
        "обследовали"
    );
    assert!(
        sim.xp_of("excellent", science) > sci0,
        "обследование растит науку"
    );
    assert_eq!(sim.xp_of("excellent", build), build0, "а стройку — нет");
}

/// Без `skill` у тайла чертёж растит «Стройку», как всегда (§12.410).
#[test]
fn a_tile_without_a_skill_trains_building() {
    let mut sim = start();
    let science = sim.skill_index("science").expect("наука");
    let build = sim.skill_index("build").expect("стройка");
    dig_to_ruin(&mut sim);
    assert!(sim.xp_of("excellent", build) > 0, "прокоп — стройка");
    assert_eq!(sim.xp_of("excellent", science), 0, "науки за прокоп нет");
}

/// Рамка «Разобрать» по всей базе шлюз не сносит (§12.411). С §12.417 его не
/// снимает и клик: починенный шлюз неснимаем. Завал рядом рамка берёт как
/// обычно.
#[test]
fn a_rect_eraser_leaves_the_gate_alone() {
    let mut sim = start();
    dig(&mut sim);
    let rubble = sim.tile_index("rubble").expect("завал");
    let (w, h) = {
        let map = sim.world.resource::<BaseMap>();
        (map.width, map.height)
    };
    assert!(sim.plan_demolish_rect(0, 0, w, h), "рамка что-то разметила");
    assert_eq!(sim.planned_tile(11, 0), None, "шлюз рамкой не тронут");
    let left_rubble = (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .find(|&(x, y)| sim.tile(x, y) == rubble);
    if let Some((x, y)) = left_rubble {
        assert_eq!(sim.planned_tile(x, y), Some(-1), "завал рамкой размечен");
    }
    assert!(!sim.plan_demolish(11, 0), "одной клеткой — сносится");
    assert_eq!(sim.planned_tile(11, 0), None);
}

/// Под завалом в комнатке портала — сломанная лежанка (§12.423): первая
/// починка партии, раньше шлюза, и платят за неё ломом, который даёт сам
/// раскоп (`repair_cost`, §12.425). Починенная — обычная лежанка, и снос
/// возвращает её цену целиком, деталь включительно: одна лишняя деталь —
/// принятая цена простоты.
#[test]
fn the_shipped_ruleset_repairs_the_broken_bed_with_scrap() {
    let mut sim = start();
    assert!(
        sim.plan_demolish_rect(15, 2, 2, 1),
        "подход и завал над лежанкой размечены"
    );
    assert!(
        until_tile(&mut sim, 16, 2, "broken_bed", 3000),
        "под завалом нашли сломанную лежанку"
    );
    let broken = sim.tile_index("broken_bed").expect("сломанная лежанка");
    assert_eq!(
        sim.world.resource::<TileRules>().rest_of(broken),
        0,
        "на сломанной не выспаться"
    );

    // Лом от раскопа уже на базе, а детали нет: ремонт её и не просит.
    let part = sim.item_index("part").expect("деталь");
    assert_eq!(sim.item_total(part), 0, "детали на базе нет");
    assert_eq!(sim.fix_why(16, 2), "", "лома на ремонт хватает");
    assert_eq!(sim.hand_tag("excellent", 16, 2), "repair");
    assert!(sim.fix_at(16, 2), "лежанка размечена на ремонт");
    assert!(
        until_tile(&mut sim, 16, 2, "bed", 3000),
        "починили — стала обычной лежанкой"
    );

    assert!(sim.add_blueprint(16, 2, -1), "лежанку можно снести");
    assert!(
        until_tile(&mut sim, 16, 2, "floor", 3000),
        "снос оставляет пол"
    );
    assert!(
        (0..300).any(|_| {
            sim.tick_n(1);
            sim.item_total(part) == 1
        }),
        "снос вернул цену лежанки, деталь включительно"
    );
}

/// Склад под завалом (§12.423, §12.425) не чинится: откопали — и коты сами
/// понесли в него лом с раскопа, урок §12.216 делом. Это обычный склад.
#[test]
fn the_shipped_ruleset_finds_a_storage_that_fills_itself() {
    let mut sim = start();
    assert!(
        sim.plan_demolish_rect(14, 3, 1, 2),
        "подход и завал над складом размечены"
    );
    assert!(
        until_tile(&mut sim, 14, 4, "storage", 3000),
        "под завалом нашёлся склад"
    );
    assert!(
        sim.plan_demolish_rect(15, 2, 1, 1),
        "ещё клетка раскопа — ещё лом"
    );
    let scrap = sim.item_index("scrap").expect("лом");
    assert!(
        (0..3000).any(|_| {
            sim.tick_n(1);
            sim.item_at(14, 4, scrap) > 0
        }),
        "лом с раскопа доехал на склад сам"
    );
}

/// Цена ремонта — тоже находка (§12.426): пока база не видела ничего из неё,
/// «Починить» нет вовсе (`unseen`); видела лом, но не деталь — кнопка
/// погашена, деталь в цене «??» (`unknown`); видела всё, но не хватает —
/// прежнее `lacks`.
#[test]
fn the_shipped_ruleset_hides_the_repair_until_its_price_is_seen() {
    let mut sim = start();
    let ruin = sim.tile_index("gate_ruin").expect("сломанный шлюз");
    sim.force_tile(11, 0, ruin);
    let scrap = sim.item_index("scrap").expect("лом");
    let part = sim.item_index("part").expect("деталь");
    assert!(
        !sim.seen(scrap) && !sim.seen(part),
        "на старте база не видела ничего"
    );
    assert_eq!(sim.fix_why(11, 0), "unseen");
    assert!(!sim.fix_at(11, 0), "чинят тем, чего не видели");

    sim.put_item(15, 3, scrap, 5);
    sim.tick_n(1);
    assert!(sim.seen(scrap));
    assert_eq!(sim.fix_why(11, 0), "unknown", "деталь ещё незнакома");

    sim.put_item(15, 3, part, 1);
    sim.tick_n(1);
    sim.take_item(15, 3, part);
    assert_eq!(sim.item_total(part), 0, "деталь унесли");
    sim.tick_n(1);
    assert!(
        sim.seen(part),
        "деталь знакома и после того, как её не стало"
    );
    assert_eq!(sim.fix_why(11, 0), "lacks", "знакомо всё, но детали нет");
}

/// Знак ремонта гаснет над руиной, с которой ничего не сделать (§12.427):
/// пыльный стол до первой карты в мире базы — в списке `ruins_mute`, тем же
/// `fix_why`, каким панель прячет «Починить». Карта знакома — знак вернулся.
#[test]
fn the_shipped_ruleset_mutes_ruins_it_cannot_repair_yet() {
    let mut sim = start();
    let dusty = sim.tile_index("dusty_table").expect("пыльный стол");
    sim.force_tile(7, 3, dusty);
    let w = sim.world.resource::<BaseMap>().width;
    let cell = (3 * w + 7) as u32;
    assert!(sim.ruins_mute().contains(&cell), "стол до карты молчит");

    let map = sim.item_index("map").expect("карта");
    sim.put_item(15, 3, map, 1);
    sim.tick_n(1);
    assert!(sim.seen(map));
    assert!(
        !sim.ruins_mute().contains(&cell),
        "карта знакома — над столом знак ремонта"
    );
}

/// Знак молчит и при знакомстве с частью цены (§12.428): сломанный шлюз, когда
/// лом знаком, а деталь нет, — `unknown`, ремонт невозможен, знака нет.
/// Деталь знакома — знак вернулся, хотя её и не хватает (`lacks`).
#[test]
fn the_shipped_ruleset_mutes_the_gate_until_its_part_is_seen() {
    let mut sim = start();
    let ruin = sim.tile_index("gate_ruin").expect("сломанный шлюз");
    sim.force_tile(11, 0, ruin);
    let cell = 11u32; // (11, 0): верхний ряд, индекс row-major — это x
    let scrap = sim.item_index("scrap").expect("лом");
    let part = sim.item_index("part").expect("деталь");
    sim.put_item(15, 3, scrap, 5);
    sim.tick_n(1);
    assert_eq!(sim.fix_why(11, 0), "unknown");
    assert!(
        sim.ruins_mute().contains(&cell),
        "деталь незнакома — знака нет"
    );

    sim.put_item(15, 3, part, 1);
    sim.tick_n(1);
    sim.take_item(15, 3, part);
    sim.tick_n(1);
    assert_eq!(sim.fix_why(11, 0), "lacks");
    assert!(
        !sim.ruins_mute().contains(&cell),
        "деталь знакома — знак есть"
    );
}
