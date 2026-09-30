use crate::draw::{Draw, Sprites, TexId};
use crate::grade::blue_light_gain;
use crate::pipeline::GamePass;
use crate::quad::Quad;
use crate::shaders::{BLIT_FRAG, BLIT_VERT, BLUR_FRAG, RECT_VERT, REFLECT_FRAG};
use crate::surface::{blit_rect, GfxError, Surface, OUT_H, OUT_W};
use crate::system::{System, GB_SRC_H, GB_SRC_W};

/// Backdrop behind everything drawn into the offscreen target. Distinct from the black
/// letterbox so the blit rect is visible even with nothing else on screen.
/// Black. A grey backdrop was within 6/765 of the default cart shell, which made every
/// ordinary cart on the shelf invisible against it. Black also lets a coloured shell read as
/// plastic rather than as a tinted panel.
pub const BACKDROP: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The blur's target, as a divisor of the game frame. **One**: the source at its own size, with
/// only `BLUR_FRAG`'s 3×3 average to soften it. Anything coarser defeats the point — at half
/// (80×72) a 20px band of glow spans about three texels of a washed-out picture, which is why
/// mirroring the edge and carrying it straight out measured 1/255 apart and looked identical.
/// At the source size an extended edge reads as the edge, and a mirrored one plainly does not.
const REFLECT_DIV: u32 = 1;
/// How far, in offscreen pixels, the reflection reaches beyond the game window before it has
/// faded to nothing — the same on every side. It is deliberately **wider than any sane
/// overlay's clear band** (the ones on the card are 20px): the fade is squared, so a band that
/// ended where this margin ends would sit entirely in the tail and be nearly invisible. Out at
/// 64 the same 20px band carries two to three times the light. Anything past the overlay's own
/// opaque art is covered by it, so a wide margin costs nothing.
const REFLECT_MARGIN: f32 = 64.0;
/// How strong the glow is at the window's own edge, where the fade is at its fullest. **This is
/// the dial**, and it is the only one: the overlay's alpha no longer scales the light (it says
/// where the glow may appear and nothing else), and the overlay's art no longer sits on top of
/// it. So the brightness of the glow is one number, and the darkness of the bezel is another,
/// and moving either leaves the other where it was.
///
/// **One** is the ceiling — the band right against the screen would be the edge colour with the
/// backdrop showing through nowhere — so anything at or above it is the brightest this can be.
/// It was effectively 0.36 before the two corrections above (a 40%-opaque mask took 40% out of
/// the light as a zone and another 40% as art drawn over it), which is why the glow read as
/// barely there.
const REFLECT_GAIN: f32 = 0.75;
/// Column-major identity, so the reflection starts uncorrected until the app pushes a matrix —
/// the same starting point the game pass uses.
const IDENTITY_COL: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub struct Compositor {
    fbo: gl::types::GLuint,
    tex: gl::types::GLuint,
    blit: gl::types::GLuint,
    u_gain: gl::types::GLint,
    gain: [f32; 3],
    shake: f32,
    quad: Quad,
    game: GamePass,
    sprites: Sprites,
    /// Last mask pushed, so a per-frame `set_panel_mask` uploads only when it actually changes.
    applied_mask: Option<[[[u8; 3]; 3]; 3]>,
    /// Last colour-correction matrix pushed, for the same reason.
    applied_cc: Option<([[f32; 3]; 3], [f32; 3])>,
    /// Last colour-correction gamma pushed, for the same reason.
    applied_cc_gamma: Option<f32>,
    /// The overlay whose transparency marks where the screen reflection shows, or `None` when
    /// the overlay up is not reflective. Set every frame; read by `draw_reflection`.
    reflect_mask: Option<TexId>,
    reflect_prog: gl::types::GLuint,
    u_refl_blur: gl::types::GLint,
    u_refl_win: gl::types::GLint,
    u_refl_rect: gl::types::GLint,
    /// The colour correction, pushed to the reflection as well as the game pass so a filter
    /// tints both alike. See `set_color_correction`.
    u_refl_cc: gl::types::GLint,
    u_refl_cc_bias: gl::types::GLint,
    u_refl_cc_gamma: gl::types::GLint,
    /// The Game Boy palette lookup, shared with the game pass rather than copied: the
    /// reflection has to carry the same palette or the glow around the screen disagrees with
    /// the picture it is glowing from.
    ///
    /// Only the on/off location is kept: the sampler is bound to texture unit 2 once, beside the
    /// program link, and never again. The old field held the *wrong* location anyway — the same
    /// lookup as `u_refl_pal_on` — which is what the compiler was complaining about.
    u_refl_pal_on: gl::types::GLint,
    /// The small, blurred copy of the game frame the reflection samples, and the target it is
    /// rendered into. Fixed for the life of the compositor: only GB/GBC have a letterbox, and
    /// both hand over the same 160×144 frame.
    blur_prog: gl::types::GLuint,
    u_blur_tex: gl::types::GLint,
    u_blur_step: gl::types::GLint,
    u_blur_rect: gl::types::GLint,
    blur_fbo: gl::types::GLuint,
    blur_tex: gl::types::GLuint,
    /// Non-fatal startup notes — today, only why the reflection was left off. A device with no
    /// console has nowhere to print these, so `device_app` writes them to the card.
    warnings: Vec<String>,
}

impl Compositor {
    pub fn new(surface: &dyn Surface) -> Result<Self, GfxError> {
        crate::gl::load(surface);
        let blit = crate::shaders::program(BLIT_VERT, BLIT_FRAG)?;
        // Nearest and clamped: the blit is an integer multiply, never a resample.
        let tex = crate::gl::texture(OUT_W, OUT_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None);
        unsafe {
            let mut fbo = 0;
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex,
                0,
            );
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            if status != gl::FRAMEBUFFER_COMPLETE {
                gl::DeleteFramebuffers(1, &fbo);
                gl::DeleteTextures(1, &tex);
                gl::DeleteProgram(blit);
                return Err(GfxError::Framebuffer(status));
            }
            gl::UseProgram(blit);
            gl::Uniform1i(crate::gl::uniform_location(blit, "u_tex"), 0);
            let u_gain = crate::gl::uniform_location(blit, "u_gain");

            // The reflection's small blur target and the two programs that feed it. Built with
            // the rest of the fixed machinery; none of it is per frame except the two draws.
            let (bw, bh) = (GB_SRC_W / REFLECT_DIV, GB_SRC_H / REFLECT_DIV);
            // Linear: the upscale from this tiny texture is what makes the reflection smooth.
            let blur_tex = crate::gl::texture(bw, bh, gl::LINEAR, gl::CLAMP_TO_EDGE, gl::RGBA, None);
            let mut blur_fbo = 0;
            gl::GenFramebuffers(1, &mut blur_fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, blur_fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                blur_tex,
                0,
            );
            let blur_status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            // The reflection is an extra, and nothing here may take the compositor down with
            // it: `device_app::run` returns the instant `Compositor::new` fails, which is a
            // boot with no screen at all. So a blur target the driver will not give us, or a
            // program it will not compile, leaves the reflection off (the handles stay 0 and
            // `draw_reflection` skips) and the picture comes up exactly as it always did.
            let mut warnings: Vec<String> = Vec::new();
            if blur_status != gl::FRAMEBUFFER_COMPLETE {
                warnings.push(format!(
                    "reflection off: blur target incomplete ({blur_status:#x})"
                ));
                gl::DeleteFramebuffers(1, &blur_fbo);
                gl::DeleteTextures(1, &blur_tex);
                blur_fbo = 0;
            }
            let blur_prog = match crate::shaders::program(RECT_VERT, BLUR_FRAG) {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!("reflection off: blur program: {e}"));
                    0
                }
            };
            let reflect_prog = match crate::shaders::program(RECT_VERT, REFLECT_FRAG) {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!("reflection off: reflect program: {e}"));
                    0
                }
            };
            gl::UseProgram(blur_prog);
            gl::Uniform1i(crate::gl::uniform_location(blur_prog, "u_tex"), 0);
            gl::Uniform2f(
                crate::gl::uniform_location(blur_prog, "u_target"),
                bw as f32,
                bh as f32,
            );
            gl::UseProgram(reflect_prog);
            gl::Uniform1i(crate::gl::uniform_location(reflect_prog, "u_blur"), 0);
            gl::Uniform1i(crate::gl::uniform_location(reflect_prog, "u_mask"), 1);
            // `u_target` positions the quad (RECT_VERT); `u_panel` is the fragment's own copy of
            // the same size, under a name the vertex stage does not also declare. See
            // `REFLECT_FRAG` for why they cannot share one.
            gl::Uniform2f(
                crate::gl::uniform_location(reflect_prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            gl::Uniform2f(
                crate::gl::uniform_location(reflect_prog, "u_panel"),
                OUT_W as f32,
                OUT_H as f32,
            );
            gl::Uniform1f(
                crate::gl::uniform_location(reflect_prog, "u_margin"),
                REFLECT_MARGIN,
            );
            // Fixed for the life of the program: it is a property of the effect and not of the
            // cart or the card, so it is set here rather than pushed every frame.
            gl::Uniform1f(
                crate::gl::uniform_location(reflect_prog, "u_gain"),
                REFLECT_GAIN,
            );
            let u_refl_blur = crate::gl::uniform_location(reflect_prog, "u_blur");
            let u_refl_win = crate::gl::uniform_location(reflect_prog, "u_win");
            let u_refl_rect = crate::gl::uniform_location(reflect_prog, "u_rect");
            let u_refl_cc = crate::gl::uniform_location(reflect_prog, "u_cc");
            let u_refl_cc_bias = crate::gl::uniform_location(reflect_prog, "u_cc_bias");
            let u_refl_cc_gamma = crate::gl::uniform_location(reflect_prog, "u_cc_gamma");
            let u_refl_pal_on = crate::gl::uniform_location(reflect_prog, "u_pal_on");
            gl::Uniform1i(crate::gl::uniform_location(reflect_prog, "u_pal"), 2);
            gl::Uniform1f(u_refl_pal_on, 0.0);
            // Uncorrected until the app pushes a matrix, exactly as the game pass starts.
            gl::UniformMatrix3fv(u_refl_cc, 1, gl::FALSE, IDENTITY_COL.as_ptr());
            gl::Uniform3f(u_refl_cc_bias, 0.0, 0.0, 0.0);
            gl::Uniform1f(u_refl_cc_gamma, 1.0);
            let u_blur_tex = crate::gl::uniform_location(blur_prog, "u_tex");
            let u_blur_step = crate::gl::uniform_location(blur_prog, "u_step");
            let u_blur_rect = crate::gl::uniform_location(blur_prog, "u_rect");

            Ok(Compositor {
                fbo,
                tex,
                blit,
                u_gain,
                gain: blue_light_gain(0),
                shake: 0.0,
                quad: Quad::new(),
                game: GamePass::new()?,
                sprites: Sprites::new()?,
                applied_mask: None,
                applied_cc: None,
                applied_cc_gamma: None,
                reflect_mask: None,
                reflect_prog,
                u_refl_blur,
                u_refl_win,
                u_refl_rect,
                u_refl_cc,
                u_refl_cc_bias,
                u_refl_cc_gamma,
                u_refl_pal_on,
                blur_prog,
                u_blur_tex,
                u_blur_step,
                u_blur_rect,
                blur_fbo,
                blur_tex,
                warnings,
            })
        }
    }

    /// Whose picture the game layer is drawing, and so what frame size `upload_game` takes
    /// and where `draw_game` puts it. Forwarded to the pass, which owns both. Set when a cart
    /// goes in or the shelf changes machine — never per frame.
    pub fn set_system(&mut self, system: System) {
        self.game.set_system(system);
    }

    pub fn upload_game(&mut self, xrgb8888: &[u8]) {
        self.game.upload(xrgb8888);
    }

    /// The panel mask, as nine RGB triples in row-major order. See `GamePass::set_mask` for
    /// why it is a table rather than a shader, and why it is three by three.
    pub fn set_panel_mask(&mut self, rgb: &[[[u8; 3]; 3]; 3]) {
        if self.applied_mask.as_ref() == Some(rgb) {
            return;
        }
        let mut rgba = [255u8; 3 * 3 * 4];
        for (texel, cell) in rgba.chunks_exact_mut(4).zip(rgb.iter().flatten()) {
            texel[..3].copy_from_slice(cell);
        }
        self.game.set_mask(&rgba);
        self.applied_mask = Some(*rgb);
    }

    /// The colour-correction matrix for the game pass, row-major (output row, input column),
    /// and its black point. Identity with no black point when correction is off; a
    /// colour-saturation-style table when on; a hardware palette's two points when a Game Boy
    /// palette is chosen. Cached so cycling the preset uploads the new pair exactly once.
    pub fn set_color_correction(&mut self, m: &[[f32; 3]; 3], bias: &[f32; 3]) {
        if self.applied_cc.as_ref() == Some(&(*m, *bias)) {
            return;
        }
        self.game.set_color_correction(m, bias);
        // The reflection carries the same correction, so a filter tints the mirrored blur and
        // the picture alike. Same row-major in, column-major out, as the game pass.
        let col = [
            m[0][0], m[1][0], m[2][0], m[0][1], m[1][1], m[2][1], m[0][2], m[1][2], m[2][2],
        ];
        if self.reflect_prog != 0 {
            unsafe {
                gl::UseProgram(self.reflect_prog);
                gl::UniformMatrix3fv(self.u_refl_cc, 1, gl::FALSE, col.as_ptr());
                gl::Uniform3f(self.u_refl_cc_bias, bias[0], bias[1], bias[2]);
            }
        }
        self.applied_cc = Some((*m, *bias));
    }

    /// Hand the Game Boy's palette to both passes that apply colour, or take it back with
    /// `None`. Pushed every frame rather than cached: it is a kilobyte, and binding the unit
    /// here is what makes the reflection pick the same table up before it draws.
    pub fn set_palette(&mut self, lut: Option<&[u8; 1024]>) {
        self.game.set_palette(lut);
        unsafe {
            gl::ActiveTexture(gl::TEXTURE2);
            gl::BindTexture(gl::TEXTURE_2D, self.game.palette_texture());
            gl::ActiveTexture(gl::TEXTURE0);
            gl::UseProgram(self.reflect_prog);
            gl::Uniform1f(self.u_refl_pal_on, if lut.is_some() { 1.0 } else { 0.0 });
        }
    }

    /// Hand the pixel-art lookup to the game pass, or take it back with `None`. Forwarded to the
    /// game pass only: the glow round the screen is a blur, and quantising a blur would band it,
    /// so the reflection is deliberately left continuous (see `REFLECT_FRAG`).
    pub fn set_pixel_lut(&mut self, lut: Option<&[u8]>) {
        self.game.set_pixel_lut(lut);
    }

    /// Draw the panel lattice, or take it away with a mix of zero. Forwarded to the game pass
    /// only: the lattice belongs to the panel the picture is on, not to the light spilling out
    /// of it, so the reflection carries the palette but not the mesh.
    pub fn set_grid(&mut self, colour: &[f32; 3], mix: f32, scanline: bool) {
        self.game.set_grid(colour, mix, scanline);
    }

    /// 色彩校正的 gamma，随矩阵一起每帧推。1.0 = 编码空间直乘；2.2 = 线性空间做校正。
    pub fn set_cc_gamma(&mut self, g: f32) {
        if self.applied_cc_gamma == Some(g) {
            return;
        }
        self.game.set_cc_gamma(g);
        unsafe {
            gl::UseProgram(self.reflect_prog);
            gl::Uniform1f(self.u_refl_cc_gamma, g.max(0.01));
        }
        self.applied_cc_gamma = Some(g);
    }

    /// The overlay texture whose transparency marks where the screen reflection shows, or
    /// `None` when the overlay up is not reflective. Pushed every frame with the rest of the
    /// per-frame state; the effect itself is drawn inside `draw_list`, under the game.
    pub fn set_reflection(&mut self, mask: Option<TexId>) {
        self.reflect_mask = mask;
    }

    /// Non-fatal notes from `new` — today, only why the screen reflection was left off. Read
    /// once by `device_app`, which writes them to the card: the device has no console.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The screen glow, for a glowing overlay: the game frame blurred, its own edge carried
    /// straight out past the picture's window, faded with distance — and shown only where the
    /// overlay's own alpha leaves a hole. Two cheap draws: a 3×3 average of the frame into a
    /// half-size texture, then one full-panel quad that samples it. Nothing here reads a
    /// previous frame, so it can neither lag nor fall out of step with the picture.
    fn draw_reflection(&mut self) {
        // Built only if the device's GL took the programs and the blur target. A driver that
        // refused any of them leaves the reflection off; see `Compositor::new`.
        if self.reflect_prog == 0 || self.blur_prog == 0 || self.blur_fbo == 0 {
            return;
        }
        let Some(mask_id) = self.reflect_mask else {
            return;
        };
        let Some(mask_tex) = self.sprites.source(mask_id) else {
            return;
        };
        let game_tex = self.game.active_texture();
        let (wx, wy, ww, wh) = self.game.source_rect();
        if ww <= 1.0 || wh <= 1.0 {
            return;
        }
        let (sw, sh) = self.game.source_size();
        let (bw, bh) = (GB_SRC_W / REFLECT_DIV, GB_SRC_H / REFLECT_DIV);
        unsafe {
            // 1. The blur: a 3×3 box average of the frame into the small texture.
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.blur_fbo);
            gl::Viewport(0, 0, bw as i32, bh as i32);
            gl::Disable(gl::BLEND);
            gl::UseProgram(self.blur_prog);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, game_tex);
            gl::Uniform1i(self.u_blur_tex, 0);
            gl::Uniform2f(self.u_blur_step, 1.0 / sw as f32, 1.0 / sh as f32);
            gl::Uniform4f(self.u_blur_rect, 0.0, 0.0, bw as f32, bh as f32);
            self.quad.draw();

            // 2. The reflection, full panel, over the backdrop.
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::Viewport(0, 0, OUT_W as i32, OUT_H as i32);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.reflect_prog);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.blur_tex);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, mask_tex);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::Uniform1i(self.u_refl_blur, 0);
            gl::Uniform4f(self.u_refl_win, wx, wy, ww, wh);
            gl::Uniform4f(self.u_refl_rect, 0.0, 0.0, OUT_W as f32, OUT_H as f32);
            self.quad.draw();
            gl::Disable(gl::BLEND);
        }
    }

    pub fn draw_game(&mut self) {
        self.game.draw(&self.quad);
    }

    /// Sprites in order, with the game pass and the glow drawn wherever the list asks for
    /// them. The split is what lets the picture come up over a seated cart and stay under the
    /// HUD, and what puts the glow over the overlay art rather than under it.
    pub fn draw_list(&mut self, items: &[Draw]) {
        let mut from = 0;
        for (i, item) in items.iter().enumerate() {
            match *item {
                Draw::Game => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    self.game.draw(&self.quad);
                }
                // The glow goes **after** the art it glows past, not before. It used to be
                // drawn at the `Game` marker, which put it under the overlay: the art is drawn
                // over the glow, so a bezel painted at 40% cut the light down by another 40%
                // on top of the 40% the mask's own alpha had already taken out of it — one
                // number in a PNG dimming the same light twice, and neither the frame's
                // darkness nor the glow's strength tunable without moving the other. Over the
                // art is also simply where the light is: the art is the plastic's own colour,
                // and this is light falling on that plastic.
                Draw::Glow => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    self.draw_reflection();
                }
                Draw::Shot { tex } => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    // A shot naming a texture nobody made draws nothing, as a sprite does.
                    if let Some(tex) = self.sprites.source(tex) {
                        self.game.draw_still(tex, &self.quad);
                    }
                }
                _ => continue,
            }
            from = i + 1;
        }
        self.sprites.draw(&items[from..], &self.quad);
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.sprites.create_texture(w, h, rgba)
    }

    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.sprites.create_texture_nearest(w, h, rgba)
    }

    /// Frees one texture and leaves its slot empty, for a face the shelf has scrolled away
    /// from. See `SpritePool::release_texture` for why the pool is allowed to be full of holes.
    pub fn release_texture(&mut self, id: TexId) {
        self.sprites.release_texture(id);
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        self.sprites.update_texture(id, w, h, rgba);
    }

    pub fn set_blue_light(&mut self, step: u8) {
        self.gain = blue_light_gain(step);
    }

    /// 0.0 dark, 1.0 fully on. Scales and brightens the game layer, nothing else: the chrome
    /// stays where it is while the picture blooms out from behind it.
    pub fn set_screen_power(&mut self, t: f32) {
        self.game.set_power(t);
    }

    /// Pixels, in offscreen space, applied to the whole presented image. On the blit rather
    /// than on the draw list, so game, chrome and HUD move together as one picture. Applied
    /// inside the offscreen target it would shake the chrome against a game that stayed
    /// still. Edges reveal the letterbox for the duration, which is what a jolt looks like.
    pub fn set_shake(&mut self, dx: f32) {
        self.shake = dx;
    }

    /// The offscreen target, top row first. glReadPixels hands back the framebuffer in
    /// memory order, which is bottom up.
    pub fn read_frame(&self) -> Vec<u8> {
        let stride = OUT_W as usize * 4;
        let mut buf = vec![0u8; stride * OUT_H as usize];
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
            gl::ReadPixels(
                0,
                0,
                OUT_W as i32,
                OUT_H as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
            );
        }
        let mut top_down = Vec::with_capacity(buf.len());
        for row in buf.chunks_exact(stride).rev() {
            top_down.extend_from_slice(row);
        }
        top_down
    }

    pub fn begin_frame(&mut self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::Viewport(0, 0, OUT_W as i32, OUT_H as i32);
            gl::ClearColor(BACKDROP[0], BACKDROP[1], BACKDROP[2], BACKDROP[3]);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    pub fn end_frame(&mut self, window: (u32, u32)) {
        let (x, y, w, h) = blit_rect(window, self.shake);
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0, 0, window.0 as i32, window.1 as i32);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Viewport(x, y, w, h);
            gl::UseProgram(self.blit);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.tex);
            gl::Uniform3f(self.u_gain, self.gain[0], self.gain[1], self.gain[2]);
        }
        self.quad.draw();
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.fbo);
            gl::DeleteTextures(1, &self.tex);
            gl::DeleteProgram(self.blit);
            gl::DeleteFramebuffers(1, &self.blur_fbo);
            gl::DeleteTextures(1, &self.blur_tex);
            gl::DeleteProgram(self.blur_prog);
            gl::DeleteProgram(self.reflect_prog);
        }
    }
}
