//! Учёба в простое (§12.314): галочка на кота и навык, по которой кот сам
//! садится за парту, когда ему больше нечем заняться.
//!
//! Правило берёт только свободное время — и своё, и учителя: с работы оно не
//! срывает никого, а ученик без учителя не ждёт, а встаёт. Сеанс — один
//! уровень, очередь честная: сперва младший уровень.

use super::*;

const CORE: &str = include_str!("../../assets/rulesets/core.yaml");

const DESK: i16 = 1;
const TABLE: i16 = 2;
const LECTERN: i16 = 3;
/// Чем занять кота стройкой: любой тайл, кроме пола схемы (CLAUDE.md).
const OTHER: i16 = 5;

/// Коридор с одинокой партой без места учителя — она учит сама, как в схемах
/// до §12.258. Порог «Науки» — 20 / 100 / 300, парта доводит до второго.
fn school(rows: &[&str], desk: (i32, i32)) -> (Sim, usize) {
    let mut sim = sim_from(rows);
    let science = sim.set_skill("science", &[20, 100, 300]);
    sim.set_taught(science, 2);
    sim.set_teaches(DESK, science);
    sim.tile_rule(OTHER, |_| {});
    sim.force_tile(desk.0, desk.1, DESK);
    (sim, science)
}

/// Парта-штамп `[парта, стол, учитель]` (§12.258), как в `tutoring.rs`:
/// комната 7×2, нижний ряд — проход.
fn classroom() -> (Sim, usize) {
    let mut sim = sim_from(&["#########", "#.......#", "#a.....b#", "#########"]);
    let science = sim.set_skill("science", &[20, 100, 300]);
    sim.set_taught(science, 2);
    sim.set_teaches(DESK, science);
    sim.set_solid(TABLE, true);
    sim.tile_rule(LECTERN, |r| r.lectern = true);
    sim.tile_rule(OTHER, |_| {});
    let def = sim.set_structure(vec![vec![Some(DESK), Some(TABLE), Some(LECTERN)]]);
    assert!(sim.place_structure(def as i32, 2, 1, 0), "штамп встал");
    sim.tick_n(80); // коты строят все три клетки
    assert_eq!(sim.tile(4, 1), LECTERN, "место учителя построено");
    (sim, science)
}

const CORRIDOR: &[&str] = &["#######", "#a....#", "#######"];

/// Свободный кот садится за парту сам и доучивается до её потолка — по уровню
/// за сеанс, без записи игрока (`Enrolled`), и дальше не идёт.
#[test]
fn an_idle_cat_studies_up_to_the_desk_ceiling() {
    let (mut sim, science) = school(CORRIDOR, (5, 1));
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(2);
    assert_eq!(sim.studies_by_rule("a"), Some(science), "пошёл к парте сам");
    assert!(!sim.is_enrolled("a"), "это правило, а не запись игрока");

    sim.tick_n(400);
    assert_eq!(sim.xp_of("a", science), 100, "ровно потолок парты");
    assert!(!sim.is_studying("a"), "дальше парта не учит — встал");
    assert_eq!(
        sim.self_study_of("a"),
        vec![science],
        "галочка остаётся: её снимает игрок"
    );
}

/// Правило берёт только простой: пока есть работа, кот работает.
#[test]
fn work_comes_before_the_rule() {
    let (mut sim, science) = school(CORRIDOR, (5, 1));
    assert!(sim.add_blueprint(3, 1, OTHER as i32));
    assert!(sim.set_self_study("a", "science", true));
    let mut built = false;
    for _ in 0..200 {
        sim.tick();
        if sim.has_assignment("a") {
            assert_eq!(sim.studies_by_rule("a"), None, "стройка вперёд учёбы");
        } else if sim.tile(3, 1) == OTHER {
            built = true;
            break;
        }
    }
    assert!(built, "стройка закончилась");
    sim.tick_n(2);
    assert_eq!(
        sim.studies_by_rule("a"),
        Some(science),
        "освободился — учится"
    );
}

/// Очередь честная (§12.269): младший уровень первым, при равенстве — `id`.
#[test]
fn the_lowest_level_goes_first() {
    let (mut sim, science) = school(&["#######", "#a...b#", "#######"], (3, 1));
    sim.set_xp("a", science, 20); // первый уровень
    assert!(sim.set_self_study("a", "science", true));
    assert!(sim.set_self_study("b", "science", true));
    sim.tick_n(2);
    assert!(sim.studies_by_rule("b").is_some(), "у b ноль — он первый");
    assert!(sim.studies_by_rule("a").is_none(), "парта одна");

    let mut grew = false;
    for _ in 0..200 {
        sim.tick();
        if sim.level_of("b", science) == 1 {
            grew = true;
            break;
        }
    }
    assert!(grew, "b взял первый уровень");
    sim.tick();
    assert!(
        sim.studies_by_rule("a").is_some(),
        "оба на первом — по `id`, и это a"
    );
    assert!(sim.studies_by_rule("b").is_none());
}

/// У одного кота — сперва младший из отмеченных навыков.
#[test]
fn a_cat_studies_its_weaker_skill_first() {
    let (mut sim, science) = school(CORRIDOR, (5, 1));
    let relay = sim.set_skill("relay", &[20, 100, 300]);
    sim.set_taught(relay, 2);
    sim.add_teaches(DESK, relay);
    sim.set_xp("a", science, 20);
    assert!(sim.set_self_study("a", "science", true));
    assert!(sim.set_self_study("a", "relay", true));
    assert_eq!(sim.self_study_of("a"), vec![science, relay]);
    sim.tick_n(2);
    assert_eq!(sim.studies_by_rule("a"), Some(relay), "«Связь» на нуле");
}

/// Свободного учителя правило сажает само — пару одним ходом.
#[test]
fn the_rule_seats_an_idle_teacher() {
    let (mut sim, science) = classroom();
    sim.set_xp("b", science, 300); // третий уровень
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(3);
    assert!(sim.is_teaching("b"), "учитель простаивал — сел");
    assert_eq!(sim.studies_by_rule("a"), Some(science));

    sim.tick_n(400);
    assert_eq!(sim.xp_of("a", science), 100, "до потолка парты");
    assert!(!sim.is_teaching("b"), "учить больше некого — встал");
}

/// Учителя с работы правило не срывает (§12.314) — в отличие от «Учить»
/// (§12.258), — а без учителя ученик за парту не садится вовсе.
#[test]
fn the_rule_never_pulls_a_teacher_off_work() {
    let (mut sim, science) = classroom();
    sim.set_xp("b", science, 300);
    assert!(sim.add_blueprint(7, 1, OTHER as i32));
    sim.tick_n(2);
    assert!(sim.has_assignment("b"), "b ближе к площадке — строит он");
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(3);
    assert!(!sim.is_teaching("b"), "стройку ради правила не бросил");
    assert!(sim.has_assignment("b"));
    assert_eq!(sim.studies_by_rule("a"), None, "без учителя не сел");

    sim.tick_n(200);
    assert_eq!(sim.tile(7, 1), OTHER, "b достроил");
    assert!(sim.xp_of("a", science) > 0, "и после этого a учился у него");
}

/// Учителя увели — ученик правила не ждёт его за партой, а встаёт.
#[test]
fn a_pupil_by_rule_does_not_wait_for_a_teacher() {
    let (mut sim, science) = classroom();
    sim.set_xp("b", science, 300);
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(10);
    assert!(sim.is_teaching("b"));
    assert!(sim.set_target("b", 7, 2), "приказ увёл учителя");
    sim.tick_n(2);
    assert!(!sim.is_teaching("b"));
    assert_eq!(sim.studies_by_rule("a"), None, "ученик встал, а не ждёт");
}

/// Снятая галочка снимает и идущий сеанс этого навыка.
#[test]
fn unticking_ends_the_session() {
    let (mut sim, science) = school(CORRIDOR, (5, 1));
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(5);
    assert_eq!(sim.studies_by_rule("a"), Some(science));
    assert!(sim.set_self_study("a", "science", false));
    assert_eq!(sim.studies_by_rule("a"), None, "встал тем же кликом");
    assert!(sim.self_study_of("a").is_empty());
    sim.tick_n(5);
    assert!(!sim.is_studying("a"), "и не вернулся");
}

/// «Учить» поверх правила работает: своя парта коту не помеха, и сеанс
/// становится записью игрока.
#[test]
fn teach_takes_over_from_the_rule() {
    let (mut sim, science) = school(CORRIDOR, (5, 1));
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(5);
    assert_eq!(sim.studies_by_rule("a"), Some(science));
    assert!(
        sim.teach("a", "science"),
        "«все парты заняты» про свою — враньё"
    );
    assert!(sim.is_enrolled("a"), "теперь это запись игрока");
    assert_eq!(sim.studies_by_rule("a"), None);
}

/// Ворота — у команды (§12.93): тема открывает галочку, снятие ворот не
/// спрашивает; домену, которому за партой не учат, галочки нет.
#[test]
fn the_rule_is_gated_by_its_topic() {
    let (mut sim, _) = school(CORRIDOR, (5, 1));
    sim.set_study_gate("curriculum");
    assert!(!sim.set_self_study("a", "science", false), "снимать нечего");
    assert!(!sim.set_self_study("a", "science", true), "тема не изучена");
    sim.set_tech("curriculum");
    assert!(sim.set_self_study("a", "science", true));
    assert!(!sim.set_self_study("a", "science", true), "уже отмечено");
    sim.forget_techs();
    assert!(
        sim.set_self_study("a", "science", false),
        "снятие ворот не спрашивает"
    );

    sim.set_skill("build", &[20, 100]);
    assert!(
        !sim.set_self_study("a", "build", true),
        "за партой не учат — галочки нет"
    );
    assert!(!sim.set_self_study("a", "nonsense", true), "домена нет");
}

/// Галочки переживают загрузку партии.
#[test]
fn the_rule_survives_a_save() {
    let mut live = Sim::new(CORE).expect("рулсет");
    live.set_tech("curriculum");
    assert!(live.set_self_study("excellent", "science", true));
    assert!(live.set_self_study("excellent", "relay", true));
    let want = live.self_study_of("excellent");
    assert_eq!(want.len(), 2);

    let json = live.save().expect("снимок");
    let mut loaded = Sim::load_from(CORE, &json).expect("загрузка");
    assert_eq!(loaded.self_study_of("excellent"), want);
}

/// На боевом рулсете правило стоит за «Автоматизацией» и «Большой партой»:
/// пока учитель один, каждый его час — решение, и принимают его руками.
#[test]
fn the_shipped_ruleset_opens_self_study_after_the_big_desk() {
    let sim = Sim::new(CORE).expect("рулсет");
    assert!(
        sim.auto_gates().contains(&"curriculum".to_string()),
        "у учёбы в простое свои ворота"
    );
    let topic = sim.topic_by_id("curriculum").expect("тема есть");
    for need in ["autonomy", "big_desks"] {
        assert!(
            topic.requires.iter().any(|r| r == need),
            "«Автообучение» не требует `{need}`"
        );
    }
}

/// Учитель упал от усталости посреди урока — и `assign_tutor` того же тика
/// не зовёт ученику правила замену с работы: ученик встаёт, работник работает.
#[test]
fn a_fallen_teacher_is_not_replaced_from_work() {
    let mut sim = sim_from(&["#########", "#.......#", "#a..c..b#", "#########"]);
    let science = sim.set_skill("science", &[20, 100, 300]);
    sim.set_taught(science, 2);
    sim.set_teaches(DESK, science);
    sim.set_solid(TABLE, true);
    sim.tile_rule(LECTERN, |r| r.lectern = true);
    sim.tile_rule(OTHER, |_| {});
    let def = sim.set_structure(vec![vec![Some(DESK), Some(TABLE), Some(LECTERN)]]);
    assert!(sim.place_structure(def as i32, 2, 1, 0), "штамп встал");
    sim.tick_n(80);
    // Бодрость есть, а спать раньше нуля никто не уходит.
    sim.set_needs(100_000, 0, 1);
    sim.set_xp("b", science, 300);
    sim.set_xp("c", science, 300);
    assert!(sim.set_self_study("a", "science", true));
    sim.tick_n(3);
    assert!(sim.is_teaching("b"), "оба простаивали — сел первый по `id`");

    for x in 5..=7 {
        assert!(sim.add_blueprint(x, 1, OTHER as i32));
    }
    sim.tick_n(2);
    assert!(sim.has_assignment("c"), "c строит");

    sim.set_energy("b", 1);
    for _ in 0..5 {
        sim.tick();
        assert!(!sim.is_teaching("c"), "c не сорвали со стройки");
    }
    assert!(!sim.is_teaching("b"), "b упал");
    assert_eq!(sim.studies_by_rule("a"), None, "ученик встал");
}
