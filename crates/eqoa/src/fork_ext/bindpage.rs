//! Keep upstream's free-F2 macro fixture independent of Enhanced map bindings.
use crate::binds::{Binds, row_named};

pub(crate) fn free_fixture_key(b: &mut Binds) {
    b.bind_key(
        row_named("map_search").expect("Enhanced map search row"),
        "f24",
    );
}

#[test]
fn fixture_frees_f2_while_production_keeps_map_search_on_it() {
    let row = row_named("map_search").unwrap();
    let mut fixture = Binds::default();
    assert_eq!(fixture.key(row), Some("f2"), "production default");
    free_fixture_key(&mut fixture);
    assert_eq!(fixture.key(row), Some("f24"));
    assert_eq!(
        Binds::default().key(row),
        Some("f2"),
        "only this fixture moved"
    );
}
