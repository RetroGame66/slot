use std::sync::OnceLock;

use slot_store::System;

const CART_GBA_SVG: &str = include_str!("../assets/cart.svg");
const CART_GB_SVG: &str = include_str!("../assets/cart_gb.svg");
const CART_GBC_SVG: &str = include_str!("../assets/cart_gbc.svg");
const DETAIL_GBA_SVG: &str = include_str!("../assets/cart_detail.svg");
/// Shared by class A, B and C, which is why there is one of it rather than two.
const DETAIL_GB_SVG: &str = include_str!("../assets/cart_gb_detail.svg");

/// Which of the three shells a machine's carts are: the Advance cart's landscape outline, and
/// the Game Boy family's portrait one — carrying the top notch for class A and B (the grey and
/// black carts) and not for class C (Game Boy Color only), which is the physical lockout.
fn outline(system: System) -> &'static str {
    match system {
        System::Gba => CART_GBA_SVG,
        System::Gb => CART_GB_SVG,
        System::Gbc => CART_GBC_SVG,
    }
}

fn detail(system: System) -> &'static str {
    match system {
        System::Gba => DETAIL_GBA_SVG,
        System::Gb | System::Gbc => DETAIL_GB_SVG,
    }
}

fn index(system: System) -> usize {
    match system {
        System::Gba => 0,
        System::Gb => 1,
        System::Gbc => 2,
    }
}

/// Everything one machine's shell needs: shaped, depth-mapped, detailed. Rasterised once, at
/// the first face that asks for it, at that machine's own face size.
struct Masks {
    mask: Vec<u8>,
    depth: Vec<u8>,
    detail: Vec<u8>,
}

static MASKS: [OnceLock<Masks>; 3] = [const { OnceLock::new() }; 3];

fn masks(system: System, w: u32, h: u32) -> &'static Masks {
    MASKS[index(system)].get_or_init(|| {
        let mask =
            rasterise_svg(outline(system), w, h).unwrap_or_else(|| vec![255; (w * h) as usize]);
        let depth = depth_map(&mask, w as usize, h as usize);
        let detail =
            rasterise_svg(detail(system), w, h).unwrap_or_else(|| vec![0; (w * h) as usize]);
        Masks { mask, depth, detail }
    })
}

/// Coverage of the cart outline, one byte per pixel, row major.
pub fn silhouette(system: System, w: u32, h: u32) -> &'static [u8] {
    &masks(system, w, h).mask
}

/// Every cart of a machine is the same shape, so the mask is rasterised once per machine and
/// multiplied into its faces. Rasterised at the face resolution so the cart stays crisp at the
/// largest size the shelf draws it.
pub(crate) fn cart_mask(system: System, w: u32, h: u32) -> &'static [u8] {
    silhouette(system, w, h)
}

/// How far inside the outline each pixel sits, in city block steps, saturating at 255. A
/// translucent shell fades from its edge inward and needs the distance, not the coverage.
pub(crate) fn cart_depth(system: System, w: u32, h: u32) -> &'static [u8] {
    &masks(system, w, h).depth
}

/// The moulded detail: the grip ridge above the label and the thumb notch below it. Shaded into
/// the shell rather than drawn in a fixed colour, so it belongs to whatever colour the cart is.
pub(crate) fn detail_mask(system: System, w: u32, h: u32) -> &'static [u8] {
    &masks(system, w, h).detail
}

/// Two pass chamfer. Everything off the edge of the buffer counts as outside, so a pixel on the
/// top row is one step in rather than unreachable.
fn depth_map(mask: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut d: Vec<u8> = mask
        .iter()
        .map(|c| if *c > 127 { 255 } else { 0 })
        .collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if d[i] == 0 {
                continue;
            }
            let up = if y == 0 { 0 } else { d[i - w] };
            let left = if x == 0 { 0 } else { d[i - 1] };
            d[i] = d[i].min(up.saturating_add(1)).min(left.saturating_add(1));
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            if d[i] == 0 {
                continue;
            }
            let down = if y + 1 == h { 0 } else { d[i + w] };
            let right = if x + 1 == w { 0 } else { d[i + 1] };
            d[i] = d[i]
                .min(down.saturating_add(1))
                .min(right.saturating_add(1));
        }
    }
    d
}

fn rasterise_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    Some(pixmap.data().iter().skip(3).step_by(4).copied().collect())
}
