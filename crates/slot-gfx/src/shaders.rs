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
// 色彩校正所在的 gamma：1.0 = 直接在编码空间乘（旧行为，两步 pow 互为逆）；
// 2.2 = 先转线性、乘完再转回。饱和度与单色背光映射必须在线性空间里做才不发闷——
// 这是 RetroArch 手持着色器（nds-color / lcd1x_nds）的通行做法，直接乘编码值会把
// 「半彩」压暗、把单色背光冲淡。
uniform float u_cc_gamma;
varying vec2 v_uv;
void main() {
    vec3 c = texture2D(u_game, v_uv).rgb;
    c = pow(c, vec3(u_cc_gamma));
    c = u_cc * c;
    c = pow(max(c, vec3(0.0)), vec3(1.0 / u_cc_gamma));
    // 面板遮罩照旧在编码空间乘（顺序与旧版等价：矩阵与遮罩都是乘法，可交换）。
    vec3 rgb = c * texture2D(u_mask, v_uv * u_src).rgb;
    FRAG_COLOR = vec4(rgb * u_bright, 1.0);
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
