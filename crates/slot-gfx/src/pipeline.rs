use crate::lcd3x::mask_texture_rgba8;
use crate::power::{screen_brightness, screen_rect};
use crate::quad::Quad;
use crate::shaders::{GAME_FRAG, RECT_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

/// The only scale that exists. 240x160 to 720x480, nearest, which is what collapses LCD3x
/// to a 3x3 mask tiled once per source pixel.
pub const SCALE: u32 = 3;
pub const SRC_W: u32 = OUT_W / SCALE;
pub const SRC_H: u32 = OUT_H / SCALE;

/// Column-major identity for the colour-correction matrix, uploaded when correction is off.
const IDENTITY_CC: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub struct GamePass {
    prog: gl::types::GLuint,
    game: gl::types::GLuint,
    mask: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_cc: gl::types::GLint,
    u_cc_gamma: gl::types::GLint,
    /// A compositor with nobody driving it is a screen that is on.
    power: f32,
}

impl GamePass {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, GAME_FRAG)?;
        let game = crate::gl::texture(SRC_W, SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let mask = crate::gl::texture(
            3,
            3,
            gl::NEAREST,
            gl::REPEAT,
            gl::RGBA,
            Some(&mask_texture_rgba8()),
        );
        let (u_rect, u_bright, u_cc, u_cc_gamma);
        unsafe {
            // The other two are fixed for the life of the program: the mask always tiles once
            // per source pixel and the target is always the offscreen frame.
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_game"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_mask"), 1);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_src"),
                SRC_W as f32,
                SRC_H as f32,
            );
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_bright = crate::gl::uniform_location(prog, "u_bright");
            u_cc = crate::gl::uniform_location(prog, "u_cc");
            u_cc_gamma = crate::gl::uniform_location(prog, "u_cc_gamma");
            // 1.0 = 编码空间直乘（旧行为）。app 每帧推真正的值。
            gl::Uniform1f(u_cc_gamma, 1.0);
            // Identity until a colour correction is chosen; the compositor uploads the real one.
            gl::UniformMatrix3fv(u_cc, 1, gl::FALSE, IDENTITY_CC.as_ptr());
        }
        Ok(GamePass {
            prog,
            game,
            mask,
            u_rect,
            u_bright,
            u_cc,
            u_cc_gamma,
            power: 1.0,
        })
    }

    pub fn set_power(&mut self, t: f32) {
        self.power = t.clamp(0.0, 1.0);
    }

    /// Replace the table the picture is multiplied by. This is the whole of what this device
    /// has for a shader: LCD3x collapses to a 3x3 mask because 240x160 lands exactly three
    /// times in 720x480, and it is sampled once per source pixel. Handing the card its own
    /// table is what lets the panel look like something other than the one table that ships.
    ///
    /// Three by three and nothing else. The mask tiles once per source pixel, so a table of
    /// another shape would be sampled at three points out of its own grid and alias into
    /// noise instead of resolving into a pattern.
    pub fn set_mask(&self, rgba: &[u8]) {
        if rgba.len() < (3 * 3 * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.mask);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                3,
                3,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    /// Replace the colour-correction matrix. Row-major in, uploaded column-major to match
    /// GLSL's `mat3 * vec3`. Identity leaves the picture as the mask alone would leave it.
    pub fn set_color_correction(&self, m: &[[f32; 3]; 3]) {
        let col = [
            m[0][0], m[1][0], m[2][0], m[0][1], m[1][1], m[2][1], m[0][2], m[1][2], m[2][2],
        ];
        unsafe {
            gl::UseProgram(self.prog);
            gl::UniformMatrix3fv(self.u_cc, 1, gl::FALSE, col.as_ptr());
        }
    }

    /// 色彩校正所用的 gamma。1.0 = 编码空间直乘（旧行为）；>1 = 在线性空间做校正
    /// （半彩不再发暗、单色背光更浓郁）。与矩阵一起由 `Fbo::set_cc_gamma` 推送。
    pub fn set_cc_gamma(&self, g: f32) {
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform1f(self.u_cc_gamma, g.max(0.01));
        }
    }

    pub fn upload(&mut self, xrgb8888: &[u8]) {
        if xrgb8888.len() < (SRC_W * SRC_H * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.game);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                SRC_W as i32,
                SRC_H as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                xrgb8888.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    pub fn draw(&self, quad: &Quad) {
        self.draw_source(self.game, quad);
    }

    /// The same pass over a still. A saved shot is a picture of this panel at exactly the
    /// scale the mask is built for, so it is filtered at draw time rather than blitted flat
    /// beside a game that is filtered.
    pub fn draw_still(&self, tex: gl::types::GLuint, quad: &Quad) {
        self.draw_source(tex, quad);
    }

    fn draw_source(&self, tex: gl::types::GLuint, quad: &Quad) {
        let (x, y, w, h) = screen_rect(self.power);
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform4f(self.u_rect, x, y, w, h);
            gl::Uniform1f(self.u_bright, screen_brightness(self.power));
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.mask);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        quad.draw();
    }
}

impl Drop for GamePass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.game);
            gl::DeleteTextures(1, &self.mask);
            gl::DeleteProgram(self.prog);
        }
    }
}
