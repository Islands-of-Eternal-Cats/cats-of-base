// Вид 3/4 — прототип, только вид (ветка `worktree-view-three-quarter`).
//
// Клетки остаются квадратными, а у карты появляется высота: всё, что стоит на
// высоте `h`, рисуется сдвинутым **вверх** на `h` (косоугольная проекция, как в
// RimWorld и Prison Architect). Ядро и ввод об этом не знают — `tileAt` по-
// прежнему мерит плоскую сетку, потому что пол лежит на нулевой высоте.
//
// **Стены в разрезе.** Верх стены лежит тонкой полосой в скале, как в плоском
// виде; лицо показывает только северная стена, и лежит оно в основном на
// верхней кромке пола (`LIFT`). Стена так читается тонкой границей, а клетка скалы за ней — клеткой,
// куда можно строить. Боковые и южные стены видны только верхом: поднятые,
// они заслоняли бы пол, кучи и котов у себя за спиной.
//
// **Мебель — коробки.** Верх смещён вверх на высоту, под ним видна передняя
// грань. Высокое (стеллаж, мачта рации) заезжает на клетку выше — это и есть
// 3/4, а не ошибка.
//
// ⚠️ Узлы здесь не живут: функция только рисует в переданные `Graphics`, а
// пересобирает и разрушает их `drawMap` (§12.84, седьмое лицо).

/// Высота лица северной стены, в долях клетки: высокой (лицо в клетке скалы)
/// и опущенной на время стройки (лицо на кромке пола).
const RISE = 0.72;
const LOW = 0.45;
/// Сколько лица заходит в клетку скалы над полом; остальное лежит на верхней
/// кромке пола. Весь в скале — и стена съедает клетку, куда игрок хочет
/// строить; весь на полу — и мебель у стены закрывает лицо целиком.
const LIFT = 0.2;
/// Толщина стены (её верха) в долях клетки.
const THICK = 0.28;
/// Сколько цвета комнаты подмешано к материалу пола: материал теперь говорит
/// «что это за комната» сам, оттенок только поддерживает его.
const TINT = 0.22;

/// Материал пола по роли клетки. Неизвестное — бетон, как пол коридора.
const MATERIAL = {
  floor: "concrete",
  storage: "concrete",
  rack: "concrete",
  bin: "concrete",
  bed: "wood",
  nest: "wood",
  lab: "tile",
  bench: "tile",
  ward: "tile",
  garage: "plate",
  muster_table: "plate",
  shop: "rubber",
  post: "rubber",
  relay: "rubber",
  desk: "carpet",
  desk_table: "carpet",
  lectern: "carpet",
};

const BASE = {
  concrete: 0x3a3e47,
  wood: 0x6a4a30,
  tile: 0x8a9098,
  plate: 0x4a4c50,
  rubber: 0x2f3136,
  carpet: 0x3c3a52,
};

/// Рисует карту в четыре узла по порядку наложения: пол (`ground`), лица
/// стен (`faces`), мебель (`props`) и верха стен (`caps`).
export function drawOblique(d) {
  const { ground, faces, props, caps, at, W, H, T, palette, tints } = d;
  const { noise, shade, mix, rock, rockSpeck } = d;

  // 1. Скала — та же, что в плоском виде.
  ground.rect(0, 0, W * T, H * T).fill(rock);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (at(x, y) >= 0) continue;
      const n = noise(x, y);
      for (let i = 0; i < 1 + Math.floor(n * 2); i++) {
        ground
          .circle(
            x * T + noise(x, y, 10 + i) * T,
            y * T + noise(x, y, 20 + i) * T,
            4 + noise(x, y, 30 + i) * 9,
          )
          .fill({
            color: noise(x, y, 35 + i) > 0.5 ? rockSpeck : 0x121317,
            alpha: 0.32,
          });
      }
    }
  }

  // 2. Пол — материалом роли.
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const v = at(x, y);
      if (v < 0) continue;
      const id = palette[v]?.id;
      const mat = MATERIAL[id] ?? "concrete";
      floorCell(ground, mat, x, y, T, mix(BASE[mat], tints[v] ?? BASE[mat], TINT), d);
      // Граница зоны между комнатами разного типа — тонкая линия, как и было.
      if (at(x + 1, y) >= 0 && at(x + 1, y) !== v) {
        ground.rect(x * T + T - 1.5, y * T, 1.5, T).fill({ color: 0, alpha: 0.45 });
      }
      if (at(x, y + 1) >= 0 && at(x, y + 1) !== v) {
        ground.rect(x * T, y * T + T - 1.5, T, 1.5).fill({ color: 0, alpha: 0.45 });
      }
    }
  }

  // 3. Стены. Верх стены — полоса в скале по краю пола, и обводит каждую
  // комнату замкнутой рамкой; лицо показывает только северная. Высот у неё
  // две: пока игрок смотрит на базу, стена высокая и
  // её лицо стоит в клетке скалы над полом; с инструментом в руке стена
  // опускается — лицо уходит на кромку пола, и клетка за стеной видна скалой,
  // которую можно застроить. Высокое лицо забирало клетку почти целиком, и
  // пол рядом со стеной размечали через клетку.
  const wall = 0x535866;
  const wallTop = 0x6c707c;
  const edge = shade(wallTop, -0.35);
  const wt = T * THICK;
  const isRock = (x, y) => at(x, y) < 0;

  // Ореол скалы: клетки вокруг базы чуть светлее глубины — база читается
  // вырезанной в камне, а не нарисованной на чёрном.
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (at(x, y) >= 0) continue;
      let near = false;
      for (let dy = -1; dy <= 1 && !near; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if (at(x + dx, y + dy) >= 0) near = true;
        }
      }
      if (near) ground.rect(x * T, y * T, T, T).fill({ color: 0x2a2d36, alpha: 0.45 });
    }
  }

  if (d.lowered) lowWalls();
  else tallWalls();

  // Затенение у основания стен: пол темнеет к краю со всех сторон, сильнее в
  // углах, — объём комнаты держится не только высотой лица.
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (at(x, y) < 0) continue;
      const px = x * T;
      const py = y * T;
      const ao = (rx, ry, rw, rh) => {
        ground.rect(rx, ry, rw, rh).fill({ color: 0, alpha: 0.1 });
      };
      const s1 = T * 0.06;
      const s2 = T * 0.16;
      if (isRock(x + 1, y)) {
        ao(px + T - s2, py, s2, T);
        ao(px + T - s1, py, s1, T);
      }
      if (isRock(x - 1, y)) ao(px, py, s2, T);
      if (isRock(x, y + 1)) {
        ao(px, py + T - s2, T, s2);
        ao(px, py + T - s1, T, s1);
      }
      if (isRock(x, y - 1)) ao(px, py, T, s2);
    }
  }

  // Сетка поверх всего — только со стройкой в руке: видно, что стена стоит в
  // своей клетке, а скала за ней делится на клетки, куда целятся.
  if (d.lowered) {
    for (let x = 0; x <= W; x++) caps.rect(x * T, 0, 1, H * T).fill({ color: 0xffffff, alpha: 0.07 });
    for (let y = 0; y <= H; y++) caps.rect(0, y * T, W * T, 1).fill({ color: 0xffffff, alpha: 0.07 });
  }

  // Высокие стены: лицо в клетке скалы над полом.
  function tallWalls() {
    const h = Math.min(T * RISE, T - wt);
    const late = [];
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        if (at(x, y) < 0) continue;
        const px = x * T;
        const py = y * T;
        const up = isRock(x, y - 1);
        const down = isRock(x, y + 1);
        const left = isRock(x - 1, y);
        const right = isRock(x + 1, y);

        if (up) {
          // Перемычка скалы в одну клетку: верх северной стены доходит до южной
          // стены комнаты сверху, и перемычка читается одной стеной.
          const thin = at(x, y - 2) >= 0;
          const top = thin ? py - T : py - h - wt;
          wallFace(faces, px, py - h, T, h, x, y, tints[at(x, y)] ?? 0x464b57, d);
          caps.rect(px, top, T, py - h - top).fill(wallTop);
          caps.rect(px, py - h - 1.5, T, 1.5).fill(edge);
          // Угловые столбы: верх боковой стены поднимается к верху северной.
          if (left) caps.rect(px - wt, top, wt, py - top).fill(wallTop);
          if (right) caps.rect(px + T, top, wt, py - top).fill(wallTop);
          if (left) caps.rect(px - 1.5, py - h - 1.5, 1.5, h + 1.5).fill(edge);
          if (right) caps.rect(px + T, py - h - 1.5, 1.5, h + 1.5).fill(edge);
          // Тень стены на пол под ней — у высокой стены она длиннее.
          ground.rect(px, py, T, T * 0.32).fill({ color: 0, alpha: 0.22 });
          ground.rect(px, py, T, T * 0.12).fill({ color: 0, alpha: 0.2 });
        }
        // Боковая стена. Если под её клеткой скалы пол, эта клетка — поднятая
        // северная стена соседней комнаты (торец перемычки у коридора или
        // ступенька комнаты): её верх и лицо уже доходят до нашего края. Полоса
        // тогда идёт только до низа поднятого верха и смыкается с ним, а лицо
        // кончается своим ребром. Довести полосу до пола нельзя: она легла бы
        // столбом поперёк лица.
        const side = (s) => {
          const wx = s < 0 ? px - wt : px + T;
          const ex = s < 0 ? px - 1.5 : px + T;
          const raised = at(x + s, y + 1) >= 0;
          const bottom = !raised ? py + T : at(x + s, y - 1) >= 0 ? py : py + T - h;
          if (bottom > py) {
            caps.rect(wx, py, wt, bottom - py).fill(wallTop);
            // Кромку кладём после всех верхов: верх поднятой стены ряда ниже
            // рисуется позже и перекрыл бы её на стыке, оставив светлый зуб.
            late.push([ex, py, 1.5, bottom - py, edge]);
          }
          if (raised) {
            // Ребро, которым кончается лицо.
            const fx = s < 0 ? px - 1 : px + T;
            late.push([fx, py + T - h, 1, h, shade(wall, -0.3)]);
          }
          return bottom - py;
        };
        if (left) {
          // Тень стены на пол — только там, где стена лежит рядом верхом. Лицо
          // поднятой стены отбрасывает тень на юг, а не вбок: полоса вдоль него
          // ложилась бы чёрной щелью на углу.
          const span = side(-1);
          if (span > 0) ground.rect(px, py, T * 0.12, span).fill({ color: 0, alpha: 0.16 });
        }
        if (right) side(1);
        if (down) {
          // Южная стена: верх и тонкая передняя кромка — лицом она от камеры.
          const x0 = px - (left ? wt : 0);
          const x1 = px + T + (right ? wt : 0);
          caps.rect(x0, py + T, x1 - x0, wt).fill(wallTop);
          caps.rect(x0, py + T, x1 - x0, 1.5).fill(shade(wallTop, 0.2));
          caps.rect(x0, py + T + wt, x1 - x0, 3).fill(shade(wall, -0.35));
        }
      }
    }

    for (const [rx, ry, rw, rh, c] of late) caps.rect(rx, ry, rw, rh).fill(c);


  }

  // Опущенные стены: лицо на кромке пола, в скалу заходит только `LIFT`.
  function lowWalls() {
    const h = T * LOW;
    const lift = T * LIFT;
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        if (at(x, y) < 0) continue;
        const px = x * T;
        const py = y * T;
        const up = isRock(x, y - 1);
        const down = isRock(x, y + 1);
        const left = isRock(x - 1, y);
        const right = isRock(x + 1, y);
        const x0 = px - (left ? wt : 0);
        const x1 = px + T + (right ? wt : 0);

        if (up) {
          const fy = py - lift;
          wallFace(faces, px, fy, T, h, x, y, tints[at(x, y)] ?? 0x464b57, d);
          // Лицо кончается ребром там, где рядом пол без своей северной стены:
          // у прохода в коридор и на ступеньке комнаты.
          if (!left && !isRock(x - 1, y) && !isRock(x - 1, y - 1)) {
            faces.rect(px, fy, 1, h).fill(shade(wall, -0.3));
          }
          if (!right && !isRock(x + 1, y) && !isRock(x + 1, y - 1)) {
            faces.rect(px + T - 1, fy, 1, h).fill(shade(wall, -0.3));
          }
          caps.rect(x0, fy - wt, x1 - x0, wt).fill(wallTop);
          caps.rect(x0, fy - wt, x1 - x0, 1.5).fill(shade(wallTop, 0.2));
          // Угловые столбы: боковая стена доходит до поднятого верха северной.
          if (left) caps.rect(px - wt, fy, wt, lift).fill(wallTop);
          if (right) caps.rect(px + T, fy, wt, lift).fill(wallTop);
          // Тень стены на пол под лицом.
          ground.rect(px, fy + h, T, T * 0.2).fill({ color: 0, alpha: 0.22 });
        }
        if (left) {
          caps.rect(px - wt, py, wt, T).fill(wallTop);
          caps.rect(px - 1.5, py, 1.5, T).fill(edge);
          ground.rect(px, py, T * 0.12, T).fill({ color: 0, alpha: 0.16 });
        }
        if (right) {
          caps.rect(px + T, py, wt, T).fill(wallTop);
          caps.rect(px + T, py, 1.5, T).fill(edge);
        }
        if (down) {
          // Южная стена: верх и тонкая передняя кромка — лицом она от камеры.
          caps.rect(x0, py + T, x1 - x0, wt).fill(wallTop);
          caps.rect(x0, py + T, x1 - x0, 1.5).fill(shade(wallTop, 0.2));
          caps.rect(x0, py + T + wt, x1 - x0, 3).fill(shade(wall, -0.35));
        }
      }
    }
    // Внутренние углы скалы: пол с двух сторон по диагонали, полосы двух клеток
    // сходятся, и квадрат стыка закрываем ровной заливкой. Только сверху: снизу
    // стык закрывает поднятый верх северной стены, а квадрат лёг бы на её лицо.
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        if (at(x, y) >= 0) continue;
        for (const [dx, dy] of [
          [-1, -1],
          [1, -1],
        ]) {
          if (at(x + dx, y) < 0 || at(x, y + dy) < 0) continue;
          caps.rect(dx < 0 ? x * T : x * T + T - wt, y * T, wt, wt).fill(wallTop);
        }
      }
    }


  }

  // 4. Мебель — после лиц стен: то, что стоит у северной стены, стоит перед ней.
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const v = at(x, y);
      if (v < 0) continue;
      const def = palette[v];
      if (def) prop(props, def.id, x, y, tints[v] ?? 0x3a3e47, noise(x, y, 7), d);
    }
  }
}

function floorCell(g, mat, x, y, T, base, d) {
  const { noise, shade } = d;
  const px = x * T;
  const py = y * T;
  const n = noise(x, y, 1);
  switch (mat) {
    case "wood": {
      // Доски поперёк, со сдвинутыми стыками.
      const rows = 4;
      const bh = T / rows;
      for (let i = 0; i < rows; i++) {
        const c = shade(base, (noise(x, y, 50 + i) - 0.5) * 0.18);
        g.rect(px, py + i * bh, T, bh).fill(c);
        g.rect(px, py + i * bh + bh - 1, T, 1).fill({ color: 0x1e140c, alpha: 0.7 });
        const j = ((x * 3 + i * 5 + y) % 4) * (T / 4);
        g.rect(px + j, py + i * bh, 1, bh).fill({ color: 0x1e140c, alpha: 0.6 });
        // Волокно.
        g.rect(px + noise(x, y, 60 + i) * T * 0.6, py + i * bh + bh * 0.45, T * 0.3, 1).fill({
          color: 0x000000,
          alpha: 0.12,
        });
      }
      break;
    }
    case "tile": {
      // Мелкая плитка 2×2 с белым швом.
      const s = T / 2;
      for (let j = 0; j < 2; j++) {
        for (let i = 0; i < 2; i++) {
          const c = shade(base, (noise(x * 2 + i, y * 2 + j, 3) - 0.5) * 0.1);
          g.rect(px + i * s, py + j * s, s, s).fill(c);
          g.rect(px + i * s, py + j * s, s, 1).fill({ color: 0xffffff, alpha: 0.18 });
          g.rect(px + i * s + s - 1, py + j * s, 1, s).fill({ color: 0x000000, alpha: 0.25 });
        }
      }
      break;
    }
    case "plate": {
      // Рифлёная сталь: ромбики насечки.
      g.rect(px, py, T, T).fill(shade(base, (n - 0.5) * 0.08));
      for (let j = 0; j < 4; j++) {
        for (let i = 0; i < 4; i++) {
          const cx = px + (i + 0.5 + (j % 2) * 0.5) * (T / 4);
          const cy = py + (j + 0.5) * (T / 4);
          if (cx > px + T - 2) continue;
          g.moveTo(cx - 2.5, cy + 1)
            .lineTo(cx + 2.5, cy - 1)
            .stroke({ color: 0xffffff, width: 1, alpha: 0.12 });
        }
      }
      g.rect(px, py + T - 1, T, 1).fill({ color: 0, alpha: 0.5 });
      g.rect(px + T - 1, py, 1, T).fill({ color: 0, alpha: 0.5 });
      break;
    }
    case "rubber": {
      // Тёмное покрытие цеха с масляными пятнами.
      g.rect(px, py, T, T).fill(shade(base, (n - 0.5) * 0.1));
      if (n > 0.55) {
        g.ellipse(
          px + T * (0.3 + noise(x, y, 2) * 0.4),
          py + T * (0.3 + noise(x, y, 3) * 0.4),
          T * (0.12 + noise(x, y, 4) * 0.18),
          T * (0.08 + noise(x, y, 5) * 0.1),
        ).fill({ color: 0x000000, alpha: 0.25 });
      }
      g.rect(px, py + T - 1, T, 1).fill({ color: 0, alpha: 0.35 });
      g.rect(px + T - 1, py, 1, T).fill({ color: 0, alpha: 0.35 });
      break;
    }
    case "carpet": {
      g.rect(px, py, T, T).fill(base);
      for (let i = 0; i < 6; i++) {
        g.rect(px + noise(x, y, 70 + i) * (T - 2), py + noise(x, y, 80 + i) * (T - 2), 2, 2).fill({
          color: 0xffffff,
          alpha: 0.05,
        });
      }
      break;
    }
    default: {
      // Бетон плитами со швом — дословно плоский вид.
      g.rect(px, py, T, T).fill(shade(base, (n - 0.5) * 0.12));
      g.rect(px, py, T, 1).fill({ color: 0xffffff, alpha: 0.05 });
      g.rect(px, py, 1, T).fill({ color: 0xffffff, alpha: 0.04 });
      g.rect(px, py + T - 1, T, 1).fill({ color: 0x1c1e24, alpha: 0.9 });
      g.rect(px + T - 1, py, 1, T).fill({ color: 0x1c1e24, alpha: 0.9 });
      if (n > 0.75) {
        g.ellipse(
          px + T * (0.3 + noise(x, y, 2) * 0.4),
          py + T * (0.3 + noise(x, y, 3) * 0.4),
          T * 0.22,
          T * 0.14,
        ).fill({ color: 0x000000, alpha: 0.12 });
      }
    }
  }
}

/// Лицо стены: панели, плинтус и изредка что-то на стене.
function wallFace(g, x, y, w, h, cx, cy, tint, d) {
  const { noise, shade, mix } = d;
  // Стена покрашена в цвет комнаты: так комната читается и от двери, как на
  // референсе, а пол остаётся материалом.
  const paint = mix(0x5a5f6c, tint, 0.45);
  const panel = shade(paint, -0.28);
  g.rect(x, y, w, h).fill(paint);
  // Свет сверху: верх грани светлее низа.
  g.rect(x, y, w, h * 0.3).fill({ color: 0xffffff, alpha: 0.06 });
  // Нижняя панель (ламбрис) темнее, с кромкой; стык листов и плинтус.
  g.rect(x, y + h * 0.55, w, h * 0.45).fill(panel);
  g.rect(x, y + h * 0.55, w, 1.5).fill(shade(paint, 0.25));
  g.rect(x + w * 0.5, y + h * 0.55, 1, h * 0.45).fill({ color: 0x000000, alpha: 0.3 });
  g.rect(x, y + h - 3, w, 3).fill(shade(panel, -0.4));
  const n = noise(cx, cy, 90);
  const { T } = d;
  const mx = x + w * 0.5;
  if (n > 0.86) {
    // Вентиляционная решётка.
    g.rect(mx - T * 0.18, y + h * 0.14, T * 0.36, h * 0.24).fill(shade(paint, -0.45));
    for (let i = 0; i < 3; i++) {
      g.rect(mx - T * 0.16, y + h * 0.17 + i * h * 0.07, T * 0.32, 1).fill(shade(paint, 0.15));
    }
  } else if (n > 0.72) {
    // Бра: тёплое пятно на стене над светильником.
    g.ellipse(mx, y + h * 0.3, T * 0.36, h * 0.3).fill({ color: 0xffd890, alpha: 0.1 });
    g.rect(mx - T * 0.08, y + h * 0.26, T * 0.16, h * 0.1).fill(0xffe6b0);
    g.rect(mx - T * 0.1, y + h * 0.36, T * 0.2, 2).fill(0x3a3d46);
  } else if (n > 0.6) {
    // Труба вдоль стены.
    g.rect(x, y + h * 0.1, w, 3).fill(0x6a6f7c);
    g.rect(x, y + h * 0.1, w, 1).fill(0x9aa0ae);
  } else if (n > 0.52) {
    // Плакат.
    g.rect(mx - T * 0.13, y + h * 0.1, T * 0.26, h * 0.36).fill(0xcfc4a2);
    g.rect(mx - T * 0.1, y + h * 0.14, T * 0.2, h * 0.16).fill(shade(tint, 0.2));
  }
}

/// Мебель одной клетки в 3/4.
function prop(g, id, x, y, tint, n, d) {
  const { T, shade, tileDefAt } = d;
  const px = x * T;
  const py = y * T;
  const dark = 0x0b0d12;
  // Коробка: след на полу (rx, ry, rw, rd) в долях клетки, высота `h`.
  // Возвращает пиксельный прямоугольник верха — на нём рисуют детали.
  const box = (rx, ry, rw, rd, h, top, front = shade(top, -0.38)) => {
    const x0 = px + rx * T;
    const y0 = py + ry * T;
    const w = rw * T;
    const dd = rd * T;
    const hh = h * T;
    g.rect(x0 + 2, y0 + 2.5, w, dd).fill({ color: 0, alpha: 0.35 });
    g.rect(x0, y0 + dd - hh, w, hh).fill(front);
    g.rect(x0, y0 - hh, w, dd).fill(top);
    g.rect(x0, y0 - hh, w, 1).fill({ color: 0xffffff, alpha: 0.18 });
    return { x: x0, y: y0 - hh, w, d: dd };
  };
  switch (id) {
    case "bed":
    case "ward": {
      const white = id === "ward";
      const frame = white ? 0xa8b0b8 : 0x6a4a30;
      // Изголовье у северного края — выше рамы.
      box(0.12, 0.08, 0.76, 0.08, 0.36, shade(frame, 0.1));
      const t = box(0.14, 0.14, 0.72, 0.74, 0.16, frame);
      g.roundRect(t.x + 1, t.y + 1, t.w - 2, t.d - 2, 3).fill(white ? 0xdcdad2 : 0x8f8a7c);
      g.roundRect(t.x + 3, t.y + 3, t.w - 6, t.d * 0.26, 2).fill(white ? 0xf4f2ea : 0xe9e4d4);
      g.roundRect(t.x + 1, t.y + t.d * 0.42, t.w - 2, t.d * 0.56, 3).fill(
        white ? 0xb8c4cc : shade(tint, 0.25),
      );
      g.rect(t.x + 1, t.y + t.d * 0.42, t.w - 2, 2).fill(white ? 0xdde4ea : shade(tint, 0.5));
      if (white) {
        const cx = px + T / 2;
        // Середина передней грани изголовья.
        const cy = py + T * (0.16 - 0.18);
        g.rect(cx - 1.5, cy - 4, 3, 8).fill(0xd64a5a);
        g.rect(cx - 4, cy - 1.5, 8, 3).fill(0xd64a5a);
      }
      break;
    }
    case "nest": {
      const cx = px + T / 2;
      const cy = py + T / 2;
      const r = T * 0.36;
      const h = T * 0.16;
      g.ellipse(cx + 2, cy + 3, r, r * 0.8).fill({ color: 0, alpha: 0.35 });
      g.ellipse(cx, cy, r, r * 0.8).fill(0x4a3620);
      g.rect(cx - r, cy - h, r * 2, h).fill(0x4a3620);
      g.ellipse(cx, cy - h, r, r * 0.8).fill(0x7a5a3a);
      g.ellipse(cx, cy - h, r * 0.72, r * 0.56).fill(shade(tint, 0.3));
      break;
    }
    case "storage": {
      // Паллета: низкая, пустая (содержимое рисует `drawScrap`).
      const t = box(0.12, 0.14, 0.76, 0.74, SURFACE.storage, 0x6a5638, 0x3a2e1e);
      for (let i = 1; i < 3; i++) {
        g.rect(t.x, t.y + i * (t.d / 3), t.w, 1.5).fill(0x3a2e1e);
      }
      g.rect(px + T * 0.06, py + T * 0.06, T * 0.88, T * 0.88).stroke({
        color: 0xd9b64a,
        width: 1,
        alpha: 0.3,
      });
      break;
    }
    case "bin": {
      const t = box(0.18, 0.24, 0.64, 0.56, 0.34, shade(tint, 0.15));
      g.rect(t.x, t.y, t.w, t.d * 0.3).fill(shade(tint, 0.4));
      g.rect(t.x, t.y, t.w, t.d).stroke({ color: dark, width: 1 });
      break;
    }
    case "rack": {
      // Стеллаж — низкий стальной стол с полкой под ним. Высокий шкаф
      // закрывал своим верхом клетку позади (у северной стены — саму стену) и
      // читался серой плитой, а кучи лежали на его передней грани. Здесь верх
      // ниже середины клетки: куча, которую вид кладёт в центр клетки, ложится
      // **на столешницу**. Соседние стеллажи сливаются в один стеллаж по обеим
      // осям: стойки и кромки — только по краю ряда.
      //
      // Материал — нейтральная сталь, темнее любой стены: стена склада
      // покрашена в цвет комнаты, и столешница в тон сливалась с ней в одну
      // полосу. Паллета рядом — дерево, так два вида хранения не путаются.
      const h = T * SURFACE.rack;
      const steel = 0x50555d;
      const frame = 0x3a3e45;
      const isRack = (dx, dy) => tileDefAt(x + dx, y + dy)?.id === "rack";
      const l = isRack(-1, 0);
      const r = isRack(1, 0);
      const u = isRack(0, -1);
      // Передняя грань: тень под столешницей и полка внизу. Стеллаж ряда ниже
      // закроет её своим верхом сам.
      g.rect(px, py + T - h, T, h).fill(0x1b1e23);
      g.rect(px, py + T - h * 0.42, T, h * 0.18).fill(shade(steel, -0.15));
      g.rect(px, py + T - h * 0.24, T, h * 0.24).fill({ color: 0, alpha: 0.35 });
      // Щель за стеллажом: тёмная полоса по заднему краю отделяет его от
      // стены и от того, что стоит позади.
      if (!u) g.rect(px, py - h - 2, T, 2).fill({ color: 0, alpha: 0.55 });
      // Столешница: стальной настил с поперечными рёбрами.
      g.rect(px, py - h, T, T).fill(steel);
      for (let i = 1; i < 4; i++) {
        g.rect(px, py - h + (i * T) / 4, T, 1).fill({ color: 0, alpha: 0.22 });
      }
      if (!u) g.rect(px, py - h, T, 1.5).fill(shade(steel, 0.35));
      g.rect(px, py + T - h - 2, T, 2).fill(shade(steel, 0.18));
      // Стойки — по краю ряда; между соседями шов, а не две стойки подряд.
      if (!l) g.rect(px, py - h, 3, T + h).fill(frame);
      if (!r) g.rect(px + T - 3, py - h, 3, T + h).fill(frame);
      if (l) g.rect(px, py - h, 1, T).fill({ color: 0, alpha: 0.18 });
      break;
    }
    case "garage": {
      // Шлюз плоский: по нему ходят. Полосатая кромка и стрелка выезда.
      for (let i = 0; i < 4; i++) {
        g.moveTo(px + i * T * 0.25, py + T)
          .lineTo(px + i * T * 0.25 + T * 0.12, py + T - T * 0.14)
          .lineTo(px + i * T * 0.25 + T * 0.22, py + T - T * 0.14)
          .lineTo(px + i * T * 0.25 + T * 0.1, py + T)
          .closePath()
          .fill(0xd9b64a);
      }
      g.moveTo(px + T * 0.5, py + T * 0.16)
        .lineTo(px + T * 0.7, py + T * 0.42)
        .lineTo(px + T * 0.3, py + T * 0.42)
        .closePath()
        .fill({ color: 0xd9b64a, alpha: 0.6 });
      break;
    }
    case "desk": {
      box(0.32, 0.36, 0.36, 0.32, 0.2, shade(tint, 0.2));
      break;
    }
    case "desk_table": {
      const t = box(0.08, 0.2, 0.84, 0.6, 0.3, 0x8a6a44);
      g.rect(t.x + t.w * 0.3, t.y + t.d * 0.2, t.w * 0.4, t.d * 0.6).fill(0xe6e1d2);
      break;
    }
    case "lectern": {
      const [dx, dy] = [
        [-1, 0],
        [1, 0],
        [0, -1],
        [0, 1],
      ].find(([ox, oy]) => tileDefAt(x + ox, y + oy)?.id === "desk_table") ?? [0, 1];
      // Спинка со стороны, дальней от стола; северную рисуем до сиденья, южную после.
      const back = shade(tint, 0.35);
      if (dy > 0) box(0.3, 0.26, 0.4, 0.08, 0.5, back);
      box(0.3, 0.32, 0.4, 0.38, 0.2, shade(tint, 0.2));
      if (dx > 0) box(0.22, 0.32, 0.08, 0.38, 0.5, back);
      if (dx < 0) box(0.7, 0.32, 0.08, 0.38, 0.5, back);
      if (dy < 0) box(0.3, 0.7, 0.4, 0.08, 0.5, back);
      break;
    }
    case "muster_table": {
      const cx = px + T / 2;
      const cy = py + T / 2;
      const r = T * 0.44;
      const h = T * 0.3;
      g.ellipse(cx + 2, cy + 3, r, r * 0.86).fill({ color: 0, alpha: 0.35 });
      g.rect(cx - T * 0.06, cy - h, T * 0.12, h).fill(0x4a3a26);
      g.ellipse(cx, cy - h + 3, r, r * 0.86).fill(0x5a4630);
      g.ellipse(cx, cy - h, r, r * 0.86).fill(0x8a6a44);
      g.rect(cx - T * 0.2, cy - h - T * 0.16, T * 0.4, T * 0.32).fill(0xd9cfae);
      g.moveTo(cx - T * 0.14, cy - h + T * 0.1)
        .lineTo(cx - T * 0.02, cy - h - T * 0.08)
        .lineTo(cx + T * 0.12, cy - h + T * 0.02)
        .stroke({ color: 0xb04a3a, width: 1.5 });
      break;
    }
    case "lab": {
      const t = box(0.08, 0.4, 0.84, 0.46, 0.32, 0x7a808c);
      const fl = [0x7fd6a0, 0xb08fde, 0x6ee0e8];
      for (let i = 0; i < 3; i++) {
        const fx = t.x + t.w * (0.18 + i * 0.32);
        const fy = t.y + t.d * 0.5;
        g.rect(fx - 1.5, fy - T * 0.2, 3, T * 0.12).fill(0xc8d0dc);
        g.circle(fx, fy - T * 0.06, T * 0.09).fill(fl[i]);
        g.circle(fx - T * 0.03, fy - T * 0.09, T * 0.025).fill({ color: 0xffffff, alpha: 0.7 });
      }
      break;
    }
    case "bench": {
      const t = box(0, 0.06, 1, 0.94, SURFACE.bench, 0x6c6580);
      g.rect(t.x + 3, t.y + t.d * 0.25, t.w * 0.35, t.d * 0.4).fill(0x1a2a30);
      g.rect(t.x + 4, t.y + t.d * 0.28, t.w * 0.35 - 2, 2).fill(0x6ee0e8);
      g.circle(t.x + t.w * 0.7, t.y + t.d * 0.45, 2.5).fill(0xb08fde);
      g.circle(t.x + t.w * 0.84, t.y + t.d * 0.45, 2.5).fill(0x6cf0a0);
      break;
    }
    case "shop": {
      const t = box(0.06, 0.36, 0.88, 0.5, 0.32, 0x8a6a3c);
      // Тиски и шестерня на столешнице.
      g.rect(t.x + t.w * 0.62, t.y - 4, t.w * 0.24, t.d * 0.5 + 4).fill(0x8a8f9c);
      g.rect(t.x + t.w * 0.62, t.y - 4, t.w * 0.24, 2).fill(0xc0c4cc);
      const gx = t.x + t.w * 0.28;
      const gy = t.y + t.d * 0.4;
      g.circle(gx, gy, T * 0.12).fill(0x8a8f9c);
      g.circle(gx, gy, T * 0.05).fill(0x3a3d46);
      break;
    }
    case "post": {
      // Прилавок и навес на двух стойках.
      g.rect(px + T * 0.12, py + T * 0.2 - T * 0.72, 2, T * 0.72).fill(0x5a4630);
      g.rect(px + T * 0.86, py + T * 0.2 - T * 0.72, 2, T * 0.72).fill(0x5a4630);
      const t = box(0.08, 0.5, 0.84, 0.36, 0.38, 0x8a6a40);
      g.rect(t.x + t.w * 0.6, t.y + 2, t.w * 0.25, t.d * 0.5).fill(0xb8bcc4);
      for (let i = 0; i < 5; i++) {
        g.rect(px + T * (0.08 + i * 0.168), py + T * 0.2 - T * 0.8, T * 0.168, T * 0.16).fill(
          i % 2 ? 0xd9b64a : 0xe6e1d2,
        );
      }
      break;
    }
    case "relay": {
      const t = box(0.15, 0.5, 0.7, 0.36, 0.3, 0x4a5058);
      g.circle(t.x + t.w * 0.25, t.y + t.d * 0.5, 2).fill(0x6cf0a0);
      g.circle(t.x + t.w * 0.45, t.y + t.d * 0.5, 2).fill(0xff9a3c);
      const mx = px + T * 0.72;
      const top = py + T * 0.5 - T * 1.05;
      g.rect(mx - 1, top, 2, T * 1.05 - T * 0.3).fill(0x9aa0ae);
      g.moveTo(mx - T * 0.2, top)
        .lineTo(mx, top + T * 0.14)
        .lineTo(mx + T * 0.2, top)
        .stroke({ color: 0x9aa0ae, width: 1.5 });
      break;
    }
    default:
      break;
  }
}

/// Высота северной стены в клетках — свету: луч, вошедший в неё снизу, обязан
/// осветить всё лицо, а не кромку. У опущенной стены лицо лежит на полу.
export const wallRise = (lowered) => (lowered ? 0 : RISE);

/// Высота поверхности, на которой лежат кучи, в долях клетки. Куча рисуется
/// поднятой на эту высоту — **на** столешницу или паллету, а не на пол под ней:
/// столешница стеллажа сдвинута вверх, и куча в центре клетки оказывалась у её
/// переднего края, а куча с паллеты позади — на задней кромке стеллажа, то
/// есть читалась лежащей на нём.
const SURFACE = { rack: 0.3, bench: 0.36, storage: 0.07 };
export const surfaceRise = (id) => SURFACE[id] ?? 0;

/// Тайлы, чья мебель закрывает клетку целиком: в виде 3/4 глиф роли поверх них
/// не ставится — бледный значок на столешнице читается грязью, а роль и так
/// видна по самой мебели.
export const COVERS_CELL = new Set(["rack", "bench"]);
