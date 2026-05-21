//! Smoke test: force every compound LazyLock to initialize and confirm derivation
//! from the embedded Unity prefab assets succeeds. Since every constant in the
//! parent module is *derived* from the prefab YAML + sprites.bytes at first
//! access (no hand-extracted numbers remain), any per-field equality test would
//! be a tautology. Instead we exercise each LazyLock so missing prefabs,
//! malformed components, or unmapped material guids surface as a hard panic.

use super::*;

#[test]
fn all_compound_constants_initialize_without_panic() {
    // SubSprite LazyLocks — touch each so its derivation runs.
    let _ = (
        &*FAN_PROPELLER,
        &*FAN_ENGINE,
        &*FAN_FRAME,
        &*BRIDGE_STEP,
        &*FLOATING_BOX,
        &*FLOATING_BALLOON,
    );
    // Scalar LazyLocks.
    let _ = FLOATING_BALLOON.world_w
        + FLOATING_BALLOON.world_h;
    let _ = *FLOATING_STAR_BALLOON_DISTANCE + *FLOATING_PART_BALLOON_DISTANCE;
    let _ = FLOATING_STAR_ROPE_BALLOON_ANCHOR_LOCAL.0
        + FLOATING_STAR_ROPE_BALLOON_ANCHOR_LOCAL.1
        + FLOATING_PART_ROPE_BALLOON_ANCHOR_LOCAL.0
        + FLOATING_PART_ROPE_BALLOON_ANCHOR_LOCAL.1
        + FLOATING_STAR_ROPE_BOX_ANCHOR_LOCAL.0
        + FLOATING_STAR_ROPE_BOX_ANCHOR_LOCAL.1
        + FLOATING_PART_ROPE_BOX_ANCHOR_LOCAL.0
        + FLOATING_PART_ROPE_BOX_ANCHOR_LOCAL.1;
    // Slice LazyLocks — assert expected variant counts.
    assert_eq!(BIRD_FACES.len(), 4);
    assert!(BIRD_FACES.iter().all(|face| face.atlas == "IngameAtlas.png"));
    assert_eq!(FLOATING_BALLOON.atlas, "IngameAtlas.png");
}

#[test]
fn floating_balloon_distances_follow_prefab_layout() {
    let star = derive::load_prefab("FloatingStarBox");
    let [star_x, star_y, _] = star
        .cumulative_local_pos_by_path("Balloons")
        .expect("missing FloatingStarBox Balloons transform");
    let expected_star = (star_x * star_x + star_y * star_y).sqrt();
    assert!((*FLOATING_STAR_BALLOON_DISTANCE - expected_star).abs() < 1e-6);

    let part = derive::load_prefab("FloatingPartBox");
    let [part_x, part_y, _] = part
        .cumulative_local_pos_by_path("Balloons")
        .expect("missing FloatingPartBox Balloons transform");
    let expected_part = (part_x * part_x + part_y * part_y).sqrt();
    assert!((*FLOATING_PART_BALLOON_DISTANCE - expected_part).abs() < 1e-6);
}

#[test]
fn floating_rope_box_anchor_follows_prefab_rope_visualization() {
    let star = derive::load_prefab("FloatingStarBox");
    let star_rope = star
        .component_by_path("Balloons", "RopeVisualization")
        .expect("missing FloatingStarBox Balloons RopeVisualization");
    let [star_x, star_y, _] = star_rope
        .field_vec3("m_pos2Anchor")
        .expect("missing FloatingStarBox RopeVisualization m_pos2Anchor");
    assert!((FLOATING_STAR_ROPE_BOX_ANCHOR_LOCAL.0 - star_x).abs() < 1e-6);
    assert!((FLOATING_STAR_ROPE_BOX_ANCHOR_LOCAL.1 - star_y).abs() < 1e-6);

    let part = derive::load_prefab("FloatingPartBox");
    let part_rope = part
        .component_by_path("Balloons", "RopeVisualization")
        .expect("missing FloatingPartBox Balloons RopeVisualization");
    let [part_x, part_y, _] = part_rope
        .field_vec3("m_pos2Anchor")
        .expect("missing FloatingPartBox RopeVisualization m_pos2Anchor");
    assert!((FLOATING_PART_ROPE_BOX_ANCHOR_LOCAL.0 - part_x).abs() < 1e-6);
    assert!((FLOATING_PART_ROPE_BOX_ANCHOR_LOCAL.1 - part_y).abs() < 1e-6);
}

#[test]
fn floating_balloon_rope_anchor_follows_prefab_rope_visualization() {
    let star = derive::load_prefab("FloatingStarBox");
    let star_rope = star
        .component_by_path("Balloons", "RopeVisualization")
        .expect("missing FloatingStarBox Balloons RopeVisualization");
    let star_anchor = star_rope
        .field_vec3("m_pos1Anchor")
        .expect("missing FloatingStarBox RopeVisualization m_pos1Anchor");
    let [star_x, star_y, _] = star
        .transform_point_by_path("Balloons", star_anchor)
        .expect("failed FloatingStarBox Balloons TransformPoint(m_pos1Anchor)");
    assert!((FLOATING_STAR_ROPE_BALLOON_ANCHOR_LOCAL.0 - star_x).abs() < 1e-6);
    assert!((FLOATING_STAR_ROPE_BALLOON_ANCHOR_LOCAL.1 - star_y).abs() < 1e-6);

    let part = derive::load_prefab("FloatingPartBox");
    let part_rope = part
        .component_by_path("Balloons", "RopeVisualization")
        .expect("missing FloatingPartBox Balloons RopeVisualization");
    let part_anchor = part_rope
        .field_vec3("m_pos1Anchor")
        .expect("missing FloatingPartBox RopeVisualization m_pos1Anchor");
    let [part_x, part_y, _] = part
        .transform_point_by_path("Balloons", part_anchor)
        .expect("failed FloatingPartBox Balloons TransformPoint(m_pos1Anchor)");
    assert!((FLOATING_PART_ROPE_BALLOON_ANCHOR_LOCAL.0 - part_x).abs() < 1e-6);
    assert!((FLOATING_PART_ROPE_BALLOON_ANCHOR_LOCAL.1 - part_y).abs() < 1e-6);
}
