//! Сторож справочника (§12.204): **ключ статьи обязан существовать в рулсете, а
//! ссылка — вести в написанную статью**.
//!
//! Ловит он не код, а контент, и цена ошибки тут тихая. Опечатка в ключе
//! (`tile:strorage`) не роняет ничего: значок «?» просто не появляется у
//! склада, и заметить это можно, лишь пересчитав значки глазами. Битая ссылка
//! молча превращается в обычное слово — статья ссылается в пустоту, и ни игрок,
//! ни автор об этом не узнают.
//!
//! ⚠️ Разбираем **сам YAML**, а не мир `Sim::new`, по доводу `tech_tree`
//! (§12.134): конструктор схлопывает пропавшие `id` через `filter_map`, то есть
//! глотает как раз те опечатки, которые сторож и ловит.
//!
//! ⚠️ **Список файлов здесь второй** — первый живёт в `wiki.js`, — и это
//! безопасно ровно потому, что рядом стоит проверка полноты: забудь подключить
//! файл здесь, и `every_palette_entry_has_an_article` назовёт поимённо всё, что
//! в нём написано. Обратное (файл есть в тесте, нет в игре) поймать нечем, и
//! это единственная дыра, оставленная сознательно: `include_str!` требует
//! статичных путей, а каталог из теста не читается.

use std::collections::BTreeSet;

use crate::ruleset::Ruleset;

const CORE: &str = include_str!("../../assets/rulesets/core.yaml");

/// Разделы справочника — по одному на файл, дословно `FILES` в `wiki.js`.
const WIKI: &[&str] = &[
    include_str!("../../assets/wiki/lore.md"),
    include_str!("../../assets/wiki/tiles.md"),
    include_str!("../../assets/wiki/items.md"),
    include_str!("../../assets/wiki/research.md"),
    include_str!("../../assets/wiki/recipes.md"),
    include_str!("../../assets/wiki/missions.md"),
    include_str!("../../assets/wiki/recruits.md"),
    include_str!("../../assets/wiki/factions.md"),
    include_str!("../../assets/wiki/rules.md"),
    include_str!("../../assets/wiki/skills.md"),
];

/// Правила игрока (§12.93). Палитра у них не в рулсете, а в `automation:`, и
/// ключи те же, что у `AutoRules::gates`.
const RULE_KEYS: [&str; 3] = ["sales", "crafting", "raids"];

fn shipped() -> Ruleset {
    serde_yaml::from_str(CORE).expect("рулсет читается")
}

/// Ключи всех написанных статей — `вид:id`.
fn keys() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in WIKI {
        for line in file.lines() {
            if let Some(key) = article_key(line) {
                out.insert(key);
            }
        }
    }
    out
}

/// Заголовок статьи: `## вид:id | Название`. Название необязательно.
fn article_key(line: &str) -> Option<String> {
    let rest = line.strip_prefix("## ")?;
    let key = rest.split('|').next()?.trim();
    let (kind, id) = key.split_once(':')?;
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
    };
    (ok(kind) && ok(id)).then(|| key.to_string())
}

/// Все ссылки `[[вид:id]]` / `[[вид:id|слово]]` с именем файла для внятного
/// отчёта: «битая ссылка» без адреса ищется потом руками по десяти файлам.
fn links() -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, file) in WIKI.iter().enumerate() {
        let mut rest = *file;
        while let Some(at) = rest.find("[[") {
            rest = &rest[at + 2..];
            let Some(end) = rest.find("]]") else { break };
            let body = &rest[..end];
            rest = &rest[end + 2..];
            let target = body.split('|').next().unwrap_or("").trim();
            if target.contains(':') {
                out.push((n, target.to_string()));
            }
        }
    }
    out
}

/// Что вообще бывает записью справочника: вид → все `id` его палитры.
/// `lore` палитры не имеет — это статьи о мире, ни к чему не привязанные.
fn palettes(rs: &Ruleset) -> Vec<(&'static str, Vec<String>)> {
    let ids = |xs: Vec<&String>| xs.into_iter().cloned().collect::<Vec<_>>();
    vec![
        // Внутренность объекта (`internal`, §12.163) своей статьи не получает:
        // игрок её не ставит и отдельно не видит, о ней рассказывает объект.
        (
            "tile",
            ids(rs
                .tiles
                .iter()
                .filter(|x| !x.internal)
                .map(|x| &x.id)
                .collect()),
        ),
        (
            "structure",
            ids(rs.structures.iter().map(|x| &x.id).collect()),
        ),
        ("item", ids(rs.items.iter().map(|x| &x.id).collect())),
        ("topic", ids(rs.research.iter().map(|x| &x.id).collect())),
        ("recipe", ids(rs.recipes.iter().map(|x| &x.id).collect())),
        ("raid", ids(rs.missions.iter().map(|x| &x.id).collect())),
        ("recruit", ids(rs.recruits.iter().map(|x| &x.id).collect())),
        ("faction", ids(rs.factions.iter().map(|x| &x.id).collect())),
        ("skill", ids(rs.skills.iter().map(|x| &x.id).collect())),
        ("stat", ids(rs.stats.iter().map(|x| &x.id).collect())),
        ("perk", ids(rs.perks.iter().map(|x| &x.id).collect())),
        ("rule", RULE_KEYS.iter().map(|s| (*s).to_string()).collect()),
    ]
}

/// Ключ статьи ведёт в существующую запись рулсета.
///
/// Ошибка тут тихая вдвойне: значок «?» не появляется, а статья остаётся
/// висеть в оглавлении — то есть выглядит написанной и при этом недостижима
/// оттуда, где о ней спросили бы.
#[test]
fn every_article_key_names_a_real_entry() {
    let rs = shipped();
    let pal = palettes(&rs);
    for key in keys() {
        let (kind, id) = key.split_once(':').expect("ключ вида «вид:id»");
        if kind == "lore" {
            continue;
        }
        let Some((_, ids)) = pal.iter().find(|(k, _)| *k == kind) else {
            panic!("статья «{key}»: вида «{kind}» в игре нет");
        };
        assert!(
            ids.iter().any(|x| x == id),
            "статья «{key}»: в палитре «{kind}» такого id нет"
        );
    }
}

/// Ссылка ведёт в написанную статью. Битая молча становится обычным словом —
/// то есть обещание перехода исчезает, и заметить это можно только тыкая.
#[test]
fn every_link_points_at_a_written_article() {
    let all = keys();
    for (file, target) in links() {
        assert!(
            all.contains(&target),
            "файл №{file}: ссылка на «{target}», а статьи с таким ключом нет"
        );
    }
}

/// У каждой записи боевых палитр есть статья.
///
/// Это не педантизм: значок «?» стоит **только там, где статья есть**
/// (§12.204), и запись без неё выглядит для игрока ровно так же, как запись,
/// о которой игре нечего сказать. Разница между «не написали» и «нечего
/// сказать» видна отсюда и больше ниоткуда.
///
/// Он же ловит забытый в `WIKI` файл: пропал раздел — весь его список встанет
/// здесь поимённо.
#[test]
fn every_palette_entry_has_an_article() {
    let rs = shipped();
    let all = keys();
    let mut missing = Vec::new();
    for (kind, ids) in palettes(&rs) {
        for id in ids {
            let key = format!("{kind}:{id}");
            // Грань (§12.265) рассказана статьёй владельца.
            if !all.contains(&key) && wiki_owner(&rs, &key).is_none() {
                missing.push(key);
            }
        }
    }
    assert!(missing.is_empty(), "без статьи остались: {missing:?}");
}

/// Заголовок статьи не бывает пустым: без него окно печатает `id`, то есть
/// «sample_lore» вместо «Свойства образца», — и это выглядит поломкой, а не
/// умолчанием. Умолчание тут законно только у записей палитры, где имя даст
/// сам рулсет.
#[test]
fn a_lore_article_always_names_itself() {
    for file in WIKI {
        for line in file.lines() {
            let Some(key) = article_key(line) else {
                continue;
            };
            if !key.starts_with("lore:") {
                continue;
            }
            assert!(
                line.contains('|') && line.split('|').nth(1).is_some_and(|t| !t.trim().is_empty()),
                "статья «{key}» о мире: у неё нет заголовка, а взять его негде"
            );
        }
    }
}

/// Ключ статьи встречается один раз на весь справочник. Повтор не роняет
/// ничего: разборщик в `wiki.js` молча оставляет последнюю статью, и первая —
/// вместе с её короткой версией до маркера `---понято---` — пропадает. Так
/// «Анализатор» однажды открылся полным текстом до изучения своей темы.
#[test]
fn every_article_key_is_written_once() {
    let mut seen = BTreeSet::new();
    let twice: Vec<String> = WIKI
        .iter()
        .flat_map(|file| file.lines())
        .filter_map(article_key)
        .filter(|key| !seen.insert(key.clone()))
        .collect();
    assert!(twice.is_empty(), "статьи с повторным ключом: {twice:?}");
}

/// Кто владеет статьёй (§12.265): одна вещь — одна статья. Рецепт — грань
/// своего предмета (разбор — входа, прочие — выхода); тема — грань того
/// единственного, что она открывает (рецепты считаются их предметом), или
/// одноимённой постройки (§12.251). `None` — статья своя.
///
/// ⚠️ Дословно `wikiOwner` в `main.js`: разойдись они, и вид переадресует на
/// статью, которую сторож не требует, или потребует статью, которой вид не
/// покажет.
fn wiki_owner(rs: &Ruleset, key: &str) -> Option<String> {
    let (kind, id) = key.split_once(':')?;
    let recipe_owner = |r: &crate::ruleset::RecipeDef| {
        let side = if r.salvage { &r.cost } else { &r.gives };
        rs.items
            .iter()
            .find(|it| side.contains_key(&it.id))
            .map(|it| format!("item:{}", it.id))
    };
    match kind {
        "recipe" => recipe_owner(rs.recipes.iter().find(|r| r.id == id)?),
        "topic" => {
            let t = rs.research.iter().find(|t| t.id == id)?;
            let mut opens: Vec<Option<String>> = Vec::new();
            // Внутренность объекта (§12.163) своей статьи не имеет — её открытие
            // и есть открытие объекта, в который она входит. Но только объекта
            // **без своей технологии** (§12.287): большую парту открывает
            // «Большая парта», хотя клетки у неё те же, что у парты.
            let inner = |t: &str| {
                rs.tiles
                    .iter()
                    .any(|x| x.id == t && x.internal && x.tech == id)
            };
            opens.extend(
                rs.tiles
                    .iter()
                    .filter(|x| x.tech == id && !x.internal)
                    .map(|x| Some(format!("tile:{}", x.id))),
            );
            opens.extend(
                rs.structures
                    .iter()
                    .filter(|x| {
                        x.tech == id
                            || (x.tech.is_empty()
                                && x.cells.iter().flatten().flatten().any(|c| inner(c)))
                    })
                    .map(|x| Some(format!("structure:{}", x.id))),
            );
            opens.extend(rs.missions.iter().filter(|x| x.tech == id).map(|_| None));
            opens.extend(
                rs.research
                    .iter()
                    .filter(|x| x.requires.iter().any(|r| r == id))
                    .map(|_| None),
            );
            opens.extend(
                rs.recipes
                    .iter()
                    .filter(|x| x.requires.iter().any(|r| r == id))
                    .map(recipe_owner),
            );
            opens.extend(
                rs.items
                    .iter()
                    .filter(|x| x.requires.iter().any(|r| r == id))
                    .map(|x| Some(format!("item:{}", x.id))),
            );
            let a = &rs.automation;
            for (k, tech) in [
                ("sales", &a.sales),
                ("crafting", &a.crafting),
                ("raids", &a.raids),
            ] {
                if tech == id {
                    opens.push(Some(format!("rule:{k}")));
                }
            }
            let one: BTreeSet<Option<String>> = opens.into_iter().collect();
            if one.len() == 1
                && let Some(Some(owner)) = one.into_iter().next()
            {
                return Some(owner);
            }
            let name = if t.label.is_empty() { &t.id } else { &t.label };
            let same = |l: &String, i: &String| if l.is_empty() { i == name } else { l == name };
            rs.tiles
                .iter()
                .find(|x| x.tech == id && same(&x.label, &x.id))
                .map(|x| format!("tile:{}", x.id))
                .or_else(|| {
                    rs.structures
                        .iter()
                        .find(|x| x.tech == id && same(&x.label, &x.id))
                        .map(|x| format!("structure:{}", x.id))
                })
        }
        _ => None,
    }
}

/// Грань (§12.265) своей статьи не имеет: её текст не показывается никогда —
/// вид переадресует ключ владельцу, — то есть лежал бы в файле мёртвым и
/// расходился с тем, что игрок читает.
#[test]
fn a_facet_has_no_article_of_its_own() {
    let rs = shipped();
    let written: Vec<String> = keys()
        .into_iter()
        .filter(|k| wiki_owner(&rs, k).is_some())
        .collect();
    assert!(
        written.is_empty(),
        "грани с собственной статьёй: {written:?}"
    );
}

/// Ссылка на грань ведёт к владельцу — сторожу это невидимо, поэтому ссылку
/// ставят сразу на владельца: `[[topic:x]]` на грань сработал бы в игре, но
/// выглядел бы битым здесь.
#[test]
fn a_facet_owner_is_written() {
    let rs = shipped();
    let all = keys();
    for (kind, ids) in palettes(&rs) {
        for id in ids {
            let key = format!("{kind}:{id}");
            if let Some(owner) = wiki_owner(&rs, &key) {
                assert!(
                    all.contains(&owner),
                    "грань «{key}»: у владельца «{owner}» нет статьи"
                );
            }
        }
    }
}

/// Открыто ли с нулевого тика: ворот нет вовсе (§12.273).
///
/// ⚠️ Дословно ворота `wikiOwnGate` и `loreOpen` в `main.js`, сведённые к
/// стартовому миру: разойдись они, и сторож пропустит голую ссылку, которую вид
/// спрячет только словом, — или потребует кусок у того, что видно сразу.
fn open_at_start(rs: &Ruleset, key: &str) -> bool {
    let Some((kind, id)) = key.split_once(':') else {
        return false;
    };
    let tile_free = |t: &str| {
        rs.tiles.iter().any(|x| {
            x.id == t
                && x.tech.is_empty()
                && x.sighted.is_empty()
                && x.mastered.is_empty()
                && x.after.is_empty()
        })
    };
    match kind {
        "tile" => tile_free(id),
        "structure" => rs.structures.iter().any(|s| {
            s.id == id
                && s.tech.is_empty()
                && s.cells
                    .iter()
                    .flatten()
                    .flatten()
                    // Свободный слот (§12.283) — не тайл: штамп его не строит.
                    .filter(|c| c.as_str() != crate::ruleset::SLOT)
                    .all(|c| tile_free(c))
        }),
        // Предмет открывается находкой: с нулевого тика видно стартовый запас и
        // то, что надето на стартовых котах. Непонятый — только короткой версией,
        // поэтому `requires` закрывает его тоже.
        "item" => {
            let found = rs.stock.iter().any(|s| s.item == id)
                || rs.units.iter().any(|u| u.gear.iter().any(|g| g == id));
            found && rs.items.iter().any(|x| x.id == id && x.requires.is_empty())
        }
        "recipe" => rs
            .recipes
            .iter()
            .any(|r| r.id == id && r.requires.is_empty()),
        "raid" => rs.missions.iter().any(|m| {
            m.id == id && m.requires == 0 && m.tech.is_empty() && m.needs.is_empty() && !m.rescue
        }),
        "recruit" => rs
            .recruits
            .iter()
            .any(|r| r.id == id && r.requires == 0 && r.needs.is_empty()),
        "skill" | "stat" | "perk" => true,
        "lore" => !matches!(id, "science" | "trade" | "standing" | "fame"),
        _ => false,
    }
}

/// Какими технологиями открывается вещь, если только ими; `None` — у неё есть
/// и другие ворота (находка, известность, фракция), и технологией их не
/// выразить. Тема открывает статью изученной, то есть своей же технологией.
fn tech_gate(rs: &Ruleset, key: &str) -> Option<BTreeSet<String>> {
    let (kind, id) = key.split_once(':')?;
    let one = |t: &String| (!t.is_empty()).then(|| BTreeSet::from([t.clone()]));
    match kind {
        "tile" => rs
            .tiles
            .iter()
            .find(|x| {
                x.id == id && x.sighted.is_empty() && x.mastered.is_empty() && x.after.is_empty()
            })
            .and_then(|x| one(&x.tech)),
        // Ворота объекта — его технология **и ворота всех клеток**, как у
        // `Sim::structure_is_open` (§12.258): парту открывает «Наставничество»
        // её клетки, большую парту — обе темы. Клетка с воротами не по науке
        // делает их не только технологическими — тогда ответа нет.
        "structure" => {
            let x = rs.structures.iter().find(|x| x.id == id)?;
            let mut gate: BTreeSet<String> = one(&x.tech).unwrap_or_default();
            for cell in x.cells.iter().flatten().flatten() {
                let Some(t) = rs.tiles.iter().find(|t| &t.id == cell) else {
                    continue;
                };
                if !(t.sighted.is_empty() && t.mastered.is_empty() && t.after.is_empty()) {
                    return None;
                }
                gate.extend(one(&t.tech).unwrap_or_default());
            }
            (!gate.is_empty()).then_some(gate)
        }
        "recipe" => rs
            .recipes
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.requires.iter().cloned().collect()),
        "topic" => rs
            .research
            .iter()
            .any(|x| x.id == id)
            .then(|| BTreeSet::from([id.to_string()])),
        "raid" => rs
            .missions
            .iter()
            .find(|m| m.id == id && m.requires == 0 && m.needs.is_empty())
            .and_then(|m| one(&m.tech)),
        // Статья о рынке открывается вместе с торговым постом (`loreOpen`).
        "lore" if id == "trade" => rs.tiles.iter().find(|x| x.trade).and_then(|x| one(&x.tech)),
        "rule" => {
            let a = &rs.automation;
            let t = match id {
                "sales" => &a.sales,
                "crafting" => &a.crafting,
                _ => &a.raids,
            };
            one(t)
        }
        _ => None,
    }
}

/// Что заведомо открыто, раз открыта статья, — помимо технологий: лор о науке
/// ждёт построенной лаборатории (`loreOpen`), значит и лаборатория открыта.
/// ⚠️ `structure:big_lab` сюда не идёт: он стоит за своей технологией.
fn opened_by(rs: &Ruleset, key: &str) -> Vec<String> {
    match key {
        "lore:science" => rs
            .tiles
            .iter()
            .filter(|x| x.lab)
            .map(|x| format!("tile:{}", x.id))
            .collect(),
        // Изученная тема: лаборатория стояла, а плата и образец лежали на складе.
        _ if key.starts_with("topic:") => {
            let Some(t) = rs
                .research
                .iter()
                .find(|t| format!("topic:{}", t.id) == key)
            else {
                return Vec::new();
            };
            rs.tiles
                .iter()
                .filter(|x| x.lab)
                .map(|x| format!("tile:{}", x.id))
                .chain(
                    t.cost
                        .keys()
                        .chain(t.specimen.keys())
                        .map(|i| format!("item:{i}")),
                )
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Две фракции, которые база встречает разом: каждый заказ, где названа первая,
/// называет и вторую (развилка §12.43 — заказчик и пострадавший).
fn met_together(rs: &Ruleset, a: &str, b: &str) -> bool {
    let (Some(("faction", a)), Some(("faction", b))) = (a.split_once(':'), b.split_once(':'))
    else {
        return false;
    };
    let names = |m: &crate::ruleset::MissionDef, f: &str| m.patron == f || m.against == f;
    rs.missions
        .iter()
        .filter(|m| names(m, a))
        .all(|m| names(m, b))
}

/// Замыкание технологий: изученная тема значит изученные и все её предки.
fn tech_closure(rs: &Ruleset, mut known: BTreeSet<String>) -> BTreeSet<String> {
    loop {
        let more: Vec<String> = rs
            .research
            .iter()
            .filter(|t| known.contains(&t.id))
            .flat_map(|t| t.requires.iter().cloned())
            .filter(|r| !known.contains(r))
            .collect();
        if more.is_empty() {
            return known;
        }
        known.extend(more);
    }
}

/// Голая ссылка не называет того, что откроется позже самой статьи (§12.273).
///
/// Справочник знает не больше базы (§12.251): ссылку на закрытое вид гасит в
/// слово, но слово остаётся — «штампуется в мастерской» в статье о детали
/// рассказывало о постройке, которой в палитре ещё нет (§12.126). Такое место
/// обязано стоять в куске `{{вид:id|…}}`, который вид показывает только вместе
/// с названной вещью.
///
/// Голой ссылка законна, если её цель открыта с нулевого тика, если это сама
/// статья, или если цель закрыта **только технологиями**, и все они уже
/// известны, раз статья (или кусок вокруг ссылки) открыта: тема открывает то,
/// что даёт, а полная версия непонятого предмета (`---понято---`) — то, что
/// требует его понимание. Слова без ссылки сторож не видит: закрытую вещь в
/// тексте называют ссылкой, это и есть договор.
#[test]
fn a_bare_link_never_names_what_opens_later() {
    let rs = shipped();
    let owner = |k: &str| wiki_owner(&rs, k).unwrap_or_else(|| k.to_string());
    // Грани владельца: его статья открыта, если открыта любая из них.
    let gates_of = |k: &str| -> Vec<BTreeSet<String>> {
        let mut out: Vec<BTreeSet<String>> = tech_gate(&rs, k).into_iter().collect();
        for (kind, ids) in [
            (
                "topic",
                rs.research.iter().map(|x| x.id.clone()).collect::<Vec<_>>(),
            ),
            ("recipe", rs.recipes.iter().map(|x| x.id.clone()).collect()),
        ] {
            for id in ids {
                let f = format!("{kind}:{id}");
                if wiki_owner(&rs, &f).as_deref() == Some(k) {
                    out.extend(tech_gate(&rs, &f));
                }
            }
        }
        out
    };
    let mut bad = Vec::new();
    for file in WIKI {
        let mut key = String::new();
        let mut understood: Option<String> = None;
        let mut block: Option<Vec<String>> = None;
        for line in file.lines() {
            if let Some(k) = article_key(line) {
                key = k;
                understood = None;
                block = None;
                continue;
            }
            if let Some(m) = line.trim().strip_prefix("---понято") {
                understood = Some(m.trim_end_matches('-').trim().to_string());
                continue;
            }
            // Что известно, раз эта строка стоит перед игроком.
            let base = {
                // Своя статья открывается гранью **или** находкой, и находка
                // не обещает ничего: известно только то, без чего статьи нет.
                let mut known = tech_gate(&rs, &key).unwrap_or_default();
                if let Some(tech) = &understood {
                    if !tech.is_empty() {
                        known.insert(tech.clone());
                    } else if let Some((_, id)) = key.split_once(':')
                        && let Some(it) = rs.items.iter().find(|x| x.id == id)
                    {
                        known.extend(it.requires.iter().cloned());
                    }
                }
                known
            };
            let mut rest = line;
            loop {
                let open = rest.find("{{");
                let close = rest.find("}}");
                let link = rest.find("[[");
                let next = [open, close, link].into_iter().flatten().min();
                let Some(at) = next else { break };
                if Some(at) == open {
                    let inner = &rest[at + 2..];
                    let end = inner.find('|').unwrap_or(0);
                    block = Some(inner[..end].split(',').map(|k| owner(k.trim())).collect());
                    rest = &inner[end..];
                } else if Some(at) == close {
                    block = None;
                    rest = &rest[at + 2..];
                } else {
                    let inner = &rest[at + 2..];
                    let Some(end) = inner.find("]]") else { break };
                    let target = owner(inner[..end].split('|').next().unwrap_or("").trim());
                    rest = &inner[end + 2..];
                    let mut known = base.clone();
                    if let Some(b) = &block {
                        if b.contains(&target) {
                            continue;
                        }
                        for k in b {
                            known.extend(tech_gate(&rs, k).unwrap_or_default());
                        }
                    }
                    let known = tech_closure(&rs, known);
                    let ok = target == key
                        || opened_by(&rs, &key).contains(&target)
                        || met_together(&rs, &key, &target)
                        || open_at_start(&rs, &target)
                        || gates_of(&target)
                            .iter()
                            .any(|g| !g.is_empty() && g.is_subset(&known));
                    if !ok {
                        bad.push(format!("{key} → {target}"));
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "голые ссылки на то, что откроется позже статьи:\n{}",
        bad.join("\n")
    );
}
