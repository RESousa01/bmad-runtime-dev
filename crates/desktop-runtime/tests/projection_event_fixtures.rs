//! Rust side of the shared renderer↔host projection-event golden fixtures.
//!
//! The same JSON document is parsed by the renderer test
//! `apps/desktop-ui/src/lib/hostClient/projectionEventFixtures.test.ts`; a
//! serialization change on either side must break one of the two suites.

use desktop_runtime::ProjectionEvent;

const FIXTURES: &str = include_str!("../../../tests/ipc-fixtures/projection-events.json");

#[test]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "malformed shared fixtures must fail the test immediately"
)]
fn every_projection_event_fixture_round_trips_exactly() {
    let fixtures: Vec<serde_json::Value> =
        serde_json::from_str(FIXTURES).expect("fixture file must be valid JSON");
    assert_eq!(
        fixtures.len(),
        10,
        "fixture set must cover every ProjectionEventKind variant"
    );
    let mut seen_types = Vec::new();
    for fixture in fixtures {
        let event: ProjectionEvent =
            serde_json::from_value(fixture.clone()).expect("fixture must deserialize");
        let reserialized = serde_json::to_value(&event).expect("event must serialize");
        assert_eq!(
            reserialized, fixture,
            "serialization must match the shared fixture exactly"
        );
        let event_type = fixture["event"]["type"]
            .as_str()
            .expect("fixture event must carry a type tag")
            .to_owned();
        assert!(
            !seen_types.contains(&event_type),
            "duplicate fixture for {event_type}"
        );
        seen_types.push(event_type);
    }
}
