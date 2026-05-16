use crate::domain::types::Vec3;

use super::{
    ParticleColor, bird_sleep_prefab, fan_puff_prefab, fly_swarm_prefab, magnet_effect_prefab,
    rocket_fire_prefab, turbo_charger_prefab, wind_area_prefab,
};

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-5,
        "expected {expected}, got {actual}"
    );
}

fn assert_color_close(actual: ParticleColor, expected: ParticleColor) {
    assert_close(actual.r, expected.r);
    assert_close(actual.g, expected.g);
    assert_close(actual.b, expected.b);
    assert_close(actual.a, expected.a);
}

#[test]
fn wind_area_prefab_loads_particle_values() {
    let prefab = wind_area_prefab().expect("wind area prefab should parse");
    let system = prefab.systems.first().expect("wind area particle system");

    assert_eq!(prefab.systems.len(), 3);
    assert_close(prefab.wind_direction.x, 4.8851843);
    assert_close(prefab.wind_direction.y, -6.33522);
    assert_close(prefab.power_factor, 1.5);
    assert_close(system.start_lifetime.sample(0.0, 0.0), 4.444445);
    assert_close(system.start_speed.sample(0.0, 0.0), 6.0);
    assert_close(system.start_speed.sample(0.0, 1.0), 9.0);
    assert_close(system.start_size.sample(0.0, 0.0), 0.4);
    assert_close(system.start_size.sample(0.0, 1.0), 0.7);
    assert!(!system.color_over_lifetime_enabled);
    assert_color_close(
        system.start_color.sample(0.0, 0.0),
        ParticleColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    );
    assert_eq!(system.max_particles, 20);
    assert_close(system.emission_rate.sample(0.0, 0.0), 1.0);
    assert_eq!(system.uv_module.tiles_x, 16);
    assert_eq!(system.uv_module.tiles_y, 16);
    assert_eq!(system.uv_module.row_index, 2);
    assert_eq!(system.uv_module.sample_frame_index(0.0), 4);
    assert_eq!(system.uv_module.sample_frame_index(0.999), 6);

    let dir = system.projected_forward_xy();
    assert_close(dir.x, 0.61064816);
    assert_close(dir.y, -0.79190195);
    let side = system.projected_up_xy();
    assert_close(side.x, 0.79190195);
    assert_close(side.y, 0.6106482);
}

#[test]
fn bird_sleep_prefab_loads_particle_values() {
    let prefab = bird_sleep_prefab().expect("bird sleep prefab should parse");
    let system = &prefab.system;

    assert!(!system.play_on_awake);
    assert!(!system.prewarm);
    assert_close(system.local_position.y, 0.5);
    assert_close(system.start_lifetime.sample(0.0, 0.0), 1.0);
    assert_close(system.start_lifetime.sample(0.0, 1.0), 2.0);
    assert_close(system.start_size.sample(0.0, 0.0), 0.7);
    assert_close(system.emission_rate.sample(0.0, 0.0), 2.0);
    assert!(!system.color_over_lifetime_enabled);
    assert_eq!(system.max_particles, 5);
    assert_eq!(system.uv_module.tiles_x, 8);
    assert_eq!(system.uv_module.tiles_y, 8);
    assert_eq!(system.uv_module.row_index, 2);
    assert_eq!(system.uv_module.sample_frame_index(0.0), 6);
    assert_close(system.size_over_lifetime.sample(0.355244, 0.0), 0.7);
    assert_close(system.rotation_over_lifetime.sample(0.0, 0.0), 0.0);
    assert_close(system.rotation_over_lifetime.sample(0.0, 1.0), std::f32::consts::FRAC_PI_6);
    assert_close(system.velocity_y.sample(0.0, 0.0), 0.49414596);
    assert_close(system.velocity_y.sample(0.0, 1.0), 0.3103452);

    let extents = system.projected_ellipsoid_half_extents_xy();
    assert_close(extents.x, 0.60200006);
    assert_close(extents.y, 0.518);
}

#[test]
fn fan_puff_prefab_loads_particle_values() {
    let prefab = fan_puff_prefab().expect("fan puff prefab should parse");
    let system = &prefab.system;

    assert_eq!(system.name, "Particles");
    assert_close(system.duration, 1.0);
    assert!(!system.play_on_awake);
    assert!(system.looping);
    assert_close(system.local_position.y, 0.6365275);
    assert_eq!(system.max_particles, 40);
    assert_close(system.start_lifetime.sample(0.0, 0.0), 0.7);
    assert_close(system.start_lifetime.sample(0.0, 1.0), 1.5);
    assert_close(system.start_size.sample(0.0, 0.0), 1.2);
    assert!(!system.color_over_lifetime_enabled);
    assert_close(system.velocity_x.sample(0.0, 0.0), 0.1);
    assert_close(system.velocity_x.sample(0.0, 1.0), -0.1);
    assert_close(system.velocity_y.sample(0.0, 0.0), 10.0);
    assert_close(system.velocity_y.sample(0.0, 1.0), 3.0);
    assert_close(system.force_y.sample(0.0, 0.0), -0.5);
    assert_close(system.uv_module.sample_frame_index(0.0) as f32, 3.0);
    assert_eq!(system.bursts.len(), 4);
    assert_close(system.bursts[0].time, 0.0);
    assert_close(system.bursts[1].time, 0.25);
    assert_close(system.bursts[2].time, 0.5);
    assert_close(system.bursts[3].time, 0.75);
    assert_eq!(system.bursts[0].sample_count(0.0), 1);
    assert_eq!(system.bursts[0].cycle_count, 1);
    assert_close(system.bursts[0].repeat_interval, 0.01);
    assert!(system.size_over_lifetime.sample(0.913, 0.5) > 0.12);
    assert!(system.rotation_over_lifetime.sample(0.0, 0.0) > 2.7);
    assert!(system.rotation_over_lifetime.sample(1.0, 0.0) < 0.5);
}

#[test]
fn rocket_fire_prefab_loads_particle_values() {
    let prefab = rocket_fire_prefab().expect("rocket fire prefab should parse");
    let system = prefab.systems.first().expect("rocket fire system");

    assert_eq!(prefab.systems.len(), 1);
    assert_eq!(system.name, "Particles_RocketFire_01_SET");
    assert_close(system.duration, 0.5);
    assert!(system.play_on_awake);
    assert!(system.prewarm);
    assert_close(system.start_lifetime.sample(0.0, 0.0), 0.5);
    assert_close(system.start_speed.sample(0.0, 0.0), 0.5);
    assert_close(system.start_speed.sample(0.0, 1.0), 2.0);
    assert_close(system.start_size.sample(0.0, 0.0), 0.2);
    assert_close(system.start_size.sample(0.0, 1.0), 0.4);
    assert!(!system.color_over_lifetime_enabled);
    assert_close(system.local_position.x, Vec3::default().x);
    assert_close(system.local_position.y, Vec3::default().y);
    assert_close(system.local_position.z, Vec3::default().z);
    assert_eq!(system.local_rotation, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(system.uv_module.tiles_x, 1);
    assert_eq!(system.uv_module.tiles_y, 1);
}

#[test]
fn turbo_charger_prefab_parses_enabled_color_gradient() {
    let prefab = turbo_charger_prefab().expect("turbo charger prefab should parse");
    let system = prefab.systems.first().expect("turbo charger system");

    assert_eq!(prefab.systems.len(), 1);
    assert_eq!(system.name, "TurboChargerEffect");
    assert!(system.color_over_lifetime_enabled);
    assert_eq!(system.uv_module.tiles_x, 8);
    assert_eq!(system.uv_module.tiles_y, 8);
    assert_eq!(system.uv_module.row_index, 2);

    let start = system.color_over_lifetime.sample(0.0, 0.0);
    let end = system.color_over_lifetime.sample(1.0, 0.0);
    assert!(start.r > 0.68 && start.g > 0.68 && start.b > 0.68);
    assert!(end.r < 0.01 && end.g < 0.01 && end.b < 0.01);
    assert_close(end.a, 1.0);

    let start_color = system.start_color.sample(0.0, 0.0);
    let end_color = system.start_color.sample(1.0, 0.0);
    assert!(start_color.r < 0.01 && start_color.g < 0.01 && start_color.b < 0.01);
    assert!(end_color.r < 0.01 && end_color.g < 0.01 && end_color.b < 0.01);
}

#[test]
fn magnet_effect_prefab_loads_multiple_particle_systems() {
    let prefab = magnet_effect_prefab().expect("magnet effect prefab should parse");

    assert_eq!(prefab.systems.len(), 3);
    assert_eq!(prefab.systems[0].name, "MagnetEffect");
    assert_eq!(prefab.systems[1].name, "MagnetBolts");
    assert_eq!(prefab.systems[2].name, "Particle System");
    assert_eq!(prefab.systems[0].bursts.len(), 2);
    assert_eq!(prefab.systems[0].uv_module.tiles_x, 16);
    assert_eq!(prefab.systems[0].uv_module.tiles_y, 8);

    let color_mid = prefab.systems[0].color_over_lifetime.sample(0.5, 0.0);
    assert!(color_mid.g > 0.4);
    assert!(color_mid.b > 0.7);
}

#[test]
fn fly_swarm_prefab_loads_particle_systems() {
    let prefab = fly_swarm_prefab().expect("fly swarm prefab should parse");

    assert!(!prefab.systems.is_empty());
    assert!(prefab.systems.iter().all(|system| !system.name.is_empty()));
}
