// Освещение базы — прототип, только вид.
//
// Устроено как в любой 2D-игре со светом: отдельная текстура света в пониженном
// разрешении (`LIGHT_RES`), в неё каждым кадром рисуется фон и источники
// (аддитивно), а сама текстура лежит поверх мира с `multiply` — тёмное
// затемняет, светлое пропускает. Свет мягкий, поэтому пониженное разрешение
// не видно, а размытый край тени выходит сам собой.
//
// **Тени от стен.** Свет лампы обрезан её многоугольником видимости: лучи из
// центра идут до первой клетки пустоты (скалы) и заходят в неё на `RIM`, чтобы
// стена изнутри комнаты была освещена, а снаружи — нет. Считается это только
// на смене карты, как и сами лампы.
//
// **Запекание.** Лампы, которые не мерцают, рисуются в отдельную текстуру один
// раз на смену карты (`baked`) и дальше стоят в кадре одним спрайтом, сколько
// бы их ни было. Каждым кадром живут только мерцающие лампы (их единицы) и свет
// котов — он едет за котом и стен не знает: радиус у него меньше клетки стены.
//
// ⚠️ Узлы здесь постоянные (§12.84): градиент — одна текстура на все
// источники, лампы пересобираются только в `rebuild` (зовётся из `drawMap`),
// временные узлы запекания разрушаются сразу, а котам свет раздаётся из пула и
// лишь двигается. Покадрово меняются позиции и яркость, ничего не создаётся.
//
// Ядро о свете не знает: лампа — это роль клетки, которую вид и так рисует.

import {
  BlurFilter,
  Container,
  Graphics,
  Rectangle,
  RenderTexture,
  Sprite,
  Texture,
} from "pixi.js";
import { createHdrFilter, hdrSupported } from "./hdr.js";

/// Во сколько раз текстура света мельче мира (в пикселях мира, до масштаба).
const LIGHT_RES = 0.5;
/// Фон там, куда свет не дошёл. Не чёрный: база обязана читаться и в тени,
/// а скала вокруг — оставаться камнем, а не дырой. Игрок двигает его ползунком
/// «Тень» между двумя концами (0 — ночь, 100 — почти без тени).
const SHADE_DARK = [16, 20, 32];
const SHADE_LIGHT = [168, 176, 196];
/// Насколько свет **высветляет** пол сверх его собственной краски. `multiply`
/// умеет только затемнять, а пол и так тёмный, — без этого прохода под лампой
/// было бы ровно то же, что без света, и пятна читались бы одной тенью вокруг.
/// Второй проход — те же лампы без фона, аддитивно и слабо.
/// Настройки по умолчанию: тень (0–100), сила ламп (0–60, в процентах
/// высветления) и мягкость теней (0–12, пикселей размытия). Живут в
/// `localStorage` вида, а не в партии: это выбор игрока о своём экране, как звук.
const DEFAULTS = {
  on: true,
  shade: 40,
  glow: 20,
  soft: 5,
  sat: 60,
  // HDR-сведение (`hdr.js`) и две его ручки: экспозиция (в процентах) и
  // свечение вокруг ярких ламп. Без float-текстур HDR не включается вовсе.
  hdr: true,
  exposure: 100,
  bloom: 35,
};
/// «Насыщенность» (0–100): на нуле все лампы спокойного тёплого белого
/// (`CALM`), на `SAT_BASE` — цвета из `ROLE_LIGHT` как есть, выше — сочнее.
/// Вместе с ней разгорается свет **каждой клетки** цветом её типа (`CELL`):
/// по цвету читается, где что стоит. Цвет — только от типа, никакого
/// случая: разноцветье вразнобой красиво, но карту не читать (пробовали).
const CALM = 0xfff2e2;
const SAT_BASE = 60;
/// Свет клетки: радиус в клетках и яркость на максимуме насыщенности.
const CELL = { radius: 1.1, alpha: 0.6 };
/// Сколько «сверх нормы» насыщенности (0 на `SAT_BASE` и ниже, 1 на максимуме).
/// На нём держатся три правки, без которых цвет выходит блёклым: белый потолок
/// гаснет (`CEIL_FADE`), иначе он складывается с цветом в белое; свет клеток
/// растёт и шире (`CELL_BOOST`); высветление сильнее (`GLOW_BOOST`) — `multiply`
/// цветом по синеватому полу даёт серое, и цвет виден только сложением.
function over(sat) {
  return Math.max(0, (sat - SAT_BASE) / (100 - SAT_BASE));
}
const CEIL_FADE = 0.6;
const CELL_BOOST = { radius: 0.3, alpha: 0.3 };
const GLOW_BOOST = 1;
/// Мерцание и пульс ламп (`flicker`, `pulse` в `ROLE_LIGHT`) пока выключены:
/// с ними мастерская и рация отвлекали от карты. Вернуть — `true`.
const ANIMATE = false;
const PREFS_KEY = "sp-light";
/// Сколько луч заходит в скалу, в клетках: освещает стену изнутри комнаты.
const RIM = 0.2;
/// Лучей на лампу: на радиусе в 3–4 клетки этого хватает, чтобы край тени
/// не шёл зубцами (а остаток сгладит пониженное разрешение).
const RAYS = 240;

/// Свет роли клетки: цвет, радиус в клетках, яркость и характер; `cell` —
/// во сколько раз приглушён свет её отдельных клеток (лежанка: на ней спит
/// кот, и свет, сложенный с потолочным, выбеливал его до неузнаваемости).
/// Чего нет в списке, само не светит — его освещают потолочные лампы.
/// Палитра одна на всю базу: холодный белый — общий свет, тёплое — жильё и
/// цех, цветное — то, у чего есть экран или прибор.
const ROLE_LIGHT = {
  lab: { color: 0x9d7bff, radius: 3.4, alpha: 0.95, pulse: 0.12 },
  bench: { color: 0xb08cff, radius: 2.2, alpha: 0.75 },
  shop: { color: 0xff9a3c, radius: 3.6, alpha: 1.0, flicker: 0.3 },
  ward: { color: 0xeaf6ff, radius: 3.2, alpha: 1.0 },
  relay: { color: 0x3fe6c4, radius: 3.0, alpha: 0.95, pulse: 0.35 },
  garage: { color: 0xffb04a, radius: 3.4, alpha: 1.0 },
  post: { color: 0xffd27a, radius: 3.2, alpha: 0.95 },
  storage: { color: 0xc4d6ff, radius: 2.6, alpha: 0.65 },
  rack: { color: 0xc4d6ff, radius: 2.6, alpha: 0.6 },
  bin: { color: 0xc4d6ff, radius: 2.0, alpha: 0.55 },
  bed: { color: 0xffa860, radius: 2.0, alpha: 0.3, cell: 0.4 },
  nest: { color: 0xffa860, radius: 2.0, alpha: 0.3, cell: 0.4 },
  desk: { color: 0xffe6b0, radius: 2.4, alpha: 0.8 },
  lectern: { color: 0xffe6b0, radius: 2.0, alpha: 0.65 },
  muster_table: { color: 0xffc070, radius: 2.4, alpha: 0.75 },
};
/// Потолочная лампа над полом: раз в `CEIL_STEP` клеток по обеим осям.
const CEIL = { color: 0xdde6ff, radius: 2.8, alpha: 0.6 };
const CEIL_STEP = 3;
/// Свет у кота — тёплое пятнышко, чтобы кот в тени не терялся.
const CAT = { color: 0xffd9a8, radius: 1.3, alpha: 0.3 };

function rgbOf(c) {
  return [(c >> 16) & 255, (c >> 8) & 255, c & 255];
}

function hexOf([r, g, b]) {
  return (Math.round(r) << 16) | (Math.round(g) << 8) | Math.round(b);
}

function mix(a, b, k) {
  const [ar, ag, ab] = rgbOf(a);
  const [br, bg, bb] = rgbOf(b);
  return hexOf([ar + (br - ar) * k, ag + (bg - ag) * k, ab + (bb - ab) * k]);
}

/// Насыщенность до предела: тянем цвет от его серого к самому себе и дальше.
function saturate(c, k) {
  const [r, g, b] = rgbOf(c);
  const grey = (r + g + b) / 3;
  const push = 1 + k * 1.6;
  const ch = (v) => Math.max(0, Math.min(255, grey + (v - grey) * push));
  return hexOf([ch(r), ch(g), ch(b)]);
}

/// Цвет лампы при текущей насыщенности `sat` (0–100).
function tintOf(spec, sat) {
  if (sat <= SAT_BASE) return mix(CALM, spec.color, sat / SAT_BASE);
  return saturate(spec.color, (sat - SAT_BASE) / (100 - SAT_BASE));
}

/// Радиальный спад: плотное ядро и долгий мягкий хвост — так свет читается
/// лампой, а не пятном с краем.
function gradientTexture() {
  const size = 128;
  const cv = document.createElement("canvas");
  cv.width = cv.height = size;
  const ctx = cv.getContext("2d");
  const g = ctx.createRadialGradient(
    size / 2,
    size / 2,
    0,
    size / 2,
    size / 2,
    size / 2,
  );
  g.addColorStop(0, "rgba(255,255,255,1)");
  g.addColorStop(0.2, "rgba(255,255,255,0.8)");
  g.addColorStop(0.5, "rgba(255,255,255,0.38)");
  g.addColorStop(0.78, "rgba(255,255,255,0.12)");
  g.addColorStop(1, "rgba(255,255,255,0)");
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, size, size);
  return Texture.from(cv);
}

/// Многоугольник видимости лампы в пикселях мира: точка на луч, луч идёт
/// шагом в десятую клетки до радиуса или до скалы (+ `RIM`).
function visibility(cells, W, H, cx, cy, radius, tile) {
  const isVoid = (x, y) => {
    const ix = Math.floor(x);
    const iy = Math.floor(y);
    return ix < 0 || iy < 0 || ix >= W || iy >= H || cells[iy * W + ix] < 0;
  };
  const step = 0.1;
  const pts = [];
  for (let i = 0; i < RAYS; i++) {
    const a = (i / RAYS) * Math.PI * 2;
    const dx = Math.cos(a);
    const dy = Math.sin(a);
    let d = 0;
    let hit = -1;
    while (d < radius) {
      d += step;
      if (hit < 0 && isVoid(cx + dx * d, cy + dy * d)) hit = d;
      if (hit >= 0 && d >= hit + RIM) break;
    }
    d = Math.min(d, radius);
    pts.push((cx + dx * d) * tile, (cy + dy * d) * tile);
  }
  return pts;
}

export function createLighting(app, tile) {
  const glow = gradientTexture();
  const canHdr = hdrSupported(app);
  // Свет копится во float, если можно: в простом режиме это ничего не
  // меняет, а в HDR перекрытие ламп не обрезается единицей.
  const lightFormat = canHdr ? "rgba16float" : "rgba8unorm";
  const hdr = canHdr ? createHdrFilter() : null;
  // Свечение: запечённый свет, размытый широко и в четверть разрешения света.
  let bloomTarget = null;
  const bloomSrc = new Sprite();
  bloomSrc.filters = [new BlurFilter({ strength: 14, quality: 4 })];
  /// Контейнер мировых слоёв, на который вешается HDR-фильтр, и сам мир —
  /// по нему считается, где карта лежит на экране (`attach`).
  let lit = null;
  let worldNode = null;
  const scene = new Container(); // рисуется только в текстуру, на сцене его нет
  const ambient = new Sprite(Texture.WHITE);
  const bakedSprite = new Sprite();
  bakedSprite.blendMode = "add";
  const lamps = new Container(); // живые (мерцающие) лампы: пересобираются в `rebuild`
  const cats = new Container(); // пул света котов: растёт, не убывает
  scene.addChild(ambient, bakedSprite, lamps, cats);
  // Поле вокруг карты: стены у края рисуются чуть за её пределами, и свет,
  // покрывающий ровно карту, оставлял их выступ незатемнённым, а HDR-фильтр
  // его срезал. Текстуры света шире карты на клетку с каждой стороны, а всё,
  // что стоит в координатах мира, сдвинуто внутрь на это поле.
  const PAD = tile;
  lamps.position.set(PAD, PAD);
  cats.position.set(PAD, PAD);
  // Запекание тоже со сдвигом: лампа кладётся в `bakeShift` на время рендера.
  const bakeRoot = new Container();
  const bakeShift = new Container();
  bakeShift.position.set(PAD, PAD);
  bakeRoot.addChild(bakeShift);

  let target = null;
  let glowTarget = null;
  let baked = null;
  let bakedSoft = null;
  const soften = new Sprite();
  // Размытие запечённого света: мягкий край тени, как от лампы, а не от точки.
  // Платится один раз на смену карты (или ползунка), а не каждым кадром.
  const blur = new BlurFilter({ strength: DEFAULTS.soft, quality: 3 });
  soften.filters = [blur];
  let bakedCount = 0;
  const sprite = new Sprite();
  sprite.blendMode = "multiply";
  sprite.position.set(-PAD, -PAD);
  sprite.eventMode = "none";
  const glowLayer = new Sprite();
  glowLayer.blendMode = "add";
  glowLayer.position.set(-PAD, -PAD);
  glowLayer.eventMode = "none";
  // Наружу — двумя узлами на разные места в мире: высветление ложится **под**
  // котов (светит пол и мебель, а не пересвечивает кота до белого), тень —
  // **поверх** всего, чтобы кот в тёмной комнате тоже был в тени.

  const prefs = { ...DEFAULTS };
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    // Прежний формат — голое «0»/«1» за клавишей L.
    if (raw === "0" || raw === "1") prefs.on = raw === "1";
    else if (raw) Object.assign(prefs, JSON.parse(raw));
  } catch {
    /* без хранилища — настройки по умолчанию */
  }
  let enabled = prefs.on;
  applyShade();
  applyMode();
  blur.strength = prefs.soft;
  soften.filters = prefs.soft > 0 ? [blur] : [];

  function save() {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch {
      /* не запомнили — не страшно */
    }
  }

  /// HDR сейчас и правда работает: включён свет, выбран режим, есть float.
  function hdrOn() {
    return enabled && prefs.hdr && !!hdr;
  }

  /// Подпорки простого режима (`CEIL_FADE`, `CELL_BOOST`, `GLOW_BOOST`)
  /// существуют только потому, что `multiply` обрезает свет единицей. В HDR их
  /// нет: насыщенность доводит шейдер.
  function boost() {
    return hdrOn() ? 0 : over(prefs.sat);
  }

  function applyGlow() {
    glowLayer.alpha = (prefs.glow / 100) * (1 + GLOW_BOOST * boost());
  }

  /// Кто рисует свет: фильтр на мировых слоях (HDR) или два спрайта.
  function applyMode() {
    sprite.visible = glowLayer.visible = enabled && !hdrOn();
    if (lit) lit.filters = hdrOn() ? [hdr.filter] : null;
    applyGlow();
    applyHdr();
  }

  function applyHdr() {
    if (!hdr) return;
    const t = ambient.tint;
    hdr.params({
      ambient: [
        ((t >> 16) & 255) / 255,
        ((t >> 8) & 255) / 255,
        (t & 255) / 255,
      ],
      gain: prefs.glow / 16,
      exposure: prefs.exposure / 100,
      sat: 1 + 0.8 * over(prefs.sat),
      bloom: prefs.bloom / 100,
    });
  }

  function applyShade() {
    const k = prefs.shade / 100;
    const [r, g, b] = SHADE_DARK.map((d, i) =>
      Math.round(d + (SHADE_LIGHT[i] - d) * k),
    );
    ambient.tint = (r << 16) | (g << 8) | b;
    applyHdr();
  }
  /// Стресс-режим: сколько ламп на клетку пола (0 — обычный свет) и по три на
  /// кота — проверить, где у света потолок, на карте, где ламп мало.
  /// Shift+L перебирает 0 → 1 → 10.
  let stress = 0;
  let lastMap = null;
  let clock = 0;
  // Сколько миллисекунд CPU уходит на свет за кадр, среднее за секунду замера.
  let costSum = 0;
  let costFrames = 0;
  let bakeMs = 0;
  /// Пересобрать свет карты на следующем кадре: ползунок насыщенности частит,
  /// а пересборка нужна одна на кадр, а не на каждое событие `input`.
  let dirty = false;

  function glowSprite(spec, x, y, color = spec.color) {
    const s = new Sprite(glow);
    s.anchor.set(0.5);
    s.blendMode = "add";
    s.tint = color;
    s.alpha = spec.alpha;
    s.width = s.height = spec.radius * 2 * tile;
    s.x = x;
    s.y = y;
    return s;
  }

  /// Лампа с тенью: градиент под маской своего многоугольника видимости.
  function shadedLamp(spec, map, cx, cy) {
    const node = new Container();
    const mask = new Graphics()
      .poly(visibility(map.cells, map.W, map.H, cx, cy, spec.radius, tile))
      .fill(0xffffff);
    const s = glowSprite(spec, cx * tile, cy * tile, tintOf(spec, prefs.sat));
    if (spec === CEIL) s.alpha *= 1 - CEIL_FADE * boost();
    s.mask = mask;
    node.addChild(mask, s);
    node.spec = spec;
    node.glow = s;
    node.phase = (cx * 7 + cy * 13) % 100;
    return node;
  }

  /// Свет карты. Зовётся из `drawMap`, то есть только на росте `map_version`.
  function rebuild(meta, cells, regions) {
    const t0 = performance.now();
    lastMap = { meta, cells, regions };
    const W = meta.width;
    const H = meta.height;
    const map = { cells, W, H };
    const pw = W * tile + 2 * PAD;
    const ph = H * tile + 2 * PAD;
    if (!target || target.width !== pw || target.height !== ph) {
      target?.destroy(true);
      glowTarget?.destroy(true);
      baked?.destroy(true);
      bakedSoft?.destroy(true);
      bloomTarget?.destroy(true);
      target = RenderTexture.create({
        width: pw,
        height: ph,
        resolution: LIGHT_RES,
        format: lightFormat,
      });
      glowTarget = RenderTexture.create({
        width: pw,
        height: ph,
        resolution: LIGHT_RES,
        format: lightFormat,
      });
      baked = RenderTexture.create({
        width: pw,
        height: ph,
        resolution: LIGHT_RES,
        format: lightFormat,
      });
      bakedSoft = RenderTexture.create({
        width: pw,
        height: ph,
        resolution: LIGHT_RES,
        format: lightFormat,
      });
      bloomTarget = RenderTexture.create({
        width: pw,
        height: ph,
        resolution: LIGHT_RES / 4,
        format: lightFormat,
      });
      bloomSrc.texture = bakedSoft;
      hdr?.textures(target, bloomTarget);
      soften.texture = baked;
      sprite.texture = target;
      glowLayer.texture = glowTarget;
      bakedSprite.texture = bakedSoft;
    }
    ambient.width = pw;
    ambient.height = ph;
    for (const n of lamps.removeChildren()) n.destroy({ children: true });

    // Где стоят лампы: роль — один свет на комнату, в центре своей порции
    // клеток, как глиф (§12.109); большая комната — по лампе на ~12 клеток.
    const spots = [];
    for (const r of regions) {
      const spec = ROLE_LIGHT[meta.palette[r.tile]?.id];
      if (!spec) continue;
      const per = Math.max(1, Math.round(r.cells.length / 12));
      const step = Math.max(1, Math.floor(r.cells.length / per));
      for (let i = 0; i < r.cells.length; i += step) {
        const at = r.cells[Math.min(r.cells.length - 1, i + (step >> 1))];
        const x = at % W;
        spots.push([spec, x + 0.5, (at - x) / W + 0.5]);
      }
    }
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        if (cells[y * W + x] < 0) continue;
        const ceil = x % CEIL_STEP === 1 && y % CEIL_STEP === 1;
        if (!stress && !ceil) continue;
        for (let k = 0; k < Math.max(1, stress); k++) {
          const ox = stress > 1 ? ((k * 37) % 10) / 10 : 0.5;
          const oy = stress > 1 ? ((k * 61) % 10) / 10 : 0.5;
          spots.push([CEIL, x + ox, y + oy]);
        }
      }
    }

    // Свет клеток — цветом типа, сила от насыщенности. Пол (`base`) не светит:
    // он фон, а не место. Запекается вместе с лампами, тени у него свои.
    const glowK = prefs.sat / 100;
    if (glowK > 0) {
      const cellSpec = new Map();
      for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
          const t = cells[y * W + x];
          if (t < 0) continue;
          const def = meta.palette[t];
          if (!def || def.base) continue;
          let spec = cellSpec.get(t);
          if (!spec) {
            const color =
              ROLE_LIGHT[def.id]?.color ??
              parseInt(String(def.color || "#ffffff").slice(1), 16);
            const k = boost();
            spec = {
              color,
              radius: CELL.radius + CELL_BOOST.radius * k,
              alpha:
                Math.min(1, CELL.alpha * glowK + CELL_BOOST.alpha * k) *
                (ROLE_LIGHT[def.id]?.cell ?? 1),
            };
            cellSpec.set(t, spec);
          }
          spots.push([spec, x + 0.5, y + 0.5]);
        }
      }
    }

    // Неподвижные — в запечённую текстуру, по одной и с разрушением следом;
    // мерцающие — живыми узлами в кадр.
    let first = true;
    bakedCount = 0;
    for (const [spec, cx, cy] of spots) {
      const node = shadedLamp(spec, map, cx, cy);
      if (ANIMATE && (spec.flicker || spec.pulse)) {
        lamps.addChild(node);
        continue;
      }
      bakeShift.addChild(node);
      app.renderer.render({ container: bakeRoot, target: baked, clear: first });
      bakeShift.removeChild(node);
      first = false;
      node.destroy({ children: true });
      bakedCount++;
    }
    if (first)
      app.renderer.render({
        container: new Container(),
        target: baked,
        clear: true,
      });
    resoften();
    bakeMs = performance.now() - t0;
  }

  /// Кадр: двигаем свет котов, мерцаем, перерисовываем текстуру.
  function frame(units, deltaMS, speed) {
    if (!enabled || !target) return;
    if (lit && !lit.boundsArea)
      lit.boundsArea = new Rectangle(-PAD, -PAD, target.width, target.height);
    const t0 = performance.now();
    clock += (deltaMS * Math.max(speed, 1)) / 1000;
    if (dirty && lastMap) {
      dirty = false;
      rebuild(lastMap.meta, lastMap.cells, lastMap.regions);
    }
    for (const n of lamps.children) {
      const spec = n.spec;
      if (spec.flicker) {
        const f =
          Math.sin(clock * 23 + n.phase) * Math.sin(clock * 7.3 + n.phase);
        n.glow.alpha = spec.alpha * (1 - spec.flicker * (0.5 + 0.5 * f));
      } else if (spec.pulse) {
        n.glow.alpha =
          spec.alpha *
          (1 - spec.pulse * (0.5 + 0.5 * Math.sin(clock * 2 + n.phase)));
      }
    }
    const per = stress ? 3 : 1;
    let n = 0;
    for (const c of units.values()) {
      if (!c.visible) continue;
      for (let k = 0; k < per; k++) {
        let s = cats.children[n];
        if (!s) s = cats.addChild(glowSprite(CAT, 0, 0));
        s.visible = true;
        s.x = c.x + (k ? (k === 1 ? -0.6 : 0.6) * tile : 0);
        s.y = c.y;
        n++;
      }
    }
    for (let i = n; i < cats.children.length; i++)
      cats.children[i].visible = false;
    if (hdrOn()) {
      // В HDR фон — uniform шейдера, а в текстуре только лампы и коты:
      // их сумма свободно выходит за единицу.
      ambient.visible = false;
      app.renderer.render({ container: scene, target, clear: true });
      ambient.visible = true;
      if (worldNode) {
        const p = worldNode.toGlobal({ x: -PAD, y: -PAD });
        const k = worldNode.scale.x;
        hdr.rect(p.x, p.y, target.width * k, target.height * k);
      }
      costSum += performance.now() - t0;
      costFrames++;
      return;
    }
    app.renderer.render({ container: scene, target, clear: true });
    // Высветление — та же сцена без фона и без котов: кот светит, чтобы не
    // теряться в тени, а не чтобы бликовать.
    ambient.visible = false;
    cats.visible = false;
    app.renderer.render({ container: scene, target: glowTarget, clear: true });
    ambient.visible = true;
    cats.visible = true;
    costSum += performance.now() - t0;
    costFrames++;
  }

  /// Среднее время света на кадр с прошлого вызова, мс; сбрасывает счёт.
  function takeCost() {
    const v = costFrames ? costSum / costFrames : 0;
    costSum = 0;
    costFrames = 0;
    return v;
  }

  /// Размыть запечённое заново — без пересборки ламп: лучи и маски те же.
  function resoften() {
    if (!bakedSoft) return;
    app.renderer.render({ container: soften, target: bakedSoft, clear: true });
    app.renderer.render({
      container: bloomSrc,
      target: bloomTarget,
      clear: true,
    });
  }

  function toggle() {
    enabled = !enabled;
    prefs.on = enabled;
    applyMode();
    dirty = true;
    save();
    return enabled;
  }

  function setHdr(on) {
    prefs.hdr = on;
    applyMode();
    dirty = true; // подпорки простого режима запечены в лампы
    save();
  }

  function setExposure(v) {
    prefs.exposure = v;
    applyHdr();
    save();
  }

  function setBloom(v) {
    prefs.bloom = v;
    applyHdr();
    save();
  }

  /// Мировые слои под HDR-фильтр и сам мир (для координат света).
  function attach(container, world) {
    lit = container;
    worldNode = world;
    applyMode();
  }

  function setShade(v) {
    prefs.shade = v;
    applyShade();
    save();
  }

  function setGlow(v) {
    prefs.glow = v;
    applyGlow();
    applyHdr();
    save();
  }

  function setSoft(v) {
    prefs.soft = v;
    blur.strength = v;
    // Ноль — без фильтра вовсе: размытие силой 0 всё равно гоняет проход.
    soften.filters = v > 0 ? [blur] : [];
    resoften();
    save();
  }

  function setSat(v) {
    prefs.sat = v;
    applyGlow();
    applyHdr();
    dirty = true;
    save();
  }

  function reset() {
    Object.assign(prefs, DEFAULTS, { on: enabled, hdr: prefs.hdr });
    applyShade();
    applyMode();
    setSoft(prefs.soft);
    dirty = true;
  }

  function toggleStress() {
    stress = stress === 0 ? 1 : stress === 1 ? 10 : 0;
    if (lastMap) rebuild(lastMap.meta, lastMap.cells, lastMap.regions);
    return stress;
  }

  return {
    /// Тень — поверх котов.
    shade: sprite,
    /// Высветление — под котами.
    glow: glowLayer,
    rebuild,
    frame,
    toggle,
    toggleStress,
    setShade,
    setGlow,
    setSoft,
    setSat,
    setHdr,
    setExposure,
    setBloom,
    attach,
    /// Есть ли у браузера float-текстуры, без которых HDR не считается.
    canHdr,
    reset,
    /// Текущие настройки (копия) и уровень стресса — для раздела «Свет».
    prefs: () => ({ ...prefs, on: enabled, stress, hdr: !!hdr && prefs.hdr }),
    takeCost,
    /// Сколько стоила последняя пересборка света карты, мс.
    bakeCost: () => bakeMs,
    count: () =>
      enabled
        ? bakedCount +
          lamps.children.length +
          cats.children.filter((s) => s.visible).length
        : "выкл",
  };
}
