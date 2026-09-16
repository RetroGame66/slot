use slot_input::{Action, Btn};
use slot_retro::ButtonMask;

/// One past the last `Btn` discriminant. The physical-button state is held as a fixed array
/// indexed by `Btn as usize`, so this must match the number of variants in `slot_input::Btn`.
const BTN_COUNT: usize = 19;

/// Maps a physical button to the button the core should see in its place. Identity by default;
/// a card can redirect one button to another — `X = A` for a player who has grown used to
/// reaching for X where a GBA expects A. Held as a target `Btn` per physical index rather than
/// a single folded bit, so remapping two physical buttons onto the same game button still
/// releases each correctly: releasing one clears only that physical key's contribution, not the
/// bit the other (still held) key is also driving.
#[derive(Clone, Copy)]
pub struct Remap {
    map: [Btn; BTN_COUNT],
}

impl Remap {
    /// Every physical button reports as itself.
    pub fn identity() -> Self {
        Remap {
            map: [
                Btn::Up,
                Btn::Down,
                Btn::Left,
                Btn::Right,
                Btn::A,
                Btn::B,
                Btn::X,
                Btn::Y,
                Btn::L1,
                Btn::R1,
                Btn::L2,
                Btn::R2,
                Btn::Start,
                Btn::Select,
                Btn::Menu,
                Btn::VolUp,
                Btn::VolDown,
                Btn::Power,
                Btn::Lid,
            ],
        }
    }

    /// Redirect `phys` so the core receives `target` in its place.
    pub fn set(&mut self, phys: Btn, target: Btn) {
        self.map[phys as usize] = target;
    }
}

impl Default for Remap {
    fn default() -> Self {
        Remap::identity()
    }
}

/// The buttons the gesture layer let through to the game, held as a libretro mask. It is
/// level state, not edges, because that is what the core is polled for every frame.
pub struct Pad {
    /// Per-physical-button down state. Recomputed into the mask each frame (see `mask`) so that
    /// two physical buttons mapped onto the same game button both release correctly.
    down: [bool; BTN_COUNT],
    remap: Remap,
}

impl Pad {
    pub fn new(remap: Remap) -> Self {
        Pad {
            down: [false; BTN_COUNT],
            remap,
        }
    }

    pub fn with_remap(remap: Remap) -> Self {
        Self::new(remap)
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::GbaDown(b) => self.down[b as usize] = true,
            Action::GbaUp(b) => self.down[b as usize] = false,
            _ => {}
        }
    }

    pub fn mask(&self) -> ButtonMask {
        let mut m = 0u16;
        for i in 0..BTN_COUNT {
            if self.down[i] {
                if let Some(bit) = bit(self.remap.map[i]) {
                    m |= bit;
                }
            }
        }
        ButtonMask(m)
    }

    /// Buttons pressed for the switcher are not the game's. Without this the game resumes
    /// holding whatever was down when the switcher took the input.
    pub fn clear(&mut self) {
        self.down = [false; BTN_COUNT];
    }
}

impl Default for Pad {
    fn default() -> Self {
        Pad::new(Remap::identity())
    }
}

fn bit(btn: Btn) -> Option<u16> {
    Some(match btn {
        Btn::Up => ButtonMask::UP,
        Btn::Down => ButtonMask::DOWN,
        Btn::Left => ButtonMask::LEFT,
        Btn::Right => ButtonMask::RIGHT,
        Btn::A => ButtonMask::A,
        Btn::B => ButtonMask::B,
        Btn::L1 => ButtonMask::L,
        Btn::R1 => ButtonMask::R,
        Btn::Start => ButtonMask::START,
        Btn::Select => ButtonMask::SELECT,
        _ => return None,
    })
}
