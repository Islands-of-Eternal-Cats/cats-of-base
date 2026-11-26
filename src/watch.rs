//! Сторож (§12.462): панель тревог справа. Систем у него нет и быть не
//! должно — это наблюдатель, как `check_goals` и `note_news`: он читает
//! сложившийся тик и мир не меняет. Строка встаёт **только когда решение игрока
//! не исполняется или коту плохо**, а не когда на базе чего-то нет: «нет
//! склада» в начале партии — нормальное состояние, а не беда.
//!
//! Каждая тревога — это уже существующий отказ ядра, вынесенный наружу, и
//! считается **тем же выражением**, каким его считает работа (инвариант 14):
//! «склад полон» — те же `best_tiers`, что у уборки, «лежанки нет» — те же
//! свободные места, что у `assign_rest`. Разойдись они — сторож кричал бы о
//! том, что коты уже чинят сами, или молчал бы о том, что не чинится.

use crate::components::{Bins, TileRules};
use crate::hauling::{best_tiers, stock_grid};
use crate::map::BaseMap;

/// Сколько помеченного на склад лежит без места (§12.462): уборка его хочет,
/// но ни одно хранилище, принимающее этот тип, не вместит ни штуки.
///
/// Считается **только при обычном складе на базе** (ёмкость есть, яруса нет):
/// без склада вовсе это «склада нет», а не «склад полон», — то есть
/// состояние начала партии, о котором сторож молчит. Ящики (ярус > 0) берут
/// лишь назначенное, и лом, которому место только на складе, при одних ящиках
/// полным складом назвать нельзя.
///
/// `piles` — все кучи мира: `(клетка, предмет, сколько, помечена ли)`, та же
/// форма, что у `tidy_pending`.
pub(crate) fn store_short(
    map: &BaseMap,
    rules: &TileRules,
    bins: &Bins,
    piles: &[((i32, i32), usize, i32, bool)],
) -> i32 {
    let has_store = map
        .cells
        .iter()
        .any(|&t| rules.capacity_of(t) > 0 && rules.priority_of(t) == 0);
    if !has_store {
        return 0;
    }
    let stock = stock_grid(map, piles.iter().map(|&(xy, _, n, _)| (xy, n)));
    let mut items: Vec<usize> = piles
        .iter()
        .filter(|&&(.., marked)| marked)
        .map(|&(_, item, ..)| item)
        .collect();
    items.sort_unstable();
    items.dedup();
    let best = best_tiers(map, rules, bins, &stock, &items);
    piles
        .iter()
        .filter(|&&(_, item, _, marked)| {
            marked && best.iter().any(|&(i, top)| i == item && top.is_none())
        })
        .map(|&(.., n, _)| n)
        .sum()
}

/// Свободные лежанки — **одно выражение** на `assign_rest` и сторожа (§12.462).
///
/// Занято и то, к чему идут, и то, на чём лежат: кот, свалившийся прямо на
/// лежанку, места в `Rest` не держит, но занимает его собой. Дремлющий
/// (§12.52) лежанку не держит — его сгонит пришедший спать.
///
/// `resting` — спящие: `(где лежит, к какой лежанке идёт)`.
pub(crate) fn free_beds(
    map: &BaseMap,
    rules: &TileRules,
    resting: &[((i32, i32), Option<(i32, i32)>)],
) -> Vec<(i32, i32)> {
    let mut taken: Vec<(i32, i32)> = Vec::new();
    for &(pos, spot) in resting {
        taken.extend(spot);
        if rules.rest_of(map.tile_at(pos.0, pos.1)) > 0 {
            taken.push(pos);
        }
    }
    (0..map.height)
        .flat_map(|y| (0..map.width).map(move |x| (x, y)))
        .filter(|&(x, y)| rules.rest_of(map.tile_at(x, y)) > 0)
        .filter(|cell| !taken.contains(cell))
        .collect()
}
