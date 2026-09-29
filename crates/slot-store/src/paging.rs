//! Walking a flat table one page at a time, with a caret that never leaves its page.
//!
//! The rules here are the ones a paged picker is made of, and they are pure arithmetic rather
//! than anything to do with what is being picked: `total` entries shown `per_page` at a time,
//! a page number, and a cell number inside it. They live in this crate rather than beside the
//! screen that draws them for one reason — they run on the host, so the off-by-one that puts
//! the caret on the wrong entry at a page boundary can be caught by a test instead of by eye.
//!
//! `page` and `cursor` are **view** coordinates: neither is an index into the table. The
//! translation is `index_of`, and every other function here keeps the pair consistent rather
//! than letting a caller do the arithmetic twice and get it wrong once.

/// How many entries `total` fills, `per_page` at a time. At least one, so an empty table still
/// has a page to stand on and callers never have to special-case it.
pub fn pages(total: usize, per_page: usize) -> usize {
    let per_page = per_page.max(1);
    total.div_ceil(per_page).max(1)
}

/// How many entries the given page actually holds. The last page is the short one; every other
/// page is full, and a page past the end holds nothing at all.
pub fn len_on(page: usize, total: usize, per_page: usize) -> usize {
    let per_page = per_page.max(1);
    let base = page.saturating_mul(per_page);
    total.saturating_sub(base).min(per_page)
}

/// Which entry a page and a caret name. Out-of-range pairs clamp rather than wrap, because
/// wrapping is what `step_cursor` is for and a silent wrap here would move the caret without
/// the user having asked.
pub fn index_of(page: usize, cursor: usize, total: usize, per_page: usize) -> usize {
    let base = page.saturating_mul(per_page.max(1));
    let n = len_on(page, total, per_page);
    if n == 0 {
        return base.min(total.saturating_sub(1));
    }
    base + cursor.min(n - 1)
}

/// The caret after a step of `delta`, staying on its own page and wrapping at the ends of it.
///
/// Wrapping rather than clamping is what makes the caret a ring: the page is the thing that
/// bounds it — `step_page` is how you leave — so holding a direction never strands you against
/// an edge you cannot see the other side of.
pub fn step_cursor(page: usize, cursor: usize, delta: isize, total: usize, per_page: usize) -> usize {
    let n = len_on(page, total, per_page);
    if n == 0 {
        return 0;
    }
    let c = (cursor.min(n - 1) as isize + delta).rem_euclid(n as isize);
    c as usize
}

/// The page after a step of `delta`, clamped to the ends of the table. Clamped rather than
/// wrapped because a page has a first and a last and the pips say so; a ring of pages would
/// make the ends of a long list hard to reach on purpose.
pub fn step_page(page: usize, delta: isize, total: usize, per_page: usize) -> usize {
    let last = pages(total, per_page) - 1;
    ((page as isize + delta).clamp(0, last as isize)) as usize
}

/// Which entry a *tap* of the palette key lands on: the next one on `page`, wrapping at the end
/// of that page rather than running into the next one.
///
/// The length of that walk is the page, not the table — so the key stays predictable however
/// many palettes the card's list grows to, and paging is the only way to reach a different set.
/// `at` outside the page (a palette chosen before the page moved) starts the walk at the page's
/// first entry rather than jumping to wherever the arithmetic happened to land.
pub fn cycle_on_page(page: usize, at: usize, total: usize, per_page: usize) -> usize {
    let base = page.saturating_mul(per_page.max(1));
    let n = len_on(page, total, per_page);
    if n == 0 {
        return base.min(total.saturating_sub(1));
    }
    let off = at
        .checked_sub(base)
        .filter(|o| *o < n)
        .map_or(0, |o| (o + 1) % n);
    base + off
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table this was written for: 347 palettes, fifteen to a page.
    const TOTAL: usize = 347;
    const PER: usize = 15;

    #[test]
    fn page_count_is_a_ceiling_and_never_zero() {
        assert_eq!(pages(345, PER), 23);
        assert_eq!(pages(347, PER), 24);
        assert_eq!(pages(15, PER), 1);
        assert_eq!(pages(0, PER), 1);
        // A page of one is still a page: the alternative is a divisor of zero somewhere.
        assert_eq!(pages(3, 0), 3);
    }

    #[test]
    fn the_last_page_is_the_short_one() {
        assert_eq!(len_on(0, TOTAL, PER), 15);
        assert_eq!(len_on(22, TOTAL, PER), 15);
        assert_eq!(len_on(23, TOTAL, PER), 2);
        assert_eq!(len_on(24, TOTAL, PER), 0);
    }

    #[test]
    fn a_page_and_a_caret_name_an_entry() {
        assert_eq!(index_of(0, 0, TOTAL, PER), 0);
        assert_eq!(index_of(0, 14, TOTAL, PER), 14);
        assert_eq!(index_of(1, 0, TOTAL, PER), 15);
        assert_eq!(index_of(23, 0, TOTAL, PER), 345);
        assert_eq!(index_of(23, 1, TOTAL, PER), 346);
        // A caret past the end of the short page clamps instead of running off the table.
        assert_eq!(index_of(23, 14, TOTAL, PER), 346);
    }

    #[test]
    fn the_caret_rings_inside_its_own_page() {
        assert_eq!(step_cursor(0, 0, -1, TOTAL, PER), 14);
        assert_eq!(step_cursor(0, 14, 1, TOTAL, PER), 0);
        assert_eq!(step_cursor(0, 13, 3, TOTAL, PER), 1);
        // Down a row is three, and the wrap is still the page's own end.
        assert_eq!(step_cursor(23, 1, 3, TOTAL, PER), 0);
        assert_eq!(step_cursor(23, 0, -1, TOTAL, PER), 1);
    }

    #[test]
    fn the_page_stops_at_its_ends() {
        assert_eq!(step_page(0, -1, TOTAL, PER), 0);
        assert_eq!(step_page(0, 1, TOTAL, PER), 1);
        assert_eq!(step_page(23, 1, TOTAL, PER), 23);
        assert_eq!(step_page(23, -1, TOTAL, PER), 22);
    }

    #[test]
    fn a_tap_walks_its_page_and_wraps_within_it() {
        assert_eq!(cycle_on_page(0, 0, TOTAL, PER), 1);
        assert_eq!(cycle_on_page(0, 13, TOTAL, PER), 14);
        // The end of the page wraps to its start, not into the next page.
        assert_eq!(cycle_on_page(0, 14, TOTAL, PER), 0);
        assert_eq!(cycle_on_page(1, 15, TOTAL, PER), 16);
        // And the short last page wraps inside its own two.
        assert_eq!(cycle_on_page(23, 345, TOTAL, PER), 346);
        assert_eq!(cycle_on_page(23, 346, TOTAL, PER), 345);
    }

    #[test]
    fn a_tap_from_outside_the_page_starts_at_its_first_entry() {
        // A palette picked before the page moved is not on this page at all: the walk must not
        // resume from an offset that means something else here.
        assert_eq!(cycle_on_page(1, 0, TOTAL, PER), 15);
        assert_eq!(cycle_on_page(1, 200, TOTAL, PER), 15);
        // Both ends of "outside": a palette below the page and one above it. The last page's
        // first entry is 345, so that is where the walk resumes either way.
        assert_eq!(cycle_on_page(23, 0, TOTAL, PER), 345);
        assert_eq!(cycle_on_page(23, 100, TOTAL, PER), 345);
    }
}
