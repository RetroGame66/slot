use slot_power::{Battery, Charge};
use slot_ui::{cluster_h, draw_gauge, Draw, Printed, TexId, BOLT_H, BOLT_W, STATUS_H};

/// The band's own right margin, which is what the reading is anchored by: the percent's right
/// edge lands here and the type grows leftward from it.
const RIGHT: f32 = 400.0;
/// The top of the status band. There is no capsule above the number any more, so the cluster is
/// one band of type and its top *is* the band's top.
const TOP: f32 = 100.0;

fn percent_face() -> Printed {
    Printed { face: None, w: 30 }
}

fn quads(out: &[Draw]) -> Vec<(f32, f32, f32, f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, h, .. } | Draw::Tex { x, y, w, h, .. } => Some((x, y, w, h)),
            _ => None,
        })
        .collect()
}

/// The number's own quad: everything the gauge draws is the number and the bolt, and the number
/// is the one that is not the bolt's texture.
fn number_quad(out: &[Draw]) -> Option<(f32, f32, f32, f32)> {
    quads(out).into_iter().find(|q| q.1 == TOP)
}

fn at(percent: u8, charge: Charge) -> Option<Battery> {
    Some(Battery { percent, charge })
}

/// The row's height is decided in one place and both ends of the band read it. If this ever
/// stops holding, the clock and the battery are set in boxes of two different heights and one
/// end of the band is quietly squashed by `Draw::Tex` scaling it into a shorter quad.
#[test]
fn the_cluster_is_the_same_band_the_clock_is_set_in() {
    assert_eq!(cluster_h(), STATUS_H as f32);
}

/// The reading hangs off the right margin with its own right edge, so a percent that gains a
/// digit grows leftward and the margin never moves.
#[test]
fn the_number_ends_exactly_at_the_anchor() {
    let mut out = Vec::new();
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Discharging),
        percent_face(),
        None,
        &mut out,
    );
    let (x, y, w, h) = number_quad(&out).expect("the percent was not drawn");
    assert!(
        ((x + w) - RIGHT).abs() < 0.01,
        "the number's right edge is not on the anchor: {} against {RIGHT}",
        x + w
    );
    assert_eq!(y, TOP, "the number is not on the band's own line");
    assert_eq!(w, 30.0, "the number is not the width it was handed");
    assert_eq!(h, STATUS_H as f32, "the number is not on the status band");
}

/// Nothing about the number may move when a cable goes in. The bolt is drawn in the space the
/// number leaves rather than from a reserved slot, so the test is that the number's quad is
/// *identical* — not merely still present.
#[test]
fn nothing_moves_when_the_charge_state_changes() {
    let mut idle = Vec::new();
    let mut charging = Vec::new();
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Discharging),
        percent_face(),
        None,
        &mut idle,
    );
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Charging),
        percent_face(),
        Some(TexId::from_raw(7)),
        &mut charging,
    );
    assert_eq!(
        number_quad(&idle),
        number_quad(&charging),
        "the number moved when charging started"
    );
    assert!(
        quads(&charging).len() > quads(&idle).len(),
        "the bolt did not draw at all"
    );
}

/// The bolt sits in the gap the number leaves, to its left, and never reaches into it: a bolt
/// over the type would knock a hole in the reading it belongs to.
#[test]
fn the_bolt_sits_left_of_the_number_and_never_reaches_it() {
    let mut out = Vec::new();
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Charging),
        percent_face(),
        Some(TexId::from_raw(7)),
        &mut out,
    );
    let (nx, _, _, _) = number_quad(&out).expect("the percent was not drawn");
    let (bx, by, bw, bh) = out
        .iter()
        .find_map(|d| match *d {
            Draw::Tex {
                x, y, w, h, tex, ..
            } if tex == TexId::from_raw(7) => Some((x, y, w, h)),
            _ => None,
        })
        .expect("the bolt did not draw while charging");
    assert!(
        bx + bw <= nx,
        "the bolt's right edge ({}) reaches the number ({nx})",
        bx + bw
    );
    assert_eq!((bw, bh), (BOLT_W, BOLT_H), "the bolt is not its own size");
    let band = STATUS_H as f32;
    assert!(
        (by - (TOP + (band - bh) / 2.0)).abs() < 0.01,
        "the bolt is not centred on the band: {by}"
    );
}

#[test]
fn the_bolt_is_only_drawn_while_charging() {
    let bolt = |charge| {
        let mut out = Vec::new();
        draw_gauge(
            RIGHT,
            TOP,
            at(68, charge),
            percent_face(),
            Some(TexId::from_raw(7)),
            &mut out,
        );
        out.iter()
            .any(|d| matches!(d, Draw::Tex { tex: t, .. } if *t == TexId::from_raw(7)))
    };
    assert!(bolt(Charge::Charging));
    assert!(!bolt(Charge::Discharging));
    assert!(!bolt(Charge::Full));
    // The degraded case on hardware where `status` reads empty.
    assert!(!bolt(Charge::Unknown));
}

/// With no capsule there is no fixed width to place a bolt against, so a bolt drawn before the
/// percent's face lands would sit out at the margin and then jump left the moment type arrived.
/// Nothing is drawn at all until the width is known.
#[test]
fn nothing_is_drawn_before_the_face_lands() {
    let mut out = Vec::new();
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Charging),
        Printed::default(),
        Some(TexId::from_raw(7)),
        &mut out,
    );
    assert!(
        out.is_empty(),
        "something drew against a zero width: {out:?}"
    );
}

/// No gauge is no reading. A device slot has not been ported to yet still has to come up.
#[test]
fn no_reading_draws_nothing() {
    let mut out = Vec::new();
    draw_gauge(RIGHT, TOP, None, percent_face(), None, &mut out);
    assert!(out.is_empty());
}

/// A 100% battery is four characters where 9% is two, and both must land on the same anchor.
#[test]
fn a_longer_reading_still_ends_at_the_anchor() {
    for w in [12u32, 30, 44] {
        let mut out = Vec::new();
        draw_gauge(
            RIGHT,
            TOP,
            at(100, Charge::Discharging),
            Printed { face: None, w },
            None,
            &mut out,
        );
        let (x, _, drawn, _) = number_quad(&out).expect("the percent was not drawn");
        assert!(
            (x + drawn - RIGHT).abs() < 0.01,
            "width {w} did not end on the anchor"
        );
    }
}
