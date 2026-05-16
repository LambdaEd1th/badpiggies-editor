//! Internal math helpers for Unity particle sampling.

use crate::domain::types::Vec2;

use super::types::ParticleColor;

pub(super) fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t
}

pub(super) fn normalize_xy(x: f32, y: f32) -> Vec2 {
    let len = (x * x + y * y).sqrt();
    if len <= f32::EPSILON {
        return Vec2 { x: 0.0, y: 0.0 };
    }
    Vec2 {
        x: x / len,
        y: y / len,
    }
}

pub(super) type Axis3 = (f32, f32, f32);

pub(super) fn quaternion_axes(quat: [f32; 4]) -> (Axis3, Axis3, Axis3) {
    let [x, y, z, w] = quat;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;

    let m00 = 1.0 - 2.0 * (yy + zz);
    let m01 = 2.0 * (xy - wz);
    let m02 = 2.0 * (xz + wy);
    let m10 = 2.0 * (xy + wz);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - wx);
    let m20 = 2.0 * (xz - wy);
    let m21 = 2.0 * (yz + wx);
    let m22 = 1.0 - 2.0 * (xx + yy);

    ((m00, m10, m20), (m01, m11, m21), (m02, m12, m22))
}

pub(super) fn sample_hermite(keys: &[(f32, f32, f32, f32)], time: f32, fallback: f32) -> f32 {
    let n = keys.len();
    if n == 0 {
        return fallback;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }
    if time >= keys[n - 1].0 {
        return keys[n - 1].1;
    }

    let mut index = 0;
    while index < n - 2 && keys[index + 1].0 < time {
        index += 1;
    }

    let (t0, v0, _, out_slope) = keys[index];
    let (t1, v1, in_slope, _) = keys[index + 1];
    let dt = t1 - t0;
    let s = (time - t0) / dt;
    let s2 = s * s;
    let s3 = s2 * s;

    (2.0 * s3 - 3.0 * s2 + 1.0) * v0
        + (s3 - 2.0 * s2 + s) * (out_slope * dt)
        + (-2.0 * s3 + 3.0 * s2) * v1
        + (s3 - s2) * (in_slope * dt)
}

pub(super) fn sample_gradient_color(keys: &[(f32, ParticleColor)], time: f32) -> ParticleColor {
    let time = time.clamp(0.0, 1.0);
    if time <= keys[0].0 {
        return keys[0].1;
    }
    for window in keys.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
        if time <= t1 {
            let span = (t1 - t0).max(f32::EPSILON);
            return c0.lerp(c1, (time - t0) / span);
        }
    }
    keys[keys.len() - 1].1
}

pub(super) fn sample_gradient_alpha(keys: &[(f32, f32)], time: f32) -> f32 {
    let time = time.clamp(0.0, 1.0);
    if time <= keys[0].0 {
        return keys[0].1;
    }
    for window in keys.windows(2) {
        let (t0, a0) = window[0];
        let (t1, a1) = window[1];
        if time <= t1 {
            let span = (t1 - t0).max(f32::EPSILON);
            return lerp(a0, a1, (time - t0) / span);
        }
    }
    keys[keys.len() - 1].1
}
