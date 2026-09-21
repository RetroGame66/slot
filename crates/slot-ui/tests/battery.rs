use slot_power::{Battery, Charge};
use slot_ui::{capsule_left, draw_gauge, Draw, Printed, TexId, GAUGE_H, GAUGE_W, HINT_H, WALL};

/// The cluster's outer tip — the end of the nub — which is what the gauge is anchored by now
/// that it stands at the right of the top band with the clock at the left of it.
const RIGHT: f32 = 400.0;
/// The top of the *cluster*, which is the capsule's own top: the number hangs under it, so the
/// two are one object measured from its head rather than a row of three things.
const TOP: f32 = 400.0;

fn percent_face() -> Printed {
    Printed { face: None, w: 30 }
}

/// The number's own quad: the `Rect` or `Tex` under the capsule, which is the only thing in the
/// cluster whose `y` is below the capsule's floor.
fn number_quad(out: &[Draw]) -> Option<(f32, f32, f32, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Rect { x, y, w, h, .. } | Draw::Tex { x, y, w, h, .. }
            if y >= TOP + GAUGE_H - 0.01 =>
        {
            Some((x, y, w, h))
        }
        _ => None,
    })
}

/// The capsule's own left wall, from the layout rather than from the draw list. Nothing else
/// in this file can find it by position: the number's placeholder is a `Rect` and so are the
/// capsule's four strokes and the fill.
fn wall() -> f32 {
    capsule_left(RIGHT)
}

fn quads(out: &[Draw]) -> Vec<(f32, f32, f32, f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, h, .. } | Draw::Tex { x, y, w, h, .. } => Some((x, y, w, h)),
            _ => None,
        })
        .collect()
}

fn at(percent: u8, charge: Charge) -> Option<Battery> {
    Some(Battery { percent, charge })
}

/// The reason the bolt has a slot of its own instead of living inside the capsule: a leading
/// bolt on the band would either break the margin or hold a permanent gap for the times it is
/// absent. Nothing may move when a cable goes in.
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
    let idle = quads(&idle);
    for q in idle.iter() {
        assert!(
            quads(&charging).contains(q),
            "{q:?} moved or vanished when charging started"
        );
    }
}

/// The cluster reads from the right, and the number reads *under* it.
///
/// Three properties in one test because they are one decision. The nub ends at the anchor the
/// band hangs the cluster by. The number is below the capsule rather than beside it, which is
/// what buys the cluster back the width a three-character value used to spend on the band. And
/// it is centred on the capsule's own width rather than on the capsule plus its nub, because the
/// nub is a decoration on the positive end and counting it would put the number a couple of
/// pixels off the body it names.
#[test]
fn the_number_is_under_the_capsule_and_the_nub_ends_at_the_anchor() {
    let mut out = Vec::new();
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Discharging),
        percent_face(),
        None,
        &mut out,
    );
    let rightmost = quads(&out)
        .iter()
        .map(|q| q.0 + q.2)
        .fold(f32::MIN, f32::max);
    assert!(
        (rightmost - RIGHT).abs() < 0.01,
        "the nub does not end at the anchor: {rightmost}"
    );

    let (x, y, w, h) = number_quad(&out).expect("the number was not drawn under the capsule");
    assert!(
        y >= TOP + GAUGE_H,
        "the number is not below the capsule: {y} against {}",
        TOP + GAUGE_H
    );
    assert_eq!(h, HINT_H as f32, "the number is not on a band of type");
    let centre = x + w / 2.0;
    let body = wall() + GAUGE_W / 2.0;
    assert!(
        (centre - body).abs() < 0.01,
        "the number is not centred on the capsule: {centre} against {body}"
    );
    // And nothing else is down there with it: the capsule's own strokes all stop at its floor.
    let strays = quads(&out)
        .into_iter()
        .filter(|q| q.1 >= TOP + GAUGE_H - 0.01)
        .count();
    assert_eq!(
        strays, 1,
        "something besides the number is under the capsule"
    );
}

/// The defect this whole file is guarding against was the bolt drawn *inside* the capsule,
/// over the fill, knocking a hole in whatever charge was showing. Nothing else stops that
/// from happening again except this: the bolt's own quad must end at or before the
/// capsule's leftmost wall begins.
#[test]
fn the_bolt_never_reaches_the_capsule() {
    let mut out = Vec::new();
    draw_gauge(
        RIGHT,
        TOP,
        at(68, Charge::Charging),
        percent_face(),
        Some(TexId::from_raw(7)),
        &mut out,
    );
    let bolt_right = out
        .iter()
        .find_map(|d| match *d {
            Draw::Tex { x, w, tex, .. } if tex == TexId::from_raw(7) => Some(x + w),
            _ => None,
        })
        .expect("the bolt did not draw while charging");
    let capsule_left = wall();
    assert!(
        bolt_right <= capsule_left,
        "the bolt's right edge ({bolt_right}) reaches past the capsule's left wall ({capsule_left})"
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
    // The degraded case on hardware where `status` reads empty: a plain capsule, exactly
    // what the screen would show if none of this had been added.
    assert!(!bolt(Charge::Unknown));
}

/// The fill is the one quad in the cluster whose size is meant to move with the percent, and
/// the name's two clauses are two separate properties: the fill has to grow strictly as the
/// percent does (or a constant full bar would pass), and it may never cross the capsule's own
/// inner wall (or a fill formula that outruns 100% above the midpoint would pass).
#[test]
fn the_fill_tracks_the_percent_and_never_leaves_the_capsule() {
    let mut widths = Vec::new();
    for percent in [0u8, 1, 50, 99, 100] {
        let mut out = Vec::new();
        draw_gauge(
            RIGHT,
            TOP,
            at(percent, Charge::Discharging),
            percent_face(),
            None,
            &mut out,
        );
        let capsule_left = wall();
        let inner_right = capsule_left + GAUGE_W - 2.0 * WALL;
        // The fill is the only `Rect` that starts two wall-widths in from the capsule's own
        // left edge; every other stroke starts either at the wall itself, at the nub, or at the
        // number past the bolt's slot.
        let fill = out.iter().find_map(|d| match *d {
            Draw::Rect { x, w, .. } if (x - (capsule_left + 2.0 * WALL)).abs() < 0.01 => Some(w),
            _ => None,
        });
        if let Some(w) = fill {
            assert!(
                capsule_left + 2.0 * WALL + w <= inner_right + 0.01,
                "a {percent}% fill burst through the capsule's own wall"
            );
        }
        // A 0% battery draws no fill rect at all, which is the correct degenerate case of
        // "the fill tracks the percent": there is nothing to track down to.
        widths.push(fill.unwrap_or(0.0));
    }
    for pair in widths.windows(2) {
        assert!(
            pair[1] > pair[0],
            "the fill must grow strictly with the percent, got {widths:?}"
        );
    }
}

/// No gauge is no capsule. A device slot has not been ported to yet still has to come up.
#[test]
fn no_reading_draws_nothing() {
    let mut out = Vec::new();
    draw_gauge(RIGHT, TOP, None, percent_face(), None, &mut out);
    assert!(out.is_empty());
}
