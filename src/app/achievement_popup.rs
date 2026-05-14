use std::collections::HashMap;
use std::sync::OnceLock;

use eframe::egui;

use crate::data::{assets, unity_anim};

use super::EditorApp;

const ACHIEVEMENT_ICON_PREFAB: &str = include_str!("../../unity_assets/Prefab/GameCenterManager.prefab");
const ACHIEVEMENT_SHEET_ASSET: &str = "unity/resources/achievements/achievements_sheet.png";
const ACHIEVEMENT_SHEET_GRID_COLUMNS: usize = 8;
const ACHIEVEMENT_SHEET_GRID_ROWS: usize = 8;
const DEFAULT_ACHIEVEMENT_POPUP_DURATION: f32 = 2.666667;
const DEFAULT_ACHIEVEMENT_POPUP_POS_Y: &[unity_anim::HermiteKey] = &[
    (0.0, 13.0, -9.0, -9.0),
    (0.5, 8.5, -9.0, 0.0),
    (2.166667, 8.5, 0.0, 9.000005),
    (2.666667, 13.0, 9.000005, 2.454545),
];
const POPUP_VISIBLE_Y: f32 = 8.5;
const POPUP_VISIBLE_TOP: f32 = 20.0;
const POPUP_PIXELS_PER_UNIT: f32 = 30.0;
const POPUP_SIZE: egui::Vec2 = egui::vec2(340.0, 78.0);

#[derive(Clone, Debug, PartialEq, Eq)]
enum AchievementIconSource {
    Individual(String),
    SheetIndex(usize),
}

pub(super) struct AchievementPopupPreview {
    achievement_id: String,
    icon_source: Option<AchievementIconSource>,
    started_at: f64,
}

impl EditorApp {
    pub(super) fn show_achievement_popup(&mut self, achievement_id: String, now: f64) {
        self.achievement_popup = Some(AchievementPopupPreview {
            icon_source: achievement_icon_lookup().get(&achievement_id).cloned(),
            achievement_id,
            started_at: now,
        });
    }

    pub(super) fn render_achievement_popup(&mut self, ctx: &egui::Context) {
        let Some((started_at, achievement_id, icon_source)) = self
            .achievement_popup
            .as_ref()
            .map(|popup| {
                (
                    popup.started_at,
                    popup.achievement_id.clone(),
                    popup.icon_source.clone(),
                )
            })
        else {
            return;
        };

        let now = ctx.input(|i| i.time);
        let elapsed = (now - started_at).max(0.0);
        let duration = achievement_popup_duration();
        if elapsed >= duration as f64 {
            self.achievement_popup = None;
            return;
        }

        let curve_y = achievement_popup_y(elapsed as f32);
        let top_offset = POPUP_VISIBLE_TOP - (curve_y - POPUP_VISIBLE_Y) * POPUP_PIXELS_PER_UNIT;
        let title = "Achievement Popup";
        let achievement_id = truncated_label(&achievement_id, 36);
        let icon_texture = icon_source
            .as_ref()
            .and_then(|source| self.load_achievement_popup_icon(ctx, source));

        egui::Area::new(egui::Id::new("achievement_popup_preview"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top_offset))
            .interactable(false)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(POPUP_SIZE, egui::Sense::hover());
                let painter = ui.painter();
                let shadow_rect = rect.translate(egui::vec2(0.0, 4.0));
                painter.rect_filled(
                    shadow_rect,
                    14,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 48),
                );
                painter.rect_filled(
                    rect,
                    14,
                    egui::Color32::from_rgba_unmultiplied(31, 36, 44, 244),
                );
                painter.rect_stroke(
                    rect,
                    14,
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(244, 196, 48, 180)),
                    egui::StrokeKind::Inside,
                );

                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 42.0, rect.center().y),
                    egui::vec2(48.0, 48.0),
                );
                painter.rect_filled(
                    icon_rect,
                    12.0,
                    egui::Color32::from_rgba_unmultiplied(12, 16, 20, 180),
                );
                painter.rect_stroke(
                    icon_rect,
                    12.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28)),
                    egui::StrokeKind::Inside,
                );
                if let Some(texture_id) = icon_texture {
                    painter.image(
                        texture_id,
                        icon_rect.shrink(4.0),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    let icon_center = icon_rect.center();
                    painter.circle_filled(
                        icon_center,
                        18.0,
                        egui::Color32::from_rgb(244, 196, 48),
                    );
                    painter.circle_filled(
                        icon_center,
                        9.0,
                        egui::Color32::from_rgb(255, 243, 194),
                    );
                }

                painter.text(
                    egui::pos2(rect.left() + 68.0, rect.top() + 18.0),
                    egui::Align2::LEFT_TOP,
                    title,
                    egui::FontId::proportional(15.0),
                    egui::Color32::from_rgb(248, 221, 124),
                );
                painter.text(
                    egui::pos2(rect.left() + 68.0, rect.top() + 40.0),
                    egui::Align2::LEFT_TOP,
                    achievement_id,
                    egui::FontId::proportional(20.0),
                    egui::Color32::WHITE,
                );
            });

        ctx.request_repaint();
    }

    fn load_achievement_popup_icon(
        &mut self,
        ctx: &egui::Context,
        source: &AchievementIconSource,
    ) -> Option<egui::TextureId> {
        match source {
            AchievementIconSource::Individual(icon_name) => {
                let asset_key = format!("unity/resources/achievements/{icon_name}.png");
                let cache_key = format!("achievement_popup_icon::{icon_name}");
                self.achievement_popup_tex_cache
                    .load_texture(ctx, &asset_key, &cache_key)
            }
            AchievementIconSource::SheetIndex(sheet_index) => {
                let cache_key = format!("achievement_popup_sheet::{sheet_index}");
                let uv_rect = achievement_sheet_uv(*sheet_index)?;
                self.achievement_popup_tex_cache
                    .load_sprite_crop(ctx, &cache_key, ACHIEVEMENT_SHEET_ASSET, uv_rect)
            }
        }
    }
}

fn achievement_icon_lookup() -> &'static HashMap<String, AchievementIconSource> {
    static LOOKUP: OnceLock<HashMap<String, AchievementIconSource>> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        parse_achievement_icon_lookup(ACHIEVEMENT_ICON_PREFAB, |icon_name| {
            assets::read_asset(&format!("unity/resources/achievements/{icon_name}.png")).is_some()
        })
    })
}

fn parse_achievement_icon_lookup(
    text: &str,
    mut has_individual_icon: impl FnMut(&str) -> bool,
) -> HashMap<String, AchievementIconSource> {
    let mut lookup = HashMap::new();
    let mut current_id = None;
    let mut next_sheet_index = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(id) = trimmed.strip_prefix("- id:") {
            current_id = Some(id.trim().to_string());
            continue;
        }
        if let Some(icon_name) = trimmed.strip_prefix("iconFileName:")
            && let Some(id) = current_id.take()
        {
            let icon_name = icon_name.trim().to_string();
            let source = if has_individual_icon(&icon_name) {
                AchievementIconSource::Individual(icon_name)
            } else {
                let sheet_index = next_sheet_index;
                next_sheet_index += 1;
                AchievementIconSource::SheetIndex(sheet_index)
            };
            lookup.insert(id, source);
        }
    }

    lookup
}

fn achievement_sheet_uv(sheet_index: usize) -> Option<[f32; 4]> {
    let capacity = ACHIEVEMENT_SHEET_GRID_COLUMNS * ACHIEVEMENT_SHEET_GRID_ROWS;
    if sheet_index >= capacity {
        return None;
    }

    let cell_w = 1.0 / ACHIEVEMENT_SHEET_GRID_COLUMNS as f32;
    let cell_h = 1.0 / ACHIEVEMENT_SHEET_GRID_ROWS as f32;
    let col = sheet_index % ACHIEVEMENT_SHEET_GRID_COLUMNS;
    let row = sheet_index / ACHIEVEMENT_SHEET_GRID_COLUMNS;
    Some([
        col as f32 * cell_w,
        1.0 - (row as f32 + 1.0) * cell_h,
        cell_w,
        cell_h,
    ])
}

fn achievement_popup_duration() -> f32 {
    unity_anim::achievement_popup_enter_clip()
        .map(|clip| clip.duration)
        .filter(|duration| *duration > 0.0)
        .unwrap_or(DEFAULT_ACHIEVEMENT_POPUP_DURATION)
}

fn achievement_popup_y(time: f32) -> f32 {
    let sample_time = time.clamp(0.0, achievement_popup_duration());
    sample_hermite(achievement_popup_pos_y_curve(), sample_time)
}

fn achievement_popup_pos_y_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::achievement_popup_enter_clip()
        .and_then(|clip| clip.root_position())
        .map(|curve| curve.y.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_ACHIEVEMENT_POPUP_POS_Y)
}

fn sample_hermite(keys: &[unity_anim::HermiteKey], time: f32) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }

    for window in keys.windows(2) {
        let [(t0, v0, _, out_slope), (t1, v1, in_slope, _)] = window else {
            continue;
        };
        if time > *t1 {
            continue;
        }

        let dt = *t1 - *t0;
        if dt.abs() <= f32::EPSILON {
            return *v1;
        }

        let u = ((time - *t0) / dt).clamp(0.0, 1.0);
        let u2 = u * u;
        let u3 = u2 * u;
        let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
        let h10 = u3 - 2.0 * u2 + u;
        let h01 = -2.0 * u3 + 3.0 * u2;
        let h11 = u3 - u2;
        return h00 * *v0 + h10 * dt * *out_slope + h01 * *v1 + h11 * dt * *in_slope;
    }

    keys.last().map(|key| key.1).unwrap_or(0.0)
}

fn truncated_label(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Unnamed achievement".to_string();
    }

    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        return trimmed.to_string();
    }

    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sheet_backed_icons_in_prefab_order() {
        let lookup = parse_achievement_icon_lookup(ACHIEVEMENT_ICON_PREFAB, |_| false);

        assert_eq!(
            lookup.get("grp.JUNIOR_WRECKER"),
            Some(&AchievementIconSource::SheetIndex(0))
        );
        assert_eq!(
            lookup.get("grp.GROUND_HOG_DAY"),
            Some(&AchievementIconSource::SheetIndex(1))
        );
        assert_eq!(
            lookup.get("grp.MEGA_MASTER_EXPLORER"),
            Some(&AchievementIconSource::SheetIndex(59))
        );
    }

    #[test]
    fn prefers_individual_icons_when_present() {
        let lookup = parse_achievement_icon_lookup(ACHIEVEMENT_ICON_PREFAB, |icon_name| {
            icon_name == "62_gap_the_bridge" || icon_name == "89_hidden_crate"
        });

        assert_eq!(
            lookup.get("grp.LPA_BRIDGE_BREAK"),
            Some(&AchievementIconSource::Individual(
                "62_gap_the_bridge".to_string()
            ))
        );
        assert_eq!(
            lookup.get("grp.HIDDEN_CRATE"),
            Some(&AchievementIconSource::Individual("89_hidden_crate".to_string()))
        );
        assert_eq!(
            lookup.get("grp.GROUND_HOG_DAY"),
            Some(&AchievementIconSource::SheetIndex(1))
        );
    }

    #[test]
    fn computes_sheet_uv_on_eight_by_eight_grid() {
        assert_eq!(achievement_sheet_uv(0), Some([0.0, 0.875, 0.125, 0.125]));
        assert_eq!(achievement_sheet_uv(63), Some([0.875, 0.0, 0.125, 0.125]));
        assert_eq!(achievement_sheet_uv(64), None);
    }
}