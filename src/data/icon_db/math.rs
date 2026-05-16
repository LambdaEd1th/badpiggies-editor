//! 2x3 affine matrix helpers and float rounding for icon-layer baking.

use super::types::Mat2x3;

pub(super) fn quat_to_z_angle(qx: f32, qy: f32, qz: f32, qw: f32) -> f32 {
    (2.0 * (qw * qz + qx * qy)).atan2(1.0 - 2.0 * (qy * qy + qz * qz))
}

pub(super) fn make_local_trs(position: [f32; 2], scale: [f32; 2], rotation: [f32; 4]) -> Mat2x3 {
    let angle = quat_to_z_angle(rotation[0], rotation[1], rotation[2], rotation[3]);
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    (
        cos_a * scale[0],
        -sin_a * scale[1],
        sin_a * scale[0],
        cos_a * scale[1],
        position[0],
        position[1],
    )
}

pub(super) fn mat_compose(m1: Mat2x3, m2: Mat2x3) -> Mat2x3 {
    let (a1, b1, c1, d1, tx1, ty1) = m1;
    let (a2, b2, c2, d2, tx2, ty2) = m2;
    (
        a1 * a2 + b1 * c2,
        a1 * b2 + b1 * d2,
        c1 * a2 + d1 * c2,
        c1 * b2 + d1 * d2,
        a1 * tx2 + b1 * ty2 + tx1,
        c1 * tx2 + d1 * ty2 + ty1,
    )
}

pub(super) fn mat_apply(m: Mat2x3, x: f32, y: f32) -> (f32, f32) {
    let (a, b, c, d, tx, ty) = m;
    (a * x + b * y + tx, c * x + d * y + ty)
}

pub(super) fn round_six(value: f32) -> f32 {
    (value * 1_000_000.0).round() / 1_000_000.0
}
