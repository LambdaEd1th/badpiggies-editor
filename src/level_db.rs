//! Reverse lookup: `.contraption` SHA1-hex filenames → level scene names.
//!
//! The game computes contraption filenames as:
//!   `SHA1(UTF8(levelName))[..10].toUpperHex() + ".contraption"`
//! (first 10 bytes of SHA1 = 20 uppercase hex chars).

use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::sync::OnceLock;

/// (display_label, scene_name) for every known level.
///
/// Labels follow the pattern "E{ep}-{num}" for episodes, "S-{num}" for sandbox, "R-{num}" for race.
const LEVEL_SCENES: &[(&str, &str)] = &[
    // ── Episode 1 — Ground Hog Day ──
    ("1-1", "Level_21"),
    ("1-2", "Level_05"),
    ("1-3", "test28"),
    ("1-4", "Level_20"),
    ("1-5", "Level_23"),
    ("1-6", "Level_49"),
    ("1-7", "test02"),
    ("1-8", "Level_22"),
    ("1-9", "test05"),
    ("1-10", "test29"),
    ("1-11", "test51"),
    ("1-12", "test31"),
    ("1-13", "Level_17"),
    ("1-14", "test11"),
    ("1-15", "test33"),
    ("1-16", "Level_24"),
    ("1-17", "Level_25"),
    ("1-18", "Level_14"),
    ("1-19", "test13"),
    ("1-20", "Level_27"),
    ("1-21", "Level_26"),
    ("1-22", "Level_15"),
    ("1-23", "test21"),
    ("1-24", "Level_18"),
    ("1-25", "test34"),
    ("1-26", "test35"),
    ("1-27", "test39"),
    ("1-28", "test40"),
    ("1-29", "test36"),
    ("1-30", "test37"),
    ("1-31", "test22"),
    ("1-32", "test25"),
    ("1-33", "test26"),
    ("1-34", "test23"),
    ("1-35", "Level_29"),
    ("1-36", "test41"),
    ("1-37", "test43"),
    ("1-38", "test44"),
    ("1-39", "test45"),
    ("1-40", "test42"),
    ("1-41", "Level_30"),
    ("1-42", "test47"),
    ("1-43", "Level_31"),
    ("1-44", "Level_32"),
    ("1-45", "Level_33"),
    // ── Episode 2 — Rise and Swine ──
    ("2-1", "scenario_69"),
    ("2-2", "scenario_70"),
    ("2-3", "scenario_73"),
    ("2-4", "scenario_71"),
    ("2-5", "track_18"),
    ("2-6", "scenario_59"),
    ("2-7", "scenario_60"),
    ("2-8", "scenario_61"),
    ("2-9", "scenario_63"),
    ("2-10", "scenario_62"),
    ("2-11", "course_02"),
    ("2-12", "scenario_75"),
    ("2-13", "scenario_72"),
    ("2-14", "track_21"),
    ("2-15", "track_22"),
    ("2-16", "scenario_85"),
    ("2-17", "scenario_88"),
    ("2-18", "scenario_91"),
    ("2-19", "scenario_94"),
    ("2-20", "scenario_93"),
    ("2-21", "scenario_86"),
    ("2-22", "scenario_87"),
    ("2-23", "scenario_92"),
    ("2-24", "scenario_90"),
    ("2-25", "scenario_89"),
    ("2-26", "scenario_58"),
    ("2-27", "track_14"),
    ("2-28", "track_25"),
    ("2-29", "course_04"),
    ("2-30", "scenario_76"),
    ("2-31", "track_20"),
    ("2-32", "scenario_74"),
    ("2-33", "scenario_82"),
    ("2-34", "track_31"),
    ("2-35", "scenario_84"),
    ("2-36", "track_29"),
    ("2-37", "scenario_67"),
    ("2-38", "scenario_80"),
    ("2-39", "track_27"),
    ("2-40", "scenario_77"),
    ("2-41", "track_28"),
    ("2-42", "scenario_81"),
    ("2-43", "track_12"),
    ("2-44", "scenario_79"),
    ("2-45", "scenario_65"),
    // ── Episode 3 — When Pigs Fly ──
    ("3-1", "Level_01"),
    ("3-2", "Level_16"),
    ("3-3", "Level_02"),
    ("3-4", "Level_03"),
    ("3-5", "Level_35"),
    ("3-6", "Level_04"),
    ("3-7", "test03"),
    ("3-8", "Level_07"),
    ("3-9", "Level_12"),
    ("3-10", "Level_34"),
    ("3-11", "test49"),
    ("3-12", "test01"),
    ("3-13", "test48"),
    ("3-14", "Level_11"),
    ("3-15", "Level_36"),
    ("3-16", "test14"),
    ("3-17", "test17"),
    ("3-18", "Level_37"),
    ("3-19", "test06"),
    ("3-20", "test18"),
    ("3-21", "Level_38"),
    ("3-22", "Level_39"),
    ("3-23", "Level_40"),
    ("3-24", "Level_41"),
    ("3-25", "test50"),
    ("3-26", "Level_09"),
    ("3-27", "Level_19"),
    ("3-28", "test04"),
    ("3-29", "Level_42"),
    ("3-30", "Level_48"),
    ("3-31", "Level_44"),
    ("3-32", "Level_46"),
    ("3-33", "Level_45"),
    ("3-34", "Level_47"),
    ("3-35", "test12"),
    ("3-36", "Level_06"),
    ("3-37", "test07"),
    ("3-38", "Level_08"),
    ("3-39", "test10"),
    ("3-40", "test09"),
    ("3-41", "test08"),
    ("3-42", "Level_13"),
    ("3-43", "test27"),
    ("3-44", "Level_10"),
    ("3-45", "test16"),
    // ── Episode 4 — Flight in the Night ──
    ("4-1", "Level_43"),
    ("4-2", "Level_51"),
    ("4-3", "scenario_11"),
    ("4-4", "test20"),
    ("4-5", "test15"),
    ("4-6", "scenario_18"),
    ("4-7", "Level_53"),
    ("4-8", "Level_52"),
    ("4-9", "Level_54"),
    ("4-10", "test59"),
    ("4-11", "scenario_03"),
    ("4-12", "test52"),
    ("4-13", "scenario_20"),
    ("4-14", "test54"),
    ("4-15", "test53"),
    ("4-16", "scenario_22"),
    ("4-17", "scenario_16"),
    ("4-18", "scenario_12"),
    ("4-19", "scenario_14"),
    ("4-20", "scenario_15"),
    ("4-21", "scenario_32"),
    ("4-22", "scenario_33"),
    ("4-23", "scenario_40"),
    ("4-24", "track_07"),
    ("4-25", "track_05"),
    ("4-26", "scenario_54"),
    ("4-27", "scenario_53"),
    ("4-28", "scenario_52"),
    ("4-29", "scenario_51"),
    ("4-30", "scenario_49"),
    ("4-31", "scenario_35"),
    ("4-32", "scenario_36"),
    ("4-33", "scenario_39"),
    ("4-34", "scenario_56"),
    ("4-35", "track_03"),
    ("4-36", "scenario_29"),
    ("4-37", "scenario_27"),
    ("4-38", "scenario_28"),
    ("4-39", "scenario_25"),
    ("4-40", "scenario_30"),
    ("4-41", "scenario_38"),
    ("4-42", "scenario_41"),
    ("4-43", "scenario_42"),
    ("4-44", "scenario_50"),
    ("4-45", "track_06"),
    // ── Episode 5 — Tusk 'til Dawn ──
    ("5-1", "scenario_55"),
    ("5-2", "scenario_45"),
    ("5-3", "scenario_26"),
    ("5-4", "scenario_44"),
    ("5-5", "scenario_46"),
    ("5-6", "scenario_24"),
    ("5-7", "track_04"),
    ("5-8", "scenario_37"),
    ("5-9", "scenario_21"),
    ("5-10", "scenario_97"),
    ("5-11", "scenario_02"),
    ("5-12", "test55"),
    ("5-13", "test57"),
    ("5-14", "scenario_31"),
    ("5-15", "scenario_10"),
    ("5-16", "test30"),
    ("5-17", "track_13"),
    ("5-18", "scenario_83"),
    ("5-19", "scenario_05"),
    ("5-20", "scenario_08"),
    ("5-21", "scenario_100"),
    ("5-22", "scenario_09"),
    ("5-23", "scenario_78"),
    ("5-24", "scenario_13"),
    ("5-25", "scenario_06"),
    ("5-26", "scenario_95"),
    ("5-27", "scenario_96"),
    ("5-28", "scenario_98"),
    ("5-29", "track_30"),
    ("5-30", "scenario_99"),
    // ── Episode 6 — Road Hogs ──
    ("6-1", "episode_6_level_1"),
    ("6-2", "episode_6_level_2"),
    ("6-3", "episode_6_level_3"),
    ("6-4", "episode_6_level_4"),
    ("6-5", "episode_6_level_I"),
    ("6-6", "episode_6_level_5"),
    ("6-7", "episode_6_level_6"),
    ("6-8", "episode_6_level_7"),
    ("6-9", "episode_6_level_8"),
    ("6-10", "episode_6_level_II"),
    ("6-11", "episode_6_level_9"),
    ("6-12", "episode_6_level_10"),
    ("6-13", "episode_6_level_11"),
    ("6-14", "episode_6_level_12"),
    ("6-15", "episode_6_level_III"),
    ("6-16", "episode_6_level_13"),
    ("6-17", "episode_6_level_14"),
    ("6-18", "episode_6_level_15"),
    ("6-19", "episode_6_level_16"),
    ("6-20", "episode_6_level_IV"),
    ("6-21", "episode_6_level_17"),
    ("6-22", "episode_6_level_18"),
    ("6-23", "episode_6_level_19"),
    ("6-24", "episode_6_level_20"),
    ("6-25", "episode_6_level_V"),
    ("6-26", "episode_6_level_21"),
    ("6-27", "episode_6_level_22"),
    ("6-28", "episode_6_level_23"),
    ("6-29", "episode_6_level_24"),
    ("6-30", "episode_6_level_VI"),
    ("6-31", "episode_6_level_25"),
    ("6-32", "episode_6_level_26"),
    ("6-33", "episode_6_level_27"),
    ("6-34", "episode_6_level_28"),
    ("6-35", "episode_6_level_VII"),
    ("6-36", "episode_6_level_29"),
    ("6-37", "episode_6_level_30"),
    ("6-38", "episode_6_level_31"),
    ("6-39", "episode_6_level_32"),
    ("6-40", "episode_6_level_VIII"),
    ("6-41", "episode_6_level_33"),
    ("6-42", "episode_6_level_34"),
    ("6-43", "episode_6_level_35"),
    ("6-44", "episode_6_level_36"),
    ("6-45", "episode_6_level_IX"),
    // ── Sandbox ──
    ("S-1", "Level_Sandbox_04"),
    ("S-2", "Level_Sandbox_02"),
    ("S-3", "Level_Sandbox_03"),
    ("S-4", "Level_Sandbox_05"),
    ("S-5", "Level_Sandbox_07"),
    ("S-6", "Level_Sandbox_08"),
    ("S-S", "Level_Sandbox_01"),
    ("S-F", "Level_Sandbox_06"),
    ("S-7", "Level_Sandbox_09"),
    ("S-8", "Level_Sandbox_10"),
    ("S-M", "MMSandbox"),
    ("S-9", "Episode_6_Tower Sandbox"),
    ("S-D", "Episode_6_Dark Sandbox"),
    ("S-10", "Episode_6_Ice Sandbox"),
    // ── Race ──
    ("R-1", "Level_Race_04"),
    ("R-2", "Level_Race_05"),
    ("R-3", "Level_Race_06"),
    ("R-4", "Level_Race_01"),
    ("R-5", "Level_Race_02"),
    ("R-6", "Level_Race_03"),
    ("R-7", "Level_Race_07"),
    ("R-8", "Level_Race_08"),
];

/// Sandbox scene names — used to generate multi-slot variants.
const SANDBOX_SCENES: &[&str] = &[
    "Level_Sandbox_01",
    "Level_Sandbox_02",
    "Level_Sandbox_03",
    "Level_Sandbox_04",
    "Level_Sandbox_05",
    "Level_Sandbox_06",
    "Level_Sandbox_07",
    "Level_Sandbox_08",
    "Level_Sandbox_09",
    "Level_Sandbox_10",
    "MMSandbox",
    "Episode_6_Tower Sandbox",
    "Episode_6_Dark Sandbox",
    "Episode_6_Ice Sandbox",
];

/// Race scene names — used to generate CakeRace variants.
const RACE_SCENES: &[&str] = &[
    "Level_Race_01",
    "Level_Race_02",
    "Level_Race_03",
    "Level_Race_04",
    "Level_Race_05",
    "Level_Race_06",
    "Level_Race_07",
    "Level_Race_08",
];

/// Compute the `.contraption` filename stem (20 uppercase hex chars) for a level key.
fn contraption_hash(level_key: &str) -> String {
    let hash = Sha1::digest(level_key.as_bytes());
    hash[..10].iter().map(|b| format!("{b:02X}")).collect()
}

/// Build the reverse-lookup map: SHA1 hex stem → (display_label, scene_key).
fn build_lookup() -> HashMap<String, (String, String)> {
    let mut map = HashMap::with_capacity(1200);

    // All base level scenes.
    for &(label, scene) in LEVEL_SCENES {
        let hash = contraption_hash(scene);
        map.insert(hash, (label.to_string(), scene.to_string()));
    }

    // Sandbox extra slots (1..=40).  Slot 0 uses the bare scene name (already above).
    for &scene in SANDBOX_SCENES {
        for slot in 1..=40 {
            let key = format!("{scene}_{slot}");
            let hash = contraption_hash(&key);
            map.entry(hash)
                .or_insert_with(|| (String::new(), key));
        }
    }

    // CakeRace variants: "cr_{scene}_{track}".
    for &scene in RACE_SCENES {
        for track in 0..=20 {
            let key = format!("cr_{scene}_{track}");
            let hash = contraption_hash(&key);
            map.entry(hash)
                .or_insert_with(|| (String::new(), key));
        }
    }

    map
}

/// Look up the level name for a `.contraption` filename.
///
/// `filename_stem` is the filename without the `.contraption` extension (20 hex chars).
/// Returns `(display_label, scene_key)`.  The label may be empty for sandbox-slot / cake-race
/// variants that don't map to a numbered level.
pub fn contraption_level_name(filename_stem: &str) -> Option<(&'static str, &'static str)> {
    static LOOKUP: OnceLock<HashMap<String, (String, String)>> = OnceLock::new();
    let map = LOOKUP.get_or_init(build_lookup);
    let upper = filename_stem.to_ascii_uppercase();
    map.get(&upper)
        .map(|(label, scene)| (label.as_str(), scene.as_str()))
}
