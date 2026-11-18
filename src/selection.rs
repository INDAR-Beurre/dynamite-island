//! selection — shared list-selection math for the keyboard-navigable
//! list views (Launcher, Clipboard), iter39 item a10.
//!
//! PURE LOGIC ONLY: no Wayland, no fonts, no I/O, no allocation, no
//! blocking calls. Both list panels keep the SAME two fields with the
//! SAME semantics and call these helpers for every mutation, so Up/Down,
//! Enter, filtering and reset behave identically in both views:
//!
//! * `selected` — index into the FULL filtered/match list (0 = first
//!   match). NEVER a visible-slot index.
//! * `offset` — index of the first VISIBLE row (the window is
//!   `offset .. offset + page`, `page` = visible row count: 6 in the
//!   Launcher, 8 in the Clipboard).
//! * Invariants (restored by every helper): `selected < count` (or
//!   `selected == 0` when `count == 0`); `offset <= max_offset`;
//!   the selected row is always inside the visible window.
//!
//! The panels own the fields; main.rs owns the key dispatch and calls
//! `live_move_selection` / `live_selected_exec` / `live_reset` on the
//! panels (Launcher) and `clip_move_selection` / `clip_select_async` /
//! `clip_reset` (Clipboard). Nothing here knows about IslandState, the
//! socket, or the compositor — that is what makes it unit-testable
//! under `cargo test --offline` on the Surface (this VM has no Rust
//! toolchain; see the builder report).

/// Fresh list: first row selected, window at the top.
pub const fn reset() -> (usize, usize) {
    (0, 0)
}

/// Largest legal window offset for `count` items shown `page` at a time.
pub fn max_offset(count: usize, page: usize) -> usize {
    if page == 0 {
        return 0;
    }
    count.saturating_sub(page)
}

/// Clamp a (selected, offset) pair to a list of `count` items:
/// selection into range, offset into range, then pull the window so
/// the selected row is visible. Empty list → (0, 0).
pub fn clamp(selected: usize, offset: usize, count: usize, page: usize) -> (usize, usize) {
    if count == 0 || page == 0 {
        return (0, 0);
    }
    let sel = selected.min(count - 1);
    let max_off = max_offset(count, page);
    let mut off = offset.min(max_off);
    if sel < off {
        off = sel;
    } else if sel >= off + page {
        off = sel + 1 - page;
    }
    (sel, off.min(max_off))
}

/// Move the selection `dir` rows (`-1` = up, `+1` = down), clamped at
/// both ends (no wrap — the list is not a cycle; the MENU cycle wraps,
/// list selection does not). The window follows the selection.
pub fn move_by(
    selected: usize,
    offset: usize,
    dir: i32,
    count: usize,
    page: usize,
) -> (usize, usize) {
    if count == 0 || page == 0 {
        return (0, 0);
    }
    let last = (count - 1) as i32;
    let sel = (selected as i32 + dir).clamp(0, last) as usize;
    clamp(sel, offset, count, page)
}

/// Visible index range for the current window: [start, end).
pub fn visible_range(offset: usize, count: usize, page: usize) -> (usize, usize) {
    let start = offset.min(max_offset(count, page));
    (start, (start + page).min(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_is_top_row() {
        assert_eq!(reset(), (0, 0));
    }

    #[test]
    fn move_down_within_page_keeps_offset() {
        assert_eq!(move_by(0, 0, 1, 37, 6), (1, 0));
        assert_eq!(move_by(4, 0, 1, 37, 6), (5, 0));
    }

    #[test]
    fn move_down_past_page_scrolls_window() {
        // Selected row 6 is the first row outside the 0..6 window.
        assert_eq!(move_by(5, 0, 1, 37, 6), (6, 1));
        assert_eq!(move_by(6, 1, 1, 37, 6), (7, 2));
        // Settings is index 6 of 37: reachable, and the window follows.
        assert_eq!(move_by(5, 0, 1, 37, 6).0, 6);
    }

    #[test]
    fn move_up_pulls_window_back() {
        // Minimal-window rule (see clamp): the window moves ONLY
        // when the selection would leave it. Row 6 is still inside
        // the 2..8 window, and row 5 inside 1..7, so the offset
        // stays; the window pulls back only when the selection
        // exits its top edge (third + fourth lines).
        assert_eq!(move_by(7, 2, -1, 37, 6), (6, 2));
        assert_eq!(move_by(6, 1, -1, 37, 6), (5, 1));
        assert_eq!(move_by(1, 1, -1, 37, 6), (0, 0));
        assert_eq!(move_by(2, 2, -1, 37, 6), (1, 1));
    }

    #[test]
    fn clamps_at_both_ends_no_wrap() {
        assert_eq!(move_by(0, 0, -1, 37, 6), (0, 0));
        assert_eq!(move_by(36, 31, 1, 37, 6), (36, 31));
        assert_eq!(move_by(36, 31, 5, 37, 6), (36, 31));
        assert_eq!(move_by(0, 0, -5, 37, 6), (0, 0));
    }

    #[test]
    fn empty_list_is_zeroed() {
        assert_eq!(move_by(3, 2, 1, 0, 6), (0, 0));
        assert_eq!(clamp(3, 2, 0, 6), (0, 0));
    }

    #[test]
    fn filtering_shrink_clamps_selection_to_last_row() {
        // 37 apps → query leaves 2 matches; a stale selected=6 clamps.
        assert_eq!(clamp(6, 1, 2, 6), (1, 0));
        assert_eq!(clamp(0, 5, 2, 6), (0, 0));
    }

    #[test]
    fn short_list_never_scrolls() {
        assert_eq!(move_by(5, 0, 1, 6, 6), (5, 0));
        assert_eq!(max_offset(6, 6), 0);
        assert_eq!(max_offset(5, 6), 0);
        assert_eq!(visible_range(0, 4, 6), (0, 4));
    }

    #[test]
    fn visible_range_follows_offset() {
        assert_eq!(visible_range(1, 37, 6), (1, 7));
        assert_eq!(visible_range(31, 37, 6), (31, 37));
        assert_eq!(visible_range(99, 37, 6), (31, 37));
    }

    #[test]
    fn page_eight_matches_the_clipboard_window() {
        // Clipboard shows 8 rows: moving down past row 7 scrolls.
        assert_eq!(move_by(7, 0, 1, 20, 8), (8, 1));
        assert_eq!(move_by(19, 12, 1, 20, 8), (19, 12));
        assert_eq!(move_by(0, 0, -1, 20, 8), (0, 0));
    }
}
