//! Парта на три места: ученик, стол, учитель (§12.258).
//!
//! Учит парта, только пока на месте учителя сидит тот, кто знает больше
//! ученика, и доводит до уровня учителя минус один. Учитель садится сам —
//! лучший из тех, кто может научить, — и срывается ради этого с работы; сам он
//! за это не растёт. Учителю не прийти — ученик за парту не садится, а стоит
//! в очереди и работает (§12.344); короткий перерыв учителя (сон) он ждёт за
//! партой.

use super::*;

const DESK: i16 = 1;
const TABLE: i16 = 2;
const LECTERN: i16 = 3;

/// Комната 7×2, парта-штамп `[парта, стол, учитель]` в верхнем ряду с (2, 1),
/// нижний ряд — проход. Порог «Науки» — 20 / 100 / 300, парта сама по себе
/// учит хоть до третьего уровня: предел задаёт учитель.
fn classroom(rows: &[&str]) -> (Sim, usize) {
    let mut sim = sim_from(rows);
    let science = sim.set_skill("science", &[20, 100, 300]);
    sim.set_taught(science, 3);
    sim.set_teaches(DESK, science);
    sim.set_solid(TABLE, true);
    sim.tile_rule(LECTERN, |r| r.lectern = true);
    let def = sim.set_structure(vec![vec![Some(DESK), Some(TABLE), Some(LECTERN)]]);
    assert!(sim.place_structure(def as i32, 2, 1, 0), "штамп встал");
    sim.tick_n(80); // коты строят все три клетки
    assert_eq!(sim.tile(2, 1), DESK, "парта построена");
    assert_eq!(sim.tile(4, 1), LECTERN, "место учителя построено");
    (sim, science)
}

const ROOM: &[&str] = &["#########", "#.......#", "#a.....b#", "#########"];

/// Учить некому — ученик за парту не садится (§12.344): «Учить» ставит
/// запись, и кот остаётся в очереди, а не сидит впустую.
#[test]
fn a_pupil_without_a_teacher_stays_in_the_queue() {
    let (mut sim, science) = classroom(ROOM);
    assert!(sim.teach("a", "science"), "запись, а не отказ");
    sim.tick_n(40);
    assert!(sim.is_enrolled("a"), "приписка стоит");
    assert!(!sim.is_studying("a"), "за парту не сел");
    assert_eq!(sim.xp_of("a", science), 0);
    assert!(!sim.is_teaching("b"), "b учить нечему — он сам ноль");
}

/// Учитель появился — ученик из очереди садится сам, и учитель с ним (§12.344).
#[test]
fn a_queued_pupil_sits_down_once_a_teacher_can_come() {
    let (mut sim, science) = classroom(ROOM);
    assert!(sim.teach("a", "science"));
    sim.tick_n(10);
    assert!(!sim.is_studying("a"), "учителя нет — ждёт в очереди");

    sim.set_xp("b", science, 300);
    sim.tick_n(3);
    assert!(sim.is_studying("a"), "сел за парту");
    assert!(sim.is_teaching("b"), "и учитель пришёл");
    sim.tick_n(20);
    assert!(sim.xp_of("a", science) > 0, "урок идёт");
}

/// Единственный учитель ушёл в отряд посреди урока — ученик встаёт и идёт
/// работать, приписка остаётся (§12.344). Сидеть ему пришлось бы всю вылазку.
#[test]
fn a_pupil_stands_up_when_the_only_teacher_leaves_for_a_raid() {
    let (mut sim, science) = classroom(ROOM);
    sim.set_xp("b", science, 300);
    assert!(sim.teach("a", "science"));
    sim.tick_n(20);
    assert!(sim.is_teaching("b"), "урок идёт");
    assert!(sim.is_studying("a"));

    // Шлюз — под самим учителем, в правом углу прохода.
    sim.set_gate(6, true);
    sim.force_tile(7, 2, 6);
    let m = sim.set_mission(1, 500, &[]);
    sim.launch(m, vec!["b".to_string()]);
    sim.tick_n(3);
    assert!(!sim.is_teaching("b"), "учитель ушёл в отряд");
    assert!(!sim.is_studying("a"), "ученик встал");
    assert!(sim.is_enrolled("a"), "но запись не потерял");
}

/// Учитель садится сам и доводит ученика до своего уровня минус один; сам за
/// это не растёт.
#[test]
fn a_teacher_sits_down_and_teaches_up_to_his_level_minus_one() {
    let (mut sim, science) = classroom(ROOM);
    sim.set_xp("b", science, 300); // третий уровень
    assert!(sim.teach("a", "science"));
    sim.tick_n(20);
    assert!(sim.is_teaching("b"), "учитель сел сам");
    assert_eq!(sim.pos_of("b"), (4, 1), "на своё место у парты");

    sim.tick_n(400);
    assert_eq!(
        sim.level_of("a", science),
        1,
        "«Учить» — один уровень (§12.269)"
    );
    assert!(!sim.is_studying("a"), "и парта свободна");
    assert!(sim.teach("a", "science"), "за вторым — снова");
    sim.tick_n(400);
    assert_eq!(sim.xp_of("a", science), 100, "ровно второй уровень");
    assert_eq!(sim.xp_of("b", science), 300, "учитель за учёбу не растёт");
}

/// Выучил всё, что этот учитель знает, — учитель встаёт: держать его у парты
/// значит молча отнимать у базы работника.
#[test]
fn a_teacher_leaves_when_he_has_nothing_left_to_teach() {
    let (mut sim, science) = classroom(ROOM);
    sim.set_xp("b", science, 100); // второй уровень — учит только до первого
    assert!(sim.teach("a", "science"));
    sim.tick_n(200);
    assert_eq!(sim.xp_of("a", science), 20, "первый уровень");
    assert!(!sim.is_teaching("b"), "учить больше нечему — встал");
    assert_eq!(sim.xp_of("a", science), 20);
}

/// Учитель срывается с работы (§12.258): учить или исследовать — выбор игрока,
/// и сделан он приписью ученика.
#[test]
fn a_teacher_is_pulled_off_his_work() {
    let (mut sim, science) = classroom(ROOM);
    sim.set_xp("b", science, 300);
    // Занимаем b стройкой в дальнем углу.
    assert!(sim.add_blueprint(7, 1, 5));
    sim.tick_n(2);
    assert!(
        sim.has_assignment("b"),
        "b ближе всех к площадке — строит он"
    );
    assert!(sim.teach("a", "science"));
    sim.tick_n(3);
    assert!(sim.is_teaching("b"), "бросил стройку ради ученика");
    assert!(!sim.has_assignment("b"));
}

/// Учит у парты только тот, кто дорос до порога домена (§12.269): кот второго
/// уровня за кафедру не садится, третьего — садится.
#[test]
fn a_level_two_cat_does_not_teach() {
    let (mut sim, science) = classroom(ROOM);
    sim.set_tutor(science, 3);
    sim.set_xp("b", science, 100); // второй уровень
    assert!(sim.teach("a", "science"));
    sim.tick_n(40);
    assert!(!sim.is_teaching("b"), "второго уровня мало");
    assert_eq!(sim.xp_of("a", science), 0);

    sim.set_xp("b", science, 300); // третий
    sim.tick_n(5); // сесть успеет, а ученик за это время уровня не возьмёт
    assert!(sim.is_teaching("b"), "третий — учит");
}

/// Большая парта (§12.287): три парты в ряд, ряд столов и одно место учителя
/// посередине под ними. Комната 7×4, штамп с (3, 1); по бокам и снизу проход.
const HALL: &[&str] = &[
    "#########",
    "#.......#",
    "#.......#",
    "#.......#",
    "#abc.de.#",
    "#########",
];

/// Класс на троих. Пороги «Науки» задаёт тест: короткие — чтобы проверить
/// рост, длинные — чтобы ученики сидели, пока меряют расход учителя.
fn big_class(levels: &[i32]) -> (Sim, usize) {
    let mut sim = sim_from(HALL);
    let science = sim.set_skill("science", levels);
    sim.set_taught(science, 3);
    sim.set_teaches(DESK, science);
    sim.set_solid(TABLE, true);
    sim.tile_rule(LECTERN, |r| r.lectern = true);
    let def = sim.set_structure(vec![
        vec![Some(DESK), Some(DESK), Some(DESK)],
        vec![Some(TABLE), Some(TABLE), Some(TABLE)],
        vec![None, Some(LECTERN), None],
    ]);
    assert!(sim.place_structure(def as i32, 3, 1, 0), "штамп встал");
    sim.tick_n(200); // коты строят все семь клеток
    for x in 3..=5 {
        assert_eq!(sim.tile(x, 1), DESK, "парта ({x}, 1) построена");
    }
    assert_eq!(sim.tile(4, 3), LECTERN, "место учителя построено");
    (sim, science)
}

/// Один учитель учит весь класс: место учителя у объекта одно на три парты.
#[test]
fn one_teacher_teaches_a_whole_class() {
    let (mut sim, science) = big_class(&[20, 100, 300]);
    sim.set_xp("d", science, 300); // третий уровень
    for pupil in ["a", "b", "c"] {
        assert!(sim.teach(pupil, "science"), "{pupil} сел за парту");
    }
    sim.tick_n(400);
    for pupil in ["a", "b", "c"] {
        assert_eq!(sim.level_of(pupil, science), 1, "{pupil} выучился");
    }
    assert_eq!(sim.xp_of("d", science), 300, "учитель за учёбу не растёт");
}

/// Четвёртый ученик большой парты встаёт в очередь и садится к тому же
/// учителю, когда класс доучится (§12.325) — без второго клика игрока.
#[test]
fn a_fourth_pupil_waits_for_the_next_class() {
    let (mut sim, science) = big_class(&[20, 100, 300]);
    sim.set_xp("d", science, 300); // третий уровень
    for pupil in ["a", "b", "c"] {
        assert!(sim.teach(pupil, "science"), "{pupil} сел за парту");
    }
    assert_eq!(sim.teach_gate("e", science), "taken", "мест больше нет");
    assert!(sim.teach("e", "science"), "но запись принята");
    assert!(!sim.is_studying("e"), "и он ждёт");

    sim.tick_n(800);
    for pupil in ["a", "b", "c", "e"] {
        assert_eq!(sim.level_of(pupil, science), 1, "{pupil} выучился");
    }
}

/// К одному месту учителя садится один учитель, сколько бы учеников ни ждало
/// (§12.287): до него на каждого ученика шёл бы свой учитель.
#[test]
fn a_class_gets_one_teacher() {
    let (mut sim, science) = big_class(&[2000, 4000, 6000]);
    sim.set_xp("d", science, 6000);
    sim.set_xp("e", science, 6000);
    for pupil in ["a", "b", "c"] {
        assert!(sim.teach(pupil, "science"));
    }
    for _ in 0..40 {
        sim.tick_n(1);
        let teachers = ["d", "e"]
            .into_iter()
            .filter(|t| sim.is_teaching(t))
            .count();
        assert!(teachers <= 1, "учителей у одного места: {teachers}");
    }
    assert!(
        sim.is_teaching("d") || sim.is_teaching("e"),
        "но один — сел"
    );
}

/// Сколько бодрости учитель тратит за десять тиков урока, когда за партами
/// сидят `pupils` учеников.
fn teacher_toll(pupils: &[&str]) -> i32 {
    let (mut sim, science) = big_class(&[2000, 4000, 6000]);
    sim.set_needs(100_000, 0, 1);
    sim.set_class(50);
    sim.set_xp("d", science, 6000);
    for pupil in pupils {
        assert!(sim.teach(pupil, "science"));
    }
    sim.tick_n(60); // все расселись, учитель на месте
    assert_eq!(sim.pos_of("d"), (4, 3), "учитель на своём месте");
    for pupil in pupils {
        assert_eq!(sim.pos_of(pupil).1, 1, "{pupil} за партой");
    }
    let before = sim.energy_of("d");
    sim.tick_n(10);
    before - sim.energy_of("d")
}

/// Учитель устаёт сильнее с каждым учеником (§12.287): при `class: 50` урок
/// троим стоит вдвое дороже урока одному.
#[test]
fn a_teacher_tires_faster_with_each_pupil() {
    let one = teacher_toll(&["a"]);
    let three = teacher_toll(&["a", "b", "c"]);
    assert!(one > 0, "учитель вообще устаёт");
    assert_eq!(three, one * 2, "трое — вдвое");
}

/// Двух котов послали в одну клетку большой парты (§12.287): второй садится
/// за соседнюю парту того же объекта, а не получает простой приказ «иди».
#[test]
fn a_second_cat_sent_to_a_taken_desk_takes_the_next_seat() {
    let (mut sim, _) = big_class(&[2000, 4000, 6000]);
    assert!(sim.set_target("a", 3, 1));
    assert!(sim.set_target("b", 3, 1));
    assert!(sim.is_studying("a") && sim.is_studying("b"), "учатся оба");
    let (da, db) = (sim.desk_of("a"), sim.desk_of("b"));
    assert_eq!(da, Some((3, 1)), "a — за той, в которую ткнули");
    assert!(
        db.is_some_and(|(x, y)| y == 1 && (4..=5).contains(&x)),
        "b — за соседней партой объекта: {db:?}"
    );
}

/// Парта на два домена (§12.290): к «Науке» классной комнаты прибавлена
/// «Связь» с теми же порогами.
fn two_domain_classroom() -> (Sim, usize, usize) {
    let (mut sim, science) = classroom(ROOM);
    let relay = sim.set_skill("relay", &[20, 100, 300]);
    sim.set_taught(relay, 3);
    sim.add_teaches(DESK, relay);
    (sim, science, relay)
}

/// Одна парта учит и «Связи»: учитель связи садится сам, как учитель науки.
#[test]
fn one_desk_teaches_relay_too() {
    let (mut sim, _, relay) = two_domain_classroom();
    sim.set_xp("b", relay, 300); // третий уровень «Связи»
    assert!(sim.teach("a", "relay"));
    sim.tick_n(400);
    assert_eq!(
        sim.level_of("a", relay),
        1,
        "выучился связи за той же партой"
    );
    assert_eq!(sim.xp_of("b", relay), 300, "учитель за учёбу не растёт");
}

/// Учитель учит своему домену и только ему: знаток «Науки» ученику «Связи»
/// не учитель, и тот ждёт — в очереди, а не за партой (§12.344).
#[test]
fn a_teacher_of_one_domain_does_not_teach_another() {
    let (mut sim, science, relay) = two_domain_classroom();
    sim.set_xp("b", science, 300);
    assert!(sim.teach("a", "relay"));
    sim.tick_n(200);
    assert!(sim.is_enrolled("a"), "ученик в очереди");
    assert!(!sim.is_studying("a"), "учителя связи нет — за парту не сел");
    assert!(!sim.is_teaching("b"), "учёный связи не учит");
    assert_eq!(sim.xp_of("a", relay), 0, "и опыт не растёт");
}

/// Клик по парте на два домена (§12.290): кот учится первому по списку
/// парты, где ему ещё есть куда расти.
#[test]
fn a_click_on_a_desk_picks_a_domain_the_cat_can_still_learn() {
    let (mut sim, science, _) = two_domain_classroom();
    assert!(sim.set_target("a", 2, 1));
    assert_eq!(sim.enrolled_in("a"), Some(science), "первый по списку");

    let (mut sim, science, relay) = two_domain_classroom();
    sim.set_xp("a", science, 300); // «Наука» на потолке парты
    assert!(sim.set_target("a", 2, 1));
    assert_eq!(sim.enrolled_in("a"), Some(relay), "науке учиться некуда");
}

/// Два ученика одного класса, и каждый — нужный учитель другому (§12.258,
/// §12.290): знаток «Науки» сел учиться «Связи», знаток «Связи» — «Науке».
/// Свободного учителя нет, и раньше оба ждали вечно. Ученик без учителя
/// встаёт за кафедру сам — он и так простаивает, — а приписка к парте
/// переживает урок, и после него его учат тоже.
#[test]
fn two_pupils_teach_each_other_in_turn() {
    let (mut sim, science) = big_class(&[20, 100, 300]);
    let relay = sim.set_skill("relay", &[20, 100, 300]);
    sim.set_taught(relay, 3);
    sim.add_teaches(DESK, relay);
    sim.set_xp("a", science, 300);
    sim.set_xp("b", relay, 300);
    assert!(sim.teach("a", "relay"));
    assert!(sim.teach("b", "science"));
    sim.tick_n(1500);
    assert_eq!(sim.level_of("b", science), 1, "b выучился «Науке»");
    assert_eq!(sim.level_of("a", relay), 1, "a выучился «Связи»");
}

/// Кафедру держит учитель «Связи» — ученик «Науки» за эту парту не садится,
/// а ждёт в очереди (§12.326). Сядь он, просидел бы без дела весь чужой урок:
/// учитель у кафедры один и учит одному домену (§12.290).
#[test]
fn a_pupil_does_not_sit_into_another_domains_lesson() {
    let (mut sim, science) = big_class(&[200, 400, 600]);
    let relay = sim.set_skill("relay", &[200, 400, 600]);
    sim.set_taught(relay, 3);
    sim.add_teaches(DESK, relay);
    sim.set_xp("d", relay, 600); // учитель «Связи»
    sim.set_xp("e", science, 600); // учитель «Науки»
    assert!(sim.teach("a", "relay"));
    sim.tick_n(30);
    assert!(sim.is_teaching("d"), "урок «Связи» идёт");

    assert_eq!(
        sim.teach_gate("b", science),
        "taken",
        "место есть, но урок чужой"
    );
    assert!(sim.teach("b", "science"), "запись встала в очередь");
    sim.tick_n(20);
    assert!(!sim.is_studying("b"), "за парту под чужой урок он не сел");
    assert!(
        !sim.is_teaching("e"),
        "и учителя «Науки» к занятой кафедре не звали"
    );

    sim.tick_n(1200);
    assert_eq!(sim.level_of("a", relay), 1, "урок «Связи» закончился");
    assert_eq!(sim.level_of("b", science), 1, "и «Наука» пришла следом");
}

/// Ученик «Науки» уже сидел за партой и ждал учителя, а кафедру занял учитель
/// «Связи» — ученик встаёт и возвращается в очередь, приписка остаётся
/// (§12.326). Иначе он простоял бы весь чужой урок.
#[test]
fn a_waiting_pupil_stands_up_when_another_lesson_starts() {
    let (mut sim, science) = big_class(&[200, 400, 600]);
    let relay = sim.set_skill("relay", &[200, 400, 600]);
    sim.set_taught(relay, 3);
    sim.add_teaches(DESK, relay);
    // Учитель «Науки» есть, но спит: короткий перерыв ученик ждёт за партой
    // (§12.344), и как раз тогда кафедру может занять чужой урок.
    sim.set_xp("c", science, 600);
    sim.set_needs(1000, 300, 1);
    sim.set_energy("c", 0); // лежанок нет — валится спать на полу
    sim.tick_n(3);
    assert!(sim.is_resting("c"), "учитель «Науки» спит");
    assert!(sim.teach("b", "science"));
    sim.tick_n(10);
    assert!(sim.is_studying("b"), "сел ждать");

    sim.set_xp("d", relay, 600);
    assert!(sim.teach("a", "relay"));
    sim.tick_n(30);
    assert!(sim.is_teaching("d"), "урок «Связи» начался");
    assert!(!sim.is_studying("b"), "ученик «Науки» встал");
    assert!(sim.is_enrolled("b"), "но запись не потерял");
}

/// Недостроенная парта не учит (§12.282): клетки штампа строятся порознь, и
/// место ученика вставало раньше стола — класс учил, не будучи построен.
#[test]
fn an_unfinished_class_does_not_teach() {
    let (mut sim, science) = big_class(&[20, 100, 300]);
    sim.set_xp("d", science, 300);
    sim.force_tile(5, 2, 0); // один стол класса ещё не стоит
    assert_eq!(sim.teach_gate("a", science), "nodesk", "парты ещё нет");
    assert!(!sim.teach("a", "science"), "и за неё не сажают");
    sim.tick_n(200);
    assert_eq!(sim.xp_of("a", science), 0, "и ничему не учит");
}
