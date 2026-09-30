use crate::lcd3x::mask_texture_rgba8;
use crate::power::{screen_brightness, screen_rect_in};
use crate::quad::Quad;
use crate::shaders::{GAME_FRAG, RECT_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};
use crate::system::{System, GBA_SRC_H, GBA_SRC_W, GB_SRC_H, GB_SRC_W};

/// The GBA's frame, which is what these two meant when there was one machine. Anything that
/// wants a source size should ask `System` for the one it is drawing.
pub const SRC_W: u32 = GBA_SRC_W;
pub const SRC_H: u32 = GBA_SRC_H;

/// Column-major identity for the colour-correction matrix, uploaded when correction is off.
const IDENTITY_CC: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub struct GamePass {
    prog: gl::types::GLuint,
    /// One texture per machine rather than one rebuilt on a switch, so that switching costs a
    /// bind and nothing at all else. The two together are 240 KiB — the GBA's frame is
    /// 153,600 bytes and Game Boy's 92,160 — which is not memory worth saving on anything
    /// this runs on.
    game_gba: gl::types::GLuint,
    game_gb: gl::types::GLuint,
    /// Whose frame `upload` takes and `draw` puts up. The GBA, because it is the machine this
    /// pass was written for.
    active: System,
    mask: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_cc: gl::types::GLint,
    u_cc_bias: gl::types::GLint,
    u_cc_gamma: gl::types::GLint,
    /// The Game Boy palette lookup: 256x1 RGBA, one texel per grey the core can hand over.
    /// A texture rather than a table in the shader because the four shades are arbitrary
    /// colours, and a 256-entry upload is a kilobyte.
    pal: gl::types::GLuint,
    /// The pixel-art lookup: 1024x32 RGBA — the 32^3 colour cube flattened (x = r + 32*g,
    /// y = b), each texel the palette colour nearest that cell. Only the pixel grade reads
    /// it; a texture for the same reason as `pal`.
    pix: gl::types::GLuint,
    /// Whether the lookup above is what draws the picture, or the matrix is.
    ///
    /// The *sampler's* location is not kept: `u_pal` is bound to texture unit 2 once in `new` and
    /// never again, so the location is asked for there and dropped. It used to be a field, holding
    /// the location of `u_pal_on` by mistake — harmless only because nothing ever read it.
    u_pal_on: gl::types::GLint,
    /// Whether the pixel-art lookup draws the picture (`u_pix` in GAME_FRAG).
    u_pix_on: gl::types::GLint,
    /// The lattice drawn over every 3x3 source pixel: its colour, how much of it to mix in, and
    /// whether it is the whole 5-of-9 mesh or the bottom row alone (a scanline). A mix of `0.0` is
    /// off, which is the default; every machine draws its mesh from here now.
    u_grid: gl::types::GLint,
    u_grid_mix: gl::types::GLint,
    u_grid_scan: gl::types::GLint,
    /// A compositor with nobody driving it is a screen that is on.
    power: f32,
}

impl GamePass {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, GAME_FRAG)?;
        let game_gba = crate::gl::texture(
            GBA_SRC_W,
            GBA_SRC_H,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::BGRA,
            None,
        );
        let game_gb = crate::gl::texture(
            GB_SRC_W,
            GB_SRC_H,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::BGRA,
            None,
        );
        let mask = crate::gl::texture(
            3,
            3,
            gl::NEAREST,
            gl::REPEAT,
            gl::RGBA,
            Some(&mask_texture_rgba8()),
        );
        let pal = crate::gl::texture(
            256,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&[0u8; 1024]),
        );
        let pix = crate::gl::texture(1024, 32, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None);
        let (
            u_rect,
            u_bright,
            u_cc,
            u_cc_bias,
            u_cc_gamma,
            u_pal_on,
            u_pix_on,
            u_grid,
            u_grid_mix,
            u_grid_scan,
        );
        unsafe {
            // What is fixed for the life of the program: which texture unit carries what, and
            // the offscreen target every rect is placed against. Not the source size — that
            // one belongs to the machine whose picture is up, see `set_system`.
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_game"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_mask"), 1);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_pal"), 2);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_pix"), 3);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_bright = crate::gl::uniform_location(prog, "u_bright");
            u_cc = crate::gl::uniform_location(prog, "u_cc");
            u_cc_bias = crate::gl::uniform_location(prog, "u_cc_bias");
            u_cc_gamma = crate::gl::uniform_location(prog, "u_cc_gamma");
            // 1.0 = 编码空间直乘（旧行为）。app 每帧推真正的值。
            gl::Uniform1f(u_cc_gamma, 1.0);
            // Identity until a colour correction is chosen; the compositor uploads the real one.
            gl::UniformMatrix3fv(u_cc, 1, gl::FALSE, IDENTITY_CC.as_ptr());
            // No black point by default: the saturation and backlight modes are anchored on
            // black, and only a hardware palette moves it (see `u_cc_bias` in GAME_FRAG).
            gl::Uniform3f(u_cc_bias, 0.0, 0.0, 0.0);
            u_pal_on = crate::gl::uniform_location(prog, "u_pal_on");
            u_pix_on = crate::gl::uniform_location(prog, "u_pix_on");
            gl::Uniform1f(u_pix_on, 0.0);
            u_grid = crate::gl::uniform_location(prog, "u_grid");
            u_grid_mix = crate::gl::uniform_location(prog, "u_grid_mix");
            gl::Uniform1f(u_grid_mix, 0.0);
            u_grid_scan = crate::gl::uniform_location(prog, "u_grid_scan");
            gl::Uniform1f(u_grid_scan, 0.0);
            // Off until a palette arrives; the matrix path is what runs by default.
            gl::Uniform1f(u_pal_on, 0.0);
            // The source size, primed for the machine `active` starts on. `set_system` is a no-op
            // when the machine has not changed, so a pass that only ever sees an Advance — which
            // starts as one — would otherwise never upload this at all. Both the aperture mask and
            // the lattice tile by it, and with it left at zero the lattice's `fract(v_uv * u_src)`
            // is zero everywhere: no cell ever matches, and the mesh silently never draws.
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_src"),
                System::Gba.src_w() as f32,
                System::Gba.src_h() as f32,
            );
        }
        let mut pass = GamePass {
            prog,
            game_gba,
            game_gb,
            active: System::Gba,
            mask,
            u_rect,
            u_bright,
            u_cc,
            u_cc_bias,
            u_cc_gamma,
            pal,
            pix,
            u_pal_on,
            u_pix_on,
            u_grid,
            u_grid_mix,
            u_grid_scan,
            power: 1.0,
        };
        pass.set_system(System::Gba);
        Ok(pass)
    }

    /// Whose frame this pass is drawing. Called when a cart goes in or the shelf switches
    /// machines, which is to say rarely — nothing here is per frame.
    pub fn set_system(&mut self, system: System) {
        if self.active == system {
            return;
        }
        self.active = system;
        // The mask tiles once per source pixel, so a different frame size is a different
        // tiling count and nothing more: the 3x3 table is the same table, the filter is the
        // same filter, and no part of either is written against a resolution.
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform2f(
                crate::gl::uniform_location(self.prog, "u_src"),
                system.src_w() as f32,
                system.src_h() as f32,
            );
        }
    }

    /// The texture `upload` writes into and `draw` puts up, for the machine that is active.
    pub fn active_texture(&self) -> gl::types::GLuint {
        match self.active {
            System::Gba => self.game_gba,
            System::Gb => self.game_gb,
        }
    }

    /// The rect, in offscreen pixels, the game picture currently occupies — what the screen
    /// reflection mirrors about, and how it knows where the letterbox is.
    pub fn source_rect(&self) -> (f32, f32, f32, f32) {
        screen_rect_in(self.active.screen_base(), self.power)
    }

    /// The size of the frame the active machine hands over, in pixels. The reflection's blur
    /// step is one over this, so a tap lands on a texel of whatever machine is up.
    pub fn source_size(&self) -> (u32, u32) {
        (self.active.src_w(), self.active.src_h())
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

    /// Replace the colour-correction matrix and its black point. Row-major in, uploaded
    /// column-major to match GLSL's `mat3 * vec3`. Identity with no black point leaves the
    /// picture as the mask alone would leave it; a hardware palette is a slope from its white
    /// point plus an offset to its black one, which is the only way to land four shades on the
    /// four colours the cartridge was made for.
    pub fn set_color_correction(&self, m: &[[f32; 3]; 3], bias: &[f32; 3]) {
        let col = [
            m[0][0], m[1][0], m[2][0], m[0][1], m[1][1], m[2][1], m[0][2], m[1][2], m[2][2],
        ];
        unsafe {
            gl::UseProgram(self.prog);
            gl::UniformMatrix3fv(self.u_cc, 1, gl::FALSE, col.as_ptr());
            gl::Uniform3f(self.u_cc_bias, bias[0], bias[1], bias[2]);
        }
    }

    /// Hand over the Game Boy's palette, or take it away. `None` restores the matrix path.
    ///
    /// The table is built on the CPU (`app::DisplayFilter::gb_lut`) with the four shades already
    /// interpolated between, so the shader is one nearest-neighbour fetch and lands each shade
    /// exactly on its own colour — which a 3x3 multiply cannot do for a palette whose middles
    /// sit off the line between its ends.
    pub fn set_palette(&self, lut: Option<&[u8; 1024]>) {
        unsafe {
            gl::UseProgram(self.prog);
            match lut {
                Some(l) => {
                    gl::BindTexture(gl::TEXTURE_2D, self.pal);
                    gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
                    gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        0,
                        0,
                        256,
                        1,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        l.as_ptr() as *const std::ffi::c_void,
                    );
                    gl::Uniform1f(self.u_pal_on, 1.0);
                }
                None => gl::Uniform1f(self.u_pal_on, 0.0),
            }
        }
    }

    /// Hand over the pixel-art lookup, or take it away. `None` restores the matrix path.
    ///
    /// `lut` is 1024x32 RGBA (131072 bytes): the 32^3 colour cube flattened, each texel the
    /// palette colour nearest that cell, built on the CPU from the pixel palette
    /// (`app::DisplayFilter::pixel_lut`). One NEAREST fetch per pixel; the 5-bit snap and the
    /// 4x4 dither that go with it live in GAME_FRAG.
    pub fn set_pixel_lut(&self, lut: Option<&[u8]>) {
        unsafe {
            gl::UseProgram(self.prog);
            match lut {
                Some(l) => {
                    gl::BindTexture(gl::TEXTURE_2D, self.pix);
                    gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
                    gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        0,
                        0,
                        1024,
                        32,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        l.as_ptr() as *const std::ffi::c_void,
                    );
                    gl::Uniform1f(self.u_pix_on, 1.0);
                }
                None => gl::Uniform1f(self.u_pix_on, 0.0),
            }
        }
    }

    /// Draw the panel lattice — or stop drawing it, with a mix of zero.
    ///
    /// `colour` is the shade the lines are mixed towards and `mix` is how much of it shows.
    /// `scanline` keeps the bottom row of every 3x3 source pixel and drops the right column: the
    /// same lit wire every third game pixel rather than a mesh. The geometry lives in the shader
    /// either way.
    pub fn set_grid(&self, colour: &[f32; 3], mix: f32, scanline: bool) {
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform3f(self.u_grid, colour[0], colour[1], colour[2]);
            gl::Uniform1f(self.u_grid_mix, mix.max(0.0));
            gl::Uniform1f(self.u_grid_scan, if scanline { 1.0 } else { 0.0 });
        }
    }

    /// The lookup texture, so the reflection pass can share it rather than keep a second copy.
    pub fn palette_texture(&self) -> gl::types::GLuint {
        self.pal
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
        let (w, h) = (self.active.src_w(), self.active.src_h());
        if xrgb8888.len() < (w * h * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.active_texture());
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                w as i32,
                h as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                xrgb8888.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    pub fn draw(&self, quad: &Quad) {
        self.draw_source(self.active_texture(), quad);
    }

    /// The same pass over a still. A saved shot is a picture of this panel at exactly the
    /// scale the mask is built for, so it is filtered at draw time rather than blitted flat
    /// beside a game that is filtered.
    pub fn draw_still(&self, tex: gl::types::GLuint, quad: &Quad) {
        self.draw_source(tex, quad);
    }

    fn draw_source(&self, tex: gl::types::GLuint, quad: &Quad) {
        // The strike and bloom is worked against the picture's own window, not the frame: a
        // Game Boy coming up inside the panel opens inside the window its picture occupies.
        let (x, y, w, h) = screen_rect_in(self.active.screen_base(), self.power);
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform4f(self.u_rect, x, y, w, h);
            gl::Uniform1f(self.u_bright, screen_brightness(self.power));
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.mask);
            gl::ActiveTexture(gl::TEXTURE2);
            gl::BindTexture(gl::TEXTURE_2D, self.pal);
            gl::ActiveTexture(gl::TEXTURE3);
            gl::BindTexture(gl::TEXTURE_2D, self.pix);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        quad.draw();
    }
}

impl Drop for GamePass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.game_gba);
            gl::DeleteTextures(1, &self.game_gb);
            gl::DeleteTextures(1, &self.mask);
            gl::DeleteTextures(1, &self.pal);
            gl::DeleteTextures(1, &self.pix);
            gl::DeleteProgram(self.prog);
        }
    }
}
