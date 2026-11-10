//! Preserve upstream's removed-row fixture alongside Enhanced's retained row.
use super::*;
thread_local! { static UPSTREAM_ROWS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(super) struct UpstreamRows;
pub(super) fn upstream_rows() -> UpstreamRows {
    UPSTREAM_ROWS.with(|upstream| upstream.set(true));
    UpstreamRows
}
impl Drop for UpstreamRows {
    fn drop(&mut self) {
        UPSTREAM_ROWS.with(|upstream| upstream.set(false));
    }
}
pub(super) fn visible(id: &str) -> bool {
    id != "stats_box" || !UPSTREAM_ROWS.with(|upstream| upstream.get())
}
#[test]
fn retained_rows_and_map_keyboard_and_button_bindings_round_trip() {
    let b = Binds::from_text(
        "key.map=y\nbutton.map=r3\nkey.stats_box=j\nkey.unknown_row=x\nkey.attack=u\n",
    );
    assert_eq!(b.key(row_named("map").unwrap()), Some("y"));
    assert_eq!(b.key(row_named("stats_box").unwrap()), Some("j"));
    assert_eq!(b.key(row_named("attack").unwrap()), Some("u"));
    let text = b.to_text();
    for kept in ["key.map=y\n", "button.map=r3\n", "key.stats_box=j\n"] {
        assert!(text.contains(kept), "{text}");
    }
    assert!(!text.contains("unknown_row"));
    assert_eq!(Binds::from_text(&text), b);
}
