// HDR-сведение света — прототип, только вид.
//
// Один фильтр на мировые слои (пол, кучи, мебель, коты): цвет пикселя
// умножается на свет из float-текстуры (`rgba16float`, значения больше
// единицы законны), к нему прибавляется свечение (размытый яркий свет),
// дальше экспозиция, насыщенность и тонмаппинг ACES. Ограничения `multiply`
// (свет только затемняет, цвет по синеватому полу выходит серым, перекрытие
// ламп уходит в белое) тут нет по построению: яркое не обрезается, а мягко
// сжимается, и оттенок у него остаётся.
//
// Свет лежит в координатах мира, а фильтр работает в координатах экрана,
// поэтому вершинный шейдер отдаёт фрагментному глобальную позицию пикселя, а
// `uLightRect` — где на экране лежит карта (пишет `light.js` каждым кадром).
//
// **Рельеф (bump).** Плоская заливка под светом читается картоном: свет
// ложится на неё ровным пятном. Высота берётся из карты высот `uHeight` —
// это сама карта тайлов, снятая в текстуру на её пересборке (`light.js`):
// светлое выше тёмного, так шов плиты уходит в канавку, блик стены — в ребро,
// крап скалы — в бугор. Сверху кладётся мелкое зерно (шум в координатах
// мира, неподвижный при прокрутке и масштабе): без него внутри однотонной
// заливки нормаль плоская, и рельеф был бы только по краям. Нормаль считается
// разностями высоты в пиксель мира.
//
// Свет при этом направленный, хотя текстура света — только яркость: куда
// светит лампа, читается её **градиентом** (свет ярче к лампе), а для фона
// (`uAmbient`) взят постоянный свет сверху-слева. Плоская поверхность при
// этом освещена ровно как без рельефа — множитель нормирован на `L.z`.
//
// **Объём котов.** Кот нарисован плоской заливкой, и под лампой это
// картонка. Тела у него считаются честно: голова — шар, корпус, руки и лапы —
// капсулы (`catShapes` в виде, в координатах `drawCat`). Каждым кадром
// `light.js` кладёт в `uCats` обратный трансформ каждого кота (поле света →
// корпус и → передняя рука): пиксель переводится в координаты кота, там
// спрашивается часть, а её нормаль возвращается в мир транспонированной
// матрицей — так зеркало разворота, бок спящего и взмах руки работают сами.
// Что не тело (уши, хвост, ноша) — по размытому силуэту (`uBodySoft`): к
// краю ниже, к середине выше; маска — сам силуэт (`uBody`), иначе вокруг
// кота по полу шёл бы ореол.
//
// Рельеф только у карты: кот, куча и чертёж поверх пола свою краску имеют, и
// там, где цвет пикселя расходится с картой высот, нормаль плоская — иначе
// по коту ехала бы плитка пола под ним.

import { Filter, GlProgram, Texture, UniformGroup } from "pixi.js";

/// Сколько котов и частей тела шейдер берёт: котов на базе меньше десятка
/// (с запасом на найм), частей у `catShapes` шесть.
const MAX_CATS = 32;
const MAX_PARTS = 8;

const vertex = `
in vec2 aPosition;
out vec2 vTextureCoord;
out vec2 vGlobal;

uniform vec4 uInputSize;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;

void main(void)
{
    vec2 position = aPosition * uOutputFrame.zw + uOutputFrame.xy;
    vGlobal = position;
    position.x = position.x * (2.0 / uOutputTexture.x) - 1.0;
    position.y = position.y * (2.0 * uOutputTexture.z / uOutputTexture.y) - uOutputTexture.z;
    gl_Position = vec4(position, 0.0, 1.0);
    vTextureCoord = aPosition * (uOutputFrame.zw * uInputSize.zw);
}
`;

const fragment = `
#define MAX_CATS ${MAX_CATS}
#define MAX_PARTS ${MAX_PARTS}
in vec2 vTextureCoord;
in vec2 vGlobal;
out vec4 finalColor;

uniform sampler2D uTexture;
uniform sampler2D uLight;
uniform sampler2D uBloom;

uniform vec4 uLightRect;
uniform vec3 uAmbient;
uniform float uGain;
uniform float uExposure;
uniform float uSat;
uniform float uBloomK;
uniform float uBloomT;

uniform sampler2D uHeight;
uniform vec2 uMapPx;   // поле света в пикселях мира
uniform float uTile;   // клетка в пикселях мира
uniform float uBump;   // сила рельефа, 0 — выключен
uniform float uGrain;  // доля зерна в высоте
uniform sampler2D uBody;     // силуэт котов (альфа)
uniform sampler2D uBodySoft; // он же размытый — высота купола
uniform float uRound;        // сила объёма по силуэту, 0 — выключен
// Тела котов: по три vec4 на кота — (a, b, c, d), (tx, ty, A, B), (C, D, TX, TY):
// трансформ поля в корпус и в переднюю руку, как у Matrix в Pixi.
uniform vec4 uCats[MAX_CATS * 3];
uniform float uCatN;
// Части: по два vec4 — (cx, y0, y1, rx), (ry, рука ли, 0, 0).
uniform vec4 uParts[MAX_PARTS * 2];
uniform float uPartN;
uniform float uReach;  // дальше этого от начала кота частей нет
uniform float uShape;  // сила наклона нормалей тел, 0 — выключен

// Часть тела — капсула с вертикальной осью (шар — капсула нулевой длины):
// нормаль в координатах кота и признак попадания в w.
vec4 part(vec2 p, vec4 a, vec4 b)
{
    float rx = a.w;
    float ry = b.x;
    float qx = (p.x - a.x) / rx;
    float qy = (p.y - clamp(p.y, a.y + ry, a.z - ry)) / ry;
    float s = qx * qx + qy * qy;
    if (s >= 1.0) return vec4(0.0);
    return vec4(qx, qy * rx / ry, sqrt(1.0 - s), 1.0);
}

// Нормаль тела кота под пикселем поля (w — попали ли); позже нарисованный
// кот — впереди.
vec4 catNormal(vec2 pw)
{
    vec4 got = vec4(0.0);
    for (int i = 0; i < MAX_CATS; i++) {
        if (float(i) >= uCatN) break;
        vec4 m0 = uCats[i * 3];
        vec4 m1 = uCats[i * 3 + 1];
        vec4 m2 = uCats[i * 3 + 2];
        vec2 pb = vec2(m0.x * pw.x + m0.z * pw.y + m1.x, m0.y * pw.x + m0.w * pw.y + m1.y);
        if (dot(pb, pb) > uReach * uReach) continue;
        vec2 pa = vec2(m1.z * pw.x + m2.x * pw.y + m2.z, m1.w * pw.x + m2.y * pw.y + m2.w);
        for (int k = 0; k < MAX_PARTS; k++) {
            if (float(k) >= uPartN) break;
            vec4 a = uParts[k * 2];
            vec4 b = uParts[k * 2 + 1];
            bool arm = b.y > 0.5;
            vec4 h = part(arm ? pa : pb, a, b);
            if (h.w > 0.0) {
                vec2 g = h.xy * uShape;
                vec2 gw = arm
                    ? vec2(m1.z * g.x + m1.w * g.y, m2.x * g.x + m2.y * g.y)
                    : vec2(m0.x * g.x + m0.y * g.y, m0.z * g.x + m0.w * g.y);
                got = vec4(normalize(vec3(gw, h.z)), 1.0);
                break;
            }
        }
    }
    return got;
}

float hash(vec2 p)
{
    vec3 q = fract(vec3(p.xyx) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

float vnoise(vec2 p)
{
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

float heightAt(vec2 uv)
{
    vec2 w = uv * uMapPx;
    float h = dot(texture(uHeight, uv).rgb, vec3(0.299, 0.587, 0.114));
    float g = vnoise(w * 0.45) * 0.65 + vnoise(w * 1.7) * 0.35;
    return h + g * uGrain;
}

float lum(vec3 c)
{
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// ACES (аппроксимация Нарковича): яркое сходится к белому плавно и
// сохраняет оттенок дольше, чем простое обрезание.
vec3 aces(vec3 x)
{
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

void main(void)
{
    vec4 src = texture(uTexture, vTextureCoord);
    if (src.a <= 0.0) {
        finalColor = src;
        return;
    }
    vec3 albedo = src.rgb / src.a; // вход премультиплицирован
    vec2 uv = (vGlobal - uLightRect.xy) / uLightRect.zw;
    vec3 lamp = texture(uLight, uv).rgb * uGain;
    vec3 light = uAmbient + lamp;
    if (uBump > 0.0 || uRound > 0.0 || uShape > 0.0) {
        // Нормаль по карте высот; где пиксель не карта (кот, куча) — плоская.
        vec2 e = 1.0 / uMapPx;
        vec3 n = vec3(0.0, 0.0, 1.0);
        if (uBump > 0.0) {
            float hx = heightAt(uv + vec2(e.x, 0.0)) - heightAt(uv - vec2(e.x, 0.0));
            float hy = heightAt(uv + vec2(0.0, e.y)) - heightAt(uv - vec2(0.0, e.y));
            float onMap = 1.0 - smoothstep(0.05, 0.14, distance(albedo, texture(uHeight, uv).rgb));
            n = normalize(vec3(-hx * uBump * onMap, -hy * uBump * onMap, 1.0));
        }
        vec4 cat = uShape > 0.0 ? catNormal(uv * uMapPx) : vec4(0.0);
        float body = uRound > 0.0 ? texture(uBody, uv).a : 0.0;
        if (cat.w > 0.0) {
            n = cat.xyz;
        } else if (body > 0.0) {
            vec2 b = 1.5 * e;
            float bx = texture(uBodySoft, uv + vec2(b.x, 0.0)).a - texture(uBodySoft, uv - vec2(b.x, 0.0)).a;
            float by = texture(uBodySoft, uv + vec2(0.0, b.y)).a - texture(uBodySoft, uv - vec2(0.0, b.y)).a;
            vec3 nb = normalize(vec3(-bx * uRound, -by * uRound, 1.0));
            n = normalize(mix(n, nb, smoothstep(0.0, 0.6, body)));
        }
        // Куда светят лампы: градиент яркости света на ~полклетки, отнесённый
        // к самой яркости — направление, а не сила.
        vec2 le = vec2(uTile * 0.5) / uMapPx;
        float gx = lum(texture(uLight, uv + vec2(le.x, 0.0)).rgb) - lum(texture(uLight, uv - vec2(le.x, 0.0)).rgb);
        float gy = lum(texture(uLight, uv + vec2(0.0, le.y)).rgb) - lum(texture(uLight, uv - vec2(0.0, le.y)).rgb);
        vec2 dir = vec2(gx, gy) / (lum(lamp) / max(uGain, 1e-3) + 0.03);
        float dl = length(dir);
        if (dl > 2.5) dir *= 2.5 / dl;
        vec3 L = normalize(vec3(dir, 0.7));
        vec3 K = normalize(vec3(-0.45, -0.6, 1.0));
        float fl = clamp(dot(n, L) / L.z, 0.0, 2.2);
        float fa = clamp(dot(n, K) / K.z, 0.0, 2.2);
        light = uAmbient * fa + lamp * fl;
    }
    // Свечение — только от того, что ярче порога: иначе размытый свет всей
    // комнаты ложится молочной дымкой, и большая комната уходит в засвет.
    vec3 hot = max(texture(uBloom, uv).rgb * uGain - uBloomT, 0.0);
    vec3 c = albedo * light + hot * uBloomK;
    c *= uExposure;
    float luma = dot(c, vec3(0.2126, 0.7152, 0.0722));
    c = max(mix(vec3(luma), c, uSat), 0.0);
    c = aces(c);
    finalColor = vec4(c * src.a, src.a);
}
`;

/// Можно ли рисовать во float-текстуру: без этого HDR нечем считать, и свет
/// остаётся в простом режиме.
export function hdrSupported(app) {
  const gl = app.renderer.gl;
  if (!gl || typeof WebGL2RenderingContext === "undefined") return false;
  if (!(gl instanceof WebGL2RenderingContext)) return false;
  return !!gl.getExtension("EXT_color_buffer_float");
}

export function createHdrFilter() {
  const uniforms = new UniformGroup({
    uLightRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
    uAmbient: { value: new Float32Array([0.3, 0.3, 0.35]), type: "vec3<f32>" },
    uGain: { value: 2, type: "f32" },
    uExposure: { value: 1, type: "f32" },
    uSat: { value: 1, type: "f32" },
    uBloomK: { value: 0.3, type: "f32" },
    uBloomT: { value: 0.6, type: "f32" },
    uMapPx: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
    uTile: { value: 28, type: "f32" },
    uBump: { value: 0, type: "f32" },
    uGrain: { value: 0.05, type: "f32" },
    uRound: { value: 0, type: "f32" },
    uCats: {
      value: new Float32Array(MAX_CATS * 12),
      type: "vec4<f32>",
      size: MAX_CATS * 3,
    },
    uCatN: { value: 0, type: "f32" },
    uParts: {
      value: new Float32Array(MAX_PARTS * 8),
      type: "vec4<f32>",
      size: MAX_PARTS * 2,
    },
    uPartN: { value: 0, type: "f32" },
    uReach: { value: 0, type: "f32" },
    uShape: { value: 0, type: "f32" },
  });
  const filter = new Filter({
    // highp: шум зерна считается в пикселях мира (до тысяч), и на mediump
    // хэш рассыпается полосами.
    glProgram: GlProgram.from({
      vertex,
      fragment,
      name: "hdr-light",
      preferredFragmentPrecision: "highp",
    }),
    resources: {
      hdrUniforms: uniforms,
      uLight: Texture.WHITE.source,
      uBloom: Texture.EMPTY.source,
      uHeight: Texture.EMPTY.source,
      uBody: Texture.EMPTY.source,
      uBodySoft: Texture.EMPTY.source,
    },
    // Разрешение экрана, а не единица: иначе текст и тонкие контуры внутри
    // мира мылятся (фильтр рисует слой в текстуру и только потом на экран).
    resolution: "inherit",
    antialias: "inherit",
  });
  const u = uniforms.uniforms;
  return {
    filter,
    /// Текстуры света и свечения (пересоздаются вместе с картой).
    textures(light, bloom) {
      filter.resources.uLight = light.source;
      filter.resources.uBloom = bloom.source;
    },
    /// Карта высот (снимок карты тайлов), размер поля света в пикселях мира
    /// и клетка — зовётся на пересборке карты.
    height(tex, w, h, tile) {
      filter.resources.uHeight = tex.source;
      u.uMapPx[0] = w;
      u.uMapPx[1] = h;
      u.uTile = tile;
      uniforms.update();
    },
    /// Рельеф: сила (0 — выключен) и доля зерна.
    bump(strength, grain) {
      if (u.uBump === strength && u.uGrain === grain) return;
      u.uBump = strength;
      u.uGrain = grain;
      uniforms.update();
    },
    /// Силуэт котов и он же размытый (пересоздаются вместе с полем света).
    bodies(sharp, soft) {
      filter.resources.uBody = sharp.source;
      filter.resources.uBodySoft = soft.source;
    },
    /// Объём котов: сила по силуэту и сила тел, 0 — выключен.
    round(strength, shape) {
      if (u.uRound === strength && u.uShape === shape) return;
      u.uRound = strength;
      u.uShape = shape;
      uniforms.update();
    },
    /// Части тела кота (`catShapes` в виде) — один раз.
    shapes({ reach, parts }) {
      const p = u.uParts;
      p.fill(0);
      parts.slice(0, MAX_PARTS).forEach((s, k) => {
        p.set([s.cx, s.y0, s.y1, s.rx, s.ry, s.arm ? 1 : 0, 0, 0], k * 8);
      });
      u.uPartN = Math.min(parts.length, MAX_PARTS);
      u.uReach = reach;
      uniforms.update();
    },
    /// Куда класть трансформы котов (по 12 чисел) и сколько их влезает.
    catBuffer() {
      return { data: u.uCats, max: MAX_CATS };
    },
    /// Сколько котов положено в `catBuffer` этим кадром.
    cats(n) {
      u.uCatN = n;
      uniforms.update();
    },
    /// Где на экране лежит карта: x, y, ширина, высота.
    rect(x, y, w, h) {
      const r = u.uLightRect;
      if (r[0] === x && r[1] === y && r[2] === w && r[3] === h) return;
      r[0] = x;
      r[1] = y;
      r[2] = w;
      r[3] = h;
      uniforms.update();
    },
    params({ ambient, gain, exposure, sat, bloom }) {
      u.uAmbient[0] = ambient[0];
      u.uAmbient[1] = ambient[1];
      u.uAmbient[2] = ambient[2];
      u.uGain = gain;
      u.uExposure = exposure;
      u.uSat = sat;
      u.uBloomK = bloom;
      uniforms.update();
    },
  };
}
