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
// Рельеф только у карты: кот, куча и чертёж поверх пола свою краску имеют, и
// там, где цвет пикселя расходится с картой высот, нормаль плоская — иначе
// по коту ехала бы плитка пола под ним.

import { Filter, GlProgram, Texture, UniformGroup } from "pixi.js";

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
    if (uBump > 0.0) {
        // Нормаль по карте высот; где пиксель не карта (кот, куча) — плоская.
        vec2 e = 1.0 / uMapPx;
        float hx = heightAt(uv + vec2(e.x, 0.0)) - heightAt(uv - vec2(e.x, 0.0));
        float hy = heightAt(uv + vec2(0.0, e.y)) - heightAt(uv - vec2(0.0, e.y));
        float onMap = 1.0 - smoothstep(0.05, 0.14, distance(albedo, texture(uHeight, uv).rgb));
        vec3 n = normalize(vec3(-hx * uBump * onMap, -hy * uBump * onMap, 1.0));
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
