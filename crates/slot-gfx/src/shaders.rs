//! Shader sources are GLSL ES 1.00 so the device build compiles them unchanged. Only the
//! preamble differs: the device supplies `precision` defaults and
//! `#define FRAG_COLOR gl_FragColor`, the host maps the ES names onto GL 3.3 core.

use crate::surface::GfxError;

const VERT_PREAMBLE: &str = "#version 330 core\n#define attribute in\n#define varying out\n";

const FRAG_PREAMBLE: &str = "#version 330 core\n#define varying in\n\
                             #define texture2D texture\nout vec4 FRAG_COLOR;\n";

/// ES 1.00 is the language these are written in, so the device adds nothing but the name of
/// the output. A `#version` line is omitted rather than set: 100 is the default, and the
/// drivers that reject `#version 100` outnumber the ones that require it.
const VERT_PREAMBLE_ES: &str = "";
const FRAG_PREAMBLE_ES: &str = "#define FRAG_COLOR gl_FragColor\n";

pub fn program(vert: &str, frag: &str) -> Result<gl::types::GLuint, GfxError> {
    let (vp, fp) = match crate::gl::es() {
        true => (VERT_PREAMBLE_ES, FRAG_PREAMBLE_ES),
        false => (VERT_PREAMBLE, FRAG_PREAMBLE),
    };
    crate::gl::program(&format!("{vp}{vert}"), &format!("{fp}{frag}"))
}

/// Unit quad to a rect in target pixels, origin top left. The y flip lives here, so every
/// pass drawing into the offscreen target thinks in screen coordinates and only the blit
/// deals with the framebuffer being stored bottom up.
pub const RECT_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 p = (u_rect.xy + a_pos * u_rect.zw) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

/// `RECT_VERT` for sprites, turned about the rect's centre by `u_turn`, which holds the cosine
/// and sine of the angle. The corner is placed exactly as `RECT_VERT` places it, plus the
/// difference between the corner turned and unturned; the sprite loop passes exactly (1, 0)
/// for anything that is not turned, which makes that difference exactly zero. A shader of its
/// own rather than a change to `RECT_VERT`, because the game pass links that one too and would
/// read an unset `u_turn` as (0, 0).
pub const SPRITE_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_pos * u_rect.zw - mid;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    vec2 p = (u_rect.xy + a_pos * u_rect.zw + (turned - local)) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

/// `u_src` is the source size in pixels, which is also the number of times the 3x3 mask
/// tiles across the target: one RGB triad per source pixel, exactly.
pub const GAME_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_game;
uniform sampler2D u_mask;
uniform vec2 u_src;
uniform float u_bright;
uniform mat3 u_cc;
// 色彩校正的黑点：在矩阵之后加上去（同一空间）。饱和度/单色背光那一类用不上它（恒 0），
// 但**实机色板用得上**——DMG 最亮档不是白、最暗档也不是黑（#B8F878 到 #102000），
// 只靠一个 3x3 乘只能做「黑 → 某色」的斜率，做不出这种两点映射。加了它，
// 一个色板就是「白点 - 黑点」的斜率加一个偏移，四个灰阶能落在实机那四个颜色上。
uniform vec3 u_cc_bias;
// 实机色板的查找表：256×1 的 RGBA，按画面本来的灰度直接取色（见 app::DisplayFilter::gb_lut）。
uniform sampler2D u_pal;
// 1.0 = 走色板，0.0 = 走上面的矩阵。uniform 分支，两条路各自编译好，代价可忽略。
uniform float u_pal_on;
// 格子：颜色（sRGB 0..1，取色板最浅那一档）与混入量（0 = 不画）。
// 画在每个源像素 3x3 的**右列与下行**——重叠的右下角只算一次，5 个像素——所以一个游戏像素
// 保留左上 2x2 的画面。用 `fract(v_uv * u_src)` 取子位置，几个 ALU 加一次 mix 就够，
// 比再取一张遮罩纹理便宜，而且格子色**自动跟着色板走**。
uniform vec3 u_grid;
uniform float u_grid_mix;
// 色彩校正所在的 gamma：1.0 = 直接在编码空间乘（旧行为，两步 pow 互为逆）；
// 2.2 = 先转线性、乘完再转回。饱和度与单色背光映射必须在线性空间里做才不发闷——
// 这是 RetroArch 手持着色器（nds-color / lcd1x_nds）的通行做法，直接乘编码值会把
// 「半彩」压暗、把单色背光冲淡。
uniform float u_cc_gamma;
varying vec2 v_uv;
void main() {
    vec3 c = texture2D(u_game, v_uv).rgb;
    if (u_pal_on > 0.5) {
        // 画面是四阶灰，色板按灰阶取色。表在 CPU 上就按四档插好了（含中间过渡，反射那边
        // 拿到的是模糊过的连续值），这里一次 NEAREST 取样 —— 比两次 pow + 九次乘加便宜，
        // 而且四档落点精确，多色相的色板也保得住。
        float l = dot(c, vec3(0.299, 0.587, 0.114));
        c = texture2D(u_pal, vec2(l, 0.5)).rgb;
    } else {
        c = pow(c, vec3(u_cc_gamma));
        c = u_cc * c + u_cc_bias;
        c = pow(max(c, vec3(0.0)), vec3(1.0 / u_cc_gamma));
    }
    if (u_grid_mix > 0.0) {
        vec2 sub = fract(v_uv * u_src);
        if (sub.x >= 0.66667 || sub.y >= 0.66667) {
            c = mix(c, u_grid, u_grid_mix);
        }
    }
    // 面板遮罩照旧在编码空间乘（顺序与旧版等价：矩阵与遮罩都是乘法，可交换）。
    vec3 rgb = c * texture2D(u_mask, v_uv * u_src).rgb;
    FRAG_COLOR = vec4(rgb * u_bright, 1.0);
}
"#;

/// A 3x3 box average of the game frame, rendered into a much smaller texture (a quarter of
/// the frame each way). That downscale is the blur: the next pass samples the small texture
/// with linear filtering, and the wide magnification is what makes it genuinely soft — a
/// handful of taps against the pixel-art source would only band it.
/// Written out flat rather than as a loop. A `for` in a GLSL ES 1.00 fragment shader is the
/// one construct a strict mobile driver is most likely to refuse — and this tree had none
/// before, so there was nothing to say the device takes them. Nine taps is not worth finding
/// out on a machine with no console.
pub const BLUR_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec2 u_step;
varying vec2 v_uv;
void main() {
    vec3 s =
        texture2D(u_tex, v_uv + vec2(-1.0, -1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 0.0, -1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 1.0, -1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2(-1.0,  0.0) * u_step).rgb +
        texture2D(u_tex, v_uv).rgb +
        texture2D(u_tex, v_uv + vec2( 1.0,  0.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2(-1.0,  1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 0.0,  1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 1.0,  1.0) * u_step).rgb;
    FRAG_COLOR = vec4(s / 9.0, 1.0);
}
"#;

/// The screen glow: the blurred frame's own edge extended straight out past the picture's
/// window, drawn over the backdrop and under the game. Not a mirror — see `m` below.
///
/// `u_win` is the window rect in offscreen pixels; `u_margin` is how far, in pixels, the
/// reflection reaches beyond the window — full at its edge, gone that far out. The zone the
/// reflection can ever appear in is therefore the window grown by `u_margin` on every side,
/// which is the whole of what an overlay has to leave clear for it. `u_mask` is the overlay's
/// own texture, and its alpha is the **zone**: it is a hole or it is a wall — see `hole` below
/// for why it is not also the strength.
///
/// Drawn at the list's `Draw::Glow` marker, which is *after* the overlay art, so the art is
/// what the light falls on rather than a filter the light passes through.
pub const REFLECT_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_blur;
uniform sampler2D u_mask;
uniform vec4 u_win;
// Deliberately NOT `u_target`: that name is already a uniform of `RECT_VERT`, and a uniform
// shared between the two stages must carry the same precision in both. The vertex stage
// defaults floats to highp, this stage's `precision mediump float` makes it mediump, and the
// link fails outright — which is exactly how the first build of this went dark on the device.
// A name only the fragment knows has no counterpart to disagree with.
uniform vec2 u_panel;
uniform float u_margin;
uniform float u_gain;
uniform mat3 u_cc;
uniform vec3 u_cc_bias;
uniform sampler2D u_pal;
uniform float u_pal_on;
uniform float u_cc_gamma;
varying vec2 v_uv;
void main() {
    vec2 panel = v_uv * u_panel;
    vec2 t = (panel - u_win.xy) / u_win.zw;
    // The picture's own edge, carried straight out — light spilling past the screen onto the
    // bezel keeps going, it does not turn around. Clamping samples the nearest edge texel, so
    // the colour outside the window is the colour at the window's edge, smeared outward, and
    // the blur in the source texture softens it into a glow. (It was a mirror about each edge
    // once: a mirror needs a mirrored surface, and the plastic around a screen is not one.)
    vec2 m = clamp(t, 0.0, 1.0);
    // `u_blur` was rendered through `RECT_VERT`, which flips y on the way into a texture: its
    // v=1 row holds the picture's top, the other way round from the overlay and game textures,
    // which are uploaded and sampled as they are. So the y has to be put back here. Left
    // unflipped, the band above the screen shows what is *below* it — which reads as a mirror
    // and goes on reading as one however `m` is changed.
    vec3 c = texture2D(u_blur, vec2(m.x, 1.0 - m.y)).rgb;
    // The same colour correction the game pass applies — so the reflection carries the filter
    // (DMG green, ice-blue, amber, pink, grayscale, half colour) instead of mirroring the raw
    // picture, which is what switching a filter would otherwise leave disagreeing with the
    // screen. Keep this block in step with GAME_FRAG's own.
    if (u_pal_on > 0.5) {
        c = texture2D(u_pal, vec2(dot(c, vec3(0.299, 0.587, 0.114)), 0.5)).rgb;
    } else {
        c = pow(c, vec3(u_cc_gamma));
        c = u_cc * c + u_cc_bias;
        c = pow(max(c, vec3(0.0)), vec3(1.0 / u_cc_gamma));
    }
    // How far outside the window, in offscreen pixels — so the band is the same width on all
    // four sides whatever the letterbox happens to be.
    vec2 d = max(vec2(0.0), max(-t, t - 1.0)) * u_win.zw;
    float fade = clamp(1.0 - max(d.x, d.y) / u_margin, 0.0, 1.0);
    fade *= fade;
    // Strictly outside the window. `fade` is 1 *inside* it as well — d is zero there — and the
    // glow only ever got away with that by being drawn under the picture, which covered it.
    // Now that the glow is over the art it would be over the picture too, and the picture would
    // be veiled by a blurred copy of itself. The window is the screen; the light lands beside it.
    float outside = (t.x < 0.0 || t.y < 0.0 || t.x > 1.0 || t.y > 1.0) ? 1.0 : 0.0;
    // A hole or a wall, never a strength. The alpha used to scale the light as well, and the
    // art is drawn over this pass on top of that, so a bezel painted at 40% took 40% out of the
    // glow twice — one number in a PNG doing two unrelated jobs. What the light is worth is
    // `u_gain`, decided here, and how dark the bezel is stays the art's own business.
    float hole = texture2D(u_mask, v_uv).a < 0.5 ? 1.0 : 0.0;
    FRAG_COLOR = vec4(c, fade * outside * hole * u_gain);
}
"#;

pub const SPRITE_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec4 u_colour;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = texture2D(u_tex, v_uv) * u_colour;
}
"#;

pub const BLIT_VERT: &str = r#"
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const BLIT_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec3 u_gain;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = vec4(texture2D(u_tex, v_uv).rgb * u_gain, 1.0);
}
"#;
