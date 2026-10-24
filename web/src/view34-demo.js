// Стенд вида 3/4 (`oblique.js`) — синтетическая база со всеми ролями клеток,
// без ядра и без воркера. Нужен, чтобы оценить материал, стены и мебель на
// базе крупнее стартовой; в игре вид включается клавишей V.

import { Application, Container, Graphics } from "pixi.js";
import { drawOblique, surfaceRise } from "./oblique.js";

const TILES = {
  ".": ["floor", "#1b2a3f"],
  S: ["storage", "#26506e"],
  R: ["rack", "#2f6f8e"],
  X: ["bin", "#3f7e8e"],
  L: ["lab", "#5a3a6e"],
  B: ["bench", "#6b4a80"],
  W: ["ward", "#8e4a4a"],
  G: ["garage", "#6e5a26"],
  M: ["muster_table", "#5a4a26"],
  H: ["shop", "#8e6f2f"],
  P: ["post", "#7a6f4a"],
  C: ["relay", "#2f8e7a"],
  b: ["bed", "#3f5a3a"],
  n: ["nest", "#4d7a44"],
  d: ["desk", "#4a4f7a"],
  t: ["desk_table", "#3d4166"],
  l: ["lectern", "#5a5f8a"],
};
const MAP = [
  "############################",
  "#######X##LLLLL#WWW##GG#####",
  "#RRRRRRS##LLBLL#WWW##GG#####",
  "#RSSSSSS##LLLLL#WWW##..#####",
  "####.######.#####.####.#####",
  "#.........................##",
  "#.........................##",
  "##.####.#####.#######.######",
  "#bbbb#.......#HHHH##PP#CC###",
  "#bnnb#...M...#HHHH##PP#CC###",
  "#bbbb#.......#HHHH##..#..###",
  "#bbbb#dtl....#....##..#..###",
  "############################",
];
const ids = Object.keys(TILES);
const palette = ids.map((k) => ({ id: TILES[k][0] }));
const tints = ids.map((k) => parseInt(TILES[k][1].slice(1), 16));
const H = MAP.length;
const W = MAP[0].length;
const cells = MAP.flatMap((row) => [...row].map((ch) => ids.indexOf(ch)));
const at = (x, y) => (x < 0 || y < 0 || x >= W || y >= H ? -1 : cells[y * W + x]);

function cellNoise(x, y, k = 0) {
  let h = (x * 374761393 + y * 668265263 + k * 982451653) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}
function mixColor(a, b, t) {
  const ch = (sh) => {
    const x = (a >> sh) & 0xff;
    const y = (b >> sh) & 0xff;
    return Math.round(x + (y - x) * t) & 0xff;
  };
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}
const shade = (c, t) => mixColor(c, t < 0 ? 0 : 0xffffff, Math.abs(t));

const T = 28;
const app = new Application();
await app.init({ background: 0x0e0f13, resizeTo: window, antialias: true });
document.body.appendChild(app.canvas);
const world = new Container();
app.stage.addChild(world);

const mapLayer = new Container();
world.addChild(mapLayer);
// B — опустить стены, как с инструментом стройки в руке.
let lowered = false;
function draw() {
  for (const n of mapLayer.removeChildren()) n.destroy();
  const [ground, faces, props, caps] = [0, 0, 0, 0].map(() => new Graphics());
  drawOblique({
    lowered,
    ground,
    faces,
    props,
    caps,
    at,
    W,
    H,
    T,
    palette,
    tints,
    tileDefAt: (x, y) => palette[at(x, y)],
    noise: cellNoise,
    shade,
    mix: mixColor,
    rock: 0x16171c,
    rockSpeck: 0x1e2027,
  });
  mapLayer.addChild(ground, faces, props, caps);
}
draw();
window.addEventListener("keydown", (e) => {
  if (e.code !== "KeyB") return;
  lowered = !lowered;
  draw();
});

// Кучи и коты-заглушки — проверить, читаются ли они поверх обстановки.
const extra = new Graphics();
for (const [x, y] of [
  [2, 1],
  [3, 3],
  [6, 2],
  [3, 2],
  [5, 2],
  [12, 5],
  [7, 10],
]) {
  const rise = surfaceRise(palette[at(x, y)]?.id) * T;
  for (let i = 0; i < 3; i++) {
    extra
      .rect(x * T + 6 + i * 3, y * T + T - 10 - i * 4 - rise, 14, 4)
      .fill(i % 2 ? 0xd9b64a : 0xe8c860);
  }
}
for (const [x, y, c] of [
  [4, 5, 0xe0a050],
  [9, 6, 0xc8ccd6],
  [11, 2, 0x9aa0ae],
  [17, 1, 0xe8e2d0],
  [8, 9, 0x6a6a72],
  [15, 9, 0xe0a050],
  [2, 9, 0xc8ccd6],
]) {
  const cx = x * T + T / 2;
  const cy = y * T + T / 2;
  extra.ellipse(cx, cy + 9, 8, 3).fill({ color: 0, alpha: 0.4 });
  extra.roundRect(cx - 6, cy - 4, 12, 13, 5).fill(c);
  extra.circle(cx, cy - 8, 7).fill(c);
  extra.moveTo(cx - 6, cy - 12).lineTo(cx - 4, cy - 18).lineTo(cx - 1, cy - 13).fill(c);
  extra.moveTo(cx + 6, cy - 12).lineTo(cx + 4, cy - 18).lineTo(cx + 1, cy - 13).fill(c);
  extra.circle(cx - 2.5, cy - 8, 1.2).fill(0x111111);
  extra.circle(cx + 2.5, cy - 8, 1.2).fill(0x111111);
}
world.addChild(extra);

function fit() {
  const s = Math.min(app.screen.width / (W * T + 40), app.screen.height / (H * T + 60));
  world.scale.set(s);
  world.x = (app.screen.width - W * T * s) / 2;
  world.y = (app.screen.height - H * T * s) / 2 + 10;
}
fit();
window.addEventListener("resize", fit);
globalThis.__sp = { app, world, toggle: () => { lowered = !lowered; draw(); } };
