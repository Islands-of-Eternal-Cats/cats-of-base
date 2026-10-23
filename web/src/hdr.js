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
    vec3 light = uAmbient + texture(uLight, uv).rgb * uGain;
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
  });
  const filter = new Filter({
    glProgram: GlProgram.from({ vertex, fragment, name: "hdr-light" }),
    resources: {
      hdrUniforms: uniforms,
      uLight: Texture.WHITE.source,
      uBloom: Texture.EMPTY.source,
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
