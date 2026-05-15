//! Unity particle system data types (color/curve/system definitions).

use crate::data::unity_anim::HermiteKey;
use crate::domain::types::{Vec2, Vec3};

use super::math::{
    lerp, normalize_xy, quaternion_axes, sample_gradient_alpha, sample_gradient_color,
    sample_hermite,
};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParticleColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ParticleColor {
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: lerp(self.r, other.r, t),
            g: lerp(self.g, other.g, t),
            b: lerp(self.b, other.b, t),
            a: lerp(self.a, other.a, t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnityColorGradient {
    pub color_keys: Vec<(f32, ParticleColor)>,
    pub alpha_keys: Vec<(f32, f32)>,
}

impl UnityColorGradient {
    pub fn constant(color: ParticleColor) -> Self {
        Self {
            color_keys: vec![(0.0, color), (1.0, color)],
            alpha_keys: vec![(0.0, color.a), (1.0, color.a)],
        }
    }

    pub fn sample(&self, time: f32) -> ParticleColor {
        let color = sample_gradient_color(&self.color_keys, time);
        ParticleColor {
            a: sample_gradient_alpha(&self.alpha_keys, time),
            ..color
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleColorGradient {
    pub mode: i32,
    pub min_color: ParticleColor,
    pub max_color: ParticleColor,
    pub min_gradient: UnityColorGradient,
    pub max_gradient: UnityColorGradient,
}

impl ParticleColorGradient {
    pub fn constant(color: ParticleColor) -> Self {
        Self {
            mode: 0,
            min_color: color,
            max_color: color,
            min_gradient: UnityColorGradient::constant(color),
            max_gradient: UnityColorGradient::constant(color),
        }
    }

    pub fn sample(&self, time: f32, random: f32) -> ParticleColor {
        let random = random.clamp(0.0, 1.0);
        match self.mode {
            2 => self
                .min_gradient
                .sample(time)
                .lerp(self.max_gradient.sample(time), random),
            3 => self.min_color.lerp(self.max_color, random),
            _ => self.max_gradient.sample(time),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleCurve {
    pub mode: i32,
    pub scalar: f32,
    pub min_scalar: f32,
    pub max_curve: Vec<HermiteKey>,
    pub min_curve: Vec<HermiteKey>,
}

impl ParticleCurve {
    pub fn constant(value: f32) -> Self {
        Self {
            mode: 0,
            scalar: value,
            min_scalar: value,
            max_curve: Vec::new(),
            min_curve: Vec::new(),
        }
    }

    pub fn sample(&self, time: f32, random: f32) -> f32 {
        let random = random.clamp(0.0, 1.0);
        match self.mode {
            1 => self.scalar * sample_hermite(&self.max_curve, time, 1.0),
            2 => {
                let min_value = sample_hermite(&self.min_curve, time, 1.0);
                let max_value = sample_hermite(&self.max_curve, time, 1.0);
                self.scalar * lerp(min_value, max_value, random)
            }
            3 => lerp(self.min_scalar, self.scalar, random),
            _ => self.scalar,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleUvModule {
    pub tiles_x: u32,
    pub tiles_y: u32,
    pub row_index: u32,
    pub animation_type: i32,
    pub frame_over_time: ParticleCurve,
}

impl ParticleUvModule {
    pub fn sample_frame_index(&self, random: f32) -> u32 {
        let tiles_x = self.tiles_x.max(1);
        let frame_span = if self.animation_type == 1 {
            tiles_x
        } else {
            tiles_x.saturating_mul(self.tiles_y.max(1))
        };
        if self.frame_over_time.mode == 3 {
            let min_frame =
                (self.frame_over_time.min_scalar.max(0.0) * frame_span as f32).floor();
            let max_frame = (self.frame_over_time.scalar.max(0.0) * frame_span as f32).floor();
            let min_frame = min_frame.min(max_frame) as u32;
            let max_frame = max_frame.max(min_frame as f32) as u32;
            let frame_count = max_frame.saturating_sub(min_frame) + 1;
            let offset = ((frame_count as f32) * random.clamp(0.0, 0.999_999)).floor() as u32;
            (min_frame + offset).min(frame_span - 1)
        } else {
            ((self.frame_over_time.sample(0.0, random).max(0.0) * frame_span as f32).floor()
                as u32)
                .min(frame_span - 1)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleBurst {
    pub time: f32,
    pub count: ParticleCurve,
    pub cycle_count: u32,
    pub repeat_interval: f32,
}

impl ParticleBurst {
    pub fn sample_count(&self, random: f32) -> usize {
        self.count.sample(0.0, random).round().max(0.0) as usize
    }
}

#[derive(Debug, Clone)]
pub struct UnityParticleSystemDef {
    pub name: String,
    pub local_position: Vec3,
    pub local_rotation: [f32; 4],
    pub duration: f32,
    pub play_on_awake: bool,
    pub prewarm: bool,
    pub looping: bool,
    pub max_particles: usize,
    pub start_lifetime: ParticleCurve,
    pub start_speed: ParticleCurve,
    pub start_color: ParticleColorGradient,
    pub start_size: ParticleCurve,
    pub start_rotation: ParticleCurve,
    pub emission_rate: ParticleCurve,
    pub color_over_lifetime_enabled: bool,
    pub color_over_lifetime: ParticleColorGradient,
    pub size_over_lifetime: ParticleCurve,
    pub rotation_over_lifetime: ParticleCurve,
    pub velocity_x: ParticleCurve,
    pub velocity_y: ParticleCurve,
    pub velocity_z: ParticleCurve,
    pub velocity_world_space: bool,
    pub force_x: ParticleCurve,
    pub force_y: ParticleCurve,
    pub force_z: ParticleCurve,
    pub force_world_space: bool,
    pub shape_scale: Vec3,
    pub shape_radius: f32,
    pub bursts: Vec<ParticleBurst>,
    pub uv_module: ParticleUvModule,
}

impl UnityParticleSystemDef {
    pub fn projected_right_xy(&self) -> Vec2 {
        let (right, _, _) = quaternion_axes(self.local_rotation);
        normalize_xy(right.0, right.1)
    }

    pub fn projected_up_xy(&self) -> Vec2 {
        let (_, up, _) = quaternion_axes(self.local_rotation);
        normalize_xy(up.0, up.1)
    }

    pub fn projected_forward_xy(&self) -> Vec2 {
        let (_, _, forward) = quaternion_axes(self.local_rotation);
        normalize_xy(forward.0, forward.1)
    }

    pub fn projected_ellipsoid_half_extents_xy(&self) -> Vec2 {
        let (right, up, forward) = quaternion_axes(self.local_rotation);
        let ax = self.shape_scale.x * self.shape_radius;
        let ay = self.shape_scale.y * self.shape_radius;
        let az = self.shape_scale.z * self.shape_radius;
        Vec2 {
            x: ((ax * right.0).powi(2) + (ay * up.0).powi(2) + (az * forward.0).powi(2)).sqrt(),
            y: ((ax * right.1).powi(2) + (ay * up.1).powi(2) + (az * forward.1).powi(2)).sqrt(),
        }
    }

    pub fn projected_ellipsoid_half_extents_xz(&self) -> Vec2 {
        let (right, up, forward) = quaternion_axes(self.local_rotation);
        let ax = self.shape_scale.x * self.shape_radius;
        let ay = self.shape_scale.y * self.shape_radius;
        let az = self.shape_scale.z * self.shape_radius;
        Vec2 {
            x: ((ax * right.0).powi(2) + (ay * up.0).powi(2) + (az * forward.0).powi(2)).sqrt(),
            y: ((ax * right.2).powi(2) + (ay * up.2).powi(2) + (az * forward.2).powi(2)).sqrt(),
        }
    }
}
