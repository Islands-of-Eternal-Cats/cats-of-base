//! Вещи (§12.317): штуки предметов со своим состоянием — прочностью (`Wear`) и
//! зарядами (`Charges`).
//!
//! **Счёт остаётся главным.** Сколько вещей и где, говорят `Stack`, `Carrying`,
//! `Gear` и `delivered`; сущность `Thing` несёт только то, чего число сказать не
//! может, — состояние штуки. Учёт (склад, бронь, цены, цели) о вещах не знает.
//!
//! Сходятся они **одной системой** — `reconcile_things`: где штук больше, чем
//! вещей, рождается свежая, где вещей больше, — уходит самая потраченная. Поэтому
//! у рождения (покупка, мастерская, добыча, найм) и у траты (продажа, плата,
//! разбор) крючков нет. Состояние переносят **явные переходы** (`shift`,
//! `shift_world`) там, где штуку перекладывают, а не тратят: пропущенный переход
//! не ломает счёт, но рождает на новом месте свежую вещь вместо поношенной.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::components::{
    Blueprint, Carrying, Charges, Craft, Deal, Gear, ItemRules, Place, Position, Research, Stack,
    Thing, Wear,
};

/// Какую вещь брать из нескольких на одном месте (§12.317).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    /// Самую свежую, при равной прочности — самую заряженную: так надевают.
    Freshest,
    /// Самую потраченную: так платят, тратят заряд и перекладывают.
    Spent,
}

/// Состояние вещи ключом сравнения: прочность, потом заряды. У параметра,
/// которого у вещи нет, ключ ноль — сравнивают только вещи одного предмета, и у
/// них состав параметров один.
pub(crate) fn state_key(wear: Option<&Wear>, charges: Option<&Charges>) -> (i32, i32) {
    (wear.map_or(0, |w| w.0), charges.map_or(0, |c| c.0))
}

/// Родить новую вещь: параметры полные, как записано в рулсете.
pub(crate) fn spawn_fresh(commands: &mut Commands, items: &ItemRules, item: usize, place: Place) {
    let mut e = commands.spawn((Thing { item }, place));
    let wear = items.wear_of(item);
    if wear > 0 {
        e.insert(Wear(wear));
    }
    let uses = items.uses_of(item);
    if uses > 0 {
        e.insert(Charges(uses));
    }
}

/// Запрос вещей для переходов внутри систем.
pub(crate) type ThingsMut<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Thing,
        &'static mut Place,
        Option<&'static Wear>,
        Option<&'static Charges>,
    ),
>;

/// Выбрать до `n` вещей предмета `item` на месте `at` в порядке `pick`.
///
/// Вещи с равным состоянием взаимозаменяемы, и какая из них взята, в мире не
/// видно, — поэтому сортировки по ключу довольно для детерминизма (§11), хотя
/// порядок обхода ECS тут свой у каждого мира.
fn choose(mut found: Vec<(Entity, (i32, i32))>, n: i32, pick: Pick) -> Vec<Entity> {
    match pick {
        Pick::Freshest => found.sort_by_key(|&(_, k)| std::cmp::Reverse(k)),
        Pick::Spent => found.sort_by_key(|&(_, k)| k),
    }
    found
        .into_iter()
        .take(n.max(0) as usize)
        .map(|(e, _)| e)
        .collect()
}

/// Переложить до `n` вещей предмета `item` с места `from` на `to` — внутри
/// системы. Счёт правит вызывающий, здесь едет только состояние.
///
/// Вещей на месте может оказаться меньше, чем штук: только что рождённую кучу
/// сверка ещё не догнала. Тогда переезжает сколько есть, а недостачу на новом
/// месте сверка родит свежей — то есть ровно то, что лежало бы там и без неё.
pub(crate) fn shift(
    things: &mut ThingsMut,
    from: Place,
    to: Place,
    item: usize,
    n: i32,
    pick: Pick,
) -> Vec<Entity> {
    if n <= 0 || from == to {
        return Vec::new();
    }
    let found: Vec<(Entity, (i32, i32))> = things
        .iter()
        .filter(|(_, t, p, ..)| t.item == item && **p == from)
        .map(|(e, _, _, w, c)| (e, state_key(w, c)))
        .collect();
    let moved = choose(found, n, pick);
    for &e in &moved {
        if let Ok((_, _, mut place, ..)) = things.get_mut(e) {
            *place = to;
        }
    }
    moved
}

/// То же, что `shift`, по миру — для фасада.
pub(crate) fn shift_world(
    world: &mut World,
    from: Place,
    to: Place,
    item: usize,
    n: i32,
    pick: Pick,
) -> Vec<Entity> {
    if n <= 0 || from == to {
        return Vec::new();
    }
    let mut q = world.query::<(Entity, &Thing, &Place, Option<&Wear>, Option<&Charges>)>();
    let found: Vec<(Entity, (i32, i32))> = q
        .iter(world)
        .filter(|(_, t, p, ..)| t.item == item && **p == from)
        .map(|(e, _, _, w, c)| (e, state_key(w, c)))
        .collect();
    let moved = choose(found, n, pick);
    for &e in &moved {
        if let Some(mut place) = world.get_mut::<Place>(e) {
            *place = to;
        }
    }
    moved
}

/// Сколько штук вещей-предметов лежит на каждом месте **по счёту** — то, с чем
/// сверка сводит вещи. Одно выражение на систему и на сторожа тестов.
pub(crate) fn wanted<'a>(
    items: &ItemRules,
    stacks: impl Iterator<Item = (&'a Position, &'a Stack)>,
    paws: impl Iterator<Item = (Entity, &'a Carrying)>,
    worn: impl Iterator<Item = (Entity, &'a Gear)>,
    held: impl Iterator<Item = (Entity, Vec<(usize, i32)>)>,
) -> BTreeMap<(Place, usize), i32> {
    let mut want: BTreeMap<(Place, usize), i32> = BTreeMap::new();
    let mut add = |place: Place, item: usize, n: i32| {
        if n > 0 && items.is_thing(item) {
            *want.entry((place, item)).or_default() += n;
        }
    };
    for (pos, s) in stacks {
        add(Place::Floor(pos.x, pos.y), s.item, s.count);
    }
    for (cat, c) in paws {
        add(Place::Paws(cat), c.item, c.count);
    }
    for (cat, g) in worn {
        for item in g.items() {
            add(Place::Worn(cat), item, 1);
        }
    }
    for (box_e, inside) in held {
        for (item, n) in inside {
            add(Place::Held(box_e), item, n);
        }
    }
    want
}

/// Что сдано в контейнер по счёту: набор `delivered` у площадки, станка и
/// лаборатории, счётчик у продажи.
pub(crate) fn held_of(
    bp: Option<&Blueprint>,
    craft: Option<&Craft>,
    topic: Option<&Research>,
    deal: Option<&Deal>,
) -> Vec<(usize, i32)> {
    let mut out: Vec<(usize, i32)> = Vec::new();
    out.extend(bp.into_iter().flat_map(|b| b.delivered.iter().copied()));
    out.extend(craft.into_iter().flat_map(|c| c.delivered.iter().copied()));
    out.extend(topic.into_iter().flat_map(|t| t.delivered.iter().copied()));
    if let Some(d) = deal
        && !d.buying
    {
        out.push((d.item, d.delivered));
    }
    out
}

/// Свести вещи со счётом (§12.317). Стоит сразу за `settle_stacks`: кучи уже
/// легли на свои клетки и слились, и вещи пола ищутся по клетке.
///
/// Недостача — вещь родилась: свежая. Излишек — вещь потрачена: уходит самая
/// потраченная. Место, которого больше нет (кот ушёл из мира, контейнер закрыт),
/// — излишек целиком.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn reconcile_things(
    items: Res<ItemRules>,
    mut commands: Commands,
    stacks: Query<(&Position, &Stack)>,
    paws: Query<(Entity, &Carrying)>,
    worn: Query<(Entity, &Gear)>,
    boxes: Query<
        (
            Entity,
            Option<&Blueprint>,
            Option<&Craft>,
            Option<&Research>,
            Option<&Deal>,
        ),
        Or<(With<Blueprint>, With<Craft>, With<Research>, With<Deal>)>,
    >,
    things: Query<(Entity, &Thing, &Place, Option<&Wear>, Option<&Charges>)>,
) {
    let want = wanted(
        &items,
        stacks.iter(),
        paws.iter(),
        worn.iter(),
        boxes
            .iter()
            .map(|(e, bp, craft, topic, deal)| (e, held_of(bp, craft, topic, deal))),
    );
    for (key, change) in plan(&want, things.iter()) {
        match change {
            Change::Born(n) => {
                for _ in 0..n {
                    spawn_fresh(&mut commands, &items, key.1, key.0);
                }
            }
            Change::Gone(list) => {
                for e in list {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}

/// Что сделать с одним местом, чтобы вещи сошлись со счётом.
enum Change {
    Born(i32),
    Gone(Vec<Entity>),
}

/// Разность счёта и вещей по местам.
fn plan<'a>(
    want: &BTreeMap<(Place, usize), i32>,
    things: impl Iterator<
        Item = (
            Entity,
            &'a Thing,
            &'a Place,
            Option<&'a Wear>,
            Option<&'a Charges>,
        ),
    >,
) -> Vec<((Place, usize), Change)> {
    let mut have: BTreeMap<(Place, usize), Vec<(Entity, (i32, i32))>> = BTreeMap::new();
    for (e, t, p, w, c) in things {
        have.entry((*p, t.item))
            .or_default()
            .push((e, state_key(w, c)));
    }
    let mut out = Vec::new();
    for (&key, &n) in want {
        let got = have.get(&key).map_or(0, |v| v.len() as i32);
        if got < n {
            out.push((key, Change::Born(n - got)));
        }
    }
    for (key, list) in have {
        let n = want.get(&key).copied().unwrap_or(0);
        let surplus = list.len() as i32 - n;
        if surplus > 0 {
            out.push((key, Change::Gone(choose(list, surplus, Pick::Spent))));
        }
    }
    out
}

/// Свести вещи со счётом по миру — для конструктора и загрузки, как прогон
/// `note_seen` и `note_clutter` в `Sim::new`.
pub(crate) fn reconcile_world(world: &mut World) {
    use bevy_ecs::system::RunSystemOnce;
    let _ = world.run_system_once(reconcile_things);
}

/// Места, где вещи и счёт расходятся: `(место, предмет, по счёту, вещей)`.
/// Пусто — сошлось. Сторож тестов (§12.317).
#[cfg(test)]
pub(crate) fn mismatches(world: &mut World) -> Vec<(Place, usize, i32, i32)> {
    let items = world.resource::<ItemRules>().clone();
    let stacks: Vec<(Position, Stack)> = world
        .query::<(&Position, &Stack)>()
        .iter(world)
        .map(|(p, s)| {
            (
                Position { x: p.x, y: p.y },
                Stack {
                    item: s.item,
                    count: s.count,
                },
            )
        })
        .collect();
    let paws: Vec<(Entity, Carrying)> = world
        .query::<(Entity, &Carrying)>()
        .iter(world)
        .map(|(e, c)| {
            (
                e,
                Carrying {
                    item: c.item,
                    count: c.count,
                },
            )
        })
        .collect();
    let worn: Vec<(Entity, Gear)> = world
        .query::<(Entity, &Gear)>()
        .iter(world)
        .map(|(e, g)| (e, g.clone()))
        .collect();
    let held: Vec<(Entity, Vec<(usize, i32)>)> = world
        .query::<(
            Entity,
            Option<&Blueprint>,
            Option<&Craft>,
            Option<&Research>,
            Option<&Deal>,
        )>()
        .iter(world)
        .map(|(e, bp, craft, topic, deal)| (e, held_of(bp, craft, topic, deal)))
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let want = wanted(
        &items,
        stacks.iter().map(|(p, s)| (p, s)),
        paws.iter().map(|(e, c)| (*e, c)),
        worn.iter().map(|(e, g)| (*e, g)),
        held.into_iter(),
    );
    let mut have: BTreeMap<(Place, usize), i32> = BTreeMap::new();
    for (t, p) in world.query::<(&Thing, &Place)>().iter(world) {
        *have.entry((*p, t.item)).or_default() += 1;
    }
    let mut keys: Vec<(Place, usize)> = want.keys().chain(have.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let w = want.get(&k).copied().unwrap_or(0);
            let h = have.get(&k).copied().unwrap_or(0);
            (w != h).then_some((k.0, k.1, w, h))
        })
        .collect()
}
