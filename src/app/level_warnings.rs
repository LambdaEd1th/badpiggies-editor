use eframe::egui;

use crate::domain::level_warning::{LevelWarning, LevelWarningSeverity, collect_level_warnings};
use crate::i18n::locale::I18n;
use crate::renderer::PreviewPlaybackState;

use super::EditorApp;
use super::state::{PendingLevelWarning, PendingLevelWarningAction};

impl EditorApp {
    pub(in crate::app) fn current_level_warnings(&self) -> Vec<LevelWarning> {
        self.tabs[self.active_tab]
            .level
            .as_ref()
            .map(collect_level_warnings)
            .unwrap_or_default()
    }

    pub(in crate::app) fn request_preview_playback_state(
        &mut self,
        state: PreviewPlaybackState,
        t: &'static I18n,
    ) {
        if self.tabs[self.active_tab].renderer.preview_playback_state() == state {
            return;
        }

        if state == PreviewPlaybackState::Build {
            self.tabs[self.active_tab]
                .renderer
                .set_preview_playback_state(state);
            return;
        }

        self.queue_or_execute_level_warning(
            PendingLevelWarningAction::PreviewPlaybackState(state),
            t,
        );
    }

    pub(in crate::app) fn request_export_level_bytes(&mut self, t: &'static I18n) {
        self.queue_or_execute_level_warning(PendingLevelWarningAction::ExportLevel, t);
    }

    pub(in crate::app) fn maybe_warn_about_new_level_risks(
        &mut self,
        previous_warnings: &[LevelWarning],
    ) {
        let current_warnings = self.current_level_warnings();
        if !has_new_level_warning(previous_warnings, &current_warnings) {
            return;
        }

        self.tabs[self.active_tab].pending_level_warning = Some(PendingLevelWarning {
            warnings: current_warnings,
            action: PendingLevelWarningAction::AcknowledgeOnly,
        });
    }

    fn queue_or_execute_level_warning(
        &mut self,
        action: PendingLevelWarningAction,
        t: &'static I18n,
    ) {
        let Some(level) = self.tabs[self.active_tab].level.as_ref() else {
            return;
        };

        let warnings = collect_level_warnings(level);
        if warnings.is_empty() {
            self.execute_level_warning_action(action, t);
            return;
        }

        self.tabs[self.active_tab].pending_level_warning =
            Some(PendingLevelWarning { warnings, action });
    }

    fn execute_level_warning_action(
        &mut self,
        action: PendingLevelWarningAction,
        t: &'static I18n,
    ) {
        match action {
            PendingLevelWarningAction::AcknowledgeOnly => {}
            PendingLevelWarningAction::PreviewPlaybackState(state) => {
                self.tabs[self.active_tab]
                    .renderer
                    .set_preview_playback_state(state);
            }
            PendingLevelWarningAction::ExportLevel => {
                self.export_level_bytes_now(t);
            }
        }
    }

    pub(in crate::app) fn render_level_warning_confirm(
        &mut self,
        ctx: &egui::Context,
        t: &'static I18n,
    ) {
        let Some(pending) = self.tabs[self.active_tab].pending_level_warning.clone() else {
            return;
        };

        let mut action = 0u8;
        let (high_warnings, low_warnings) = partition_level_warnings(&pending.warnings);
        egui::Window::new(t.get("win_level_warning"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(level_warning_intro(pending.action, &pending.warnings, t));
                ui.add_space(8.0);
                render_level_warning_section(
                    ui,
                    t.get("level_warning_section_high"),
                    &high_warnings,
                    t,
                );
                if !high_warnings.is_empty() && !low_warnings.is_empty() {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                }
                render_level_warning_section(
                    ui,
                    t.get("level_warning_section_low"),
                    &low_warnings,
                    t,
                );
                ui.add_space(8.0);
                ui.label(t.get("level_warning_continue"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(t.get("btn_i_understand_the_risks")).clicked() {
                        action = 1;
                    }
                    if pending.action != PendingLevelWarningAction::AcknowledgeOnly
                        && ui.button(t.get("btn_cancel")).clicked()
                    {
                        action = 2;
                    }
                });
            });

        match action {
            1 => {
                self.tabs[self.active_tab].pending_level_warning = None;
                self.execute_level_warning_action(pending.action, t);
            }
            2 => {
                self.tabs[self.active_tab].pending_level_warning = None;
            }
            _ => {}
        }
    }
}

fn level_warning_intro(
    action: PendingLevelWarningAction,
    warnings: &[LevelWarning],
    t: &'static I18n,
) -> String {
    let has_high_risk = warnings
        .iter()
        .any(|warning| warning.severity() == LevelWarningSeverity::High);

    match (action, has_high_risk) {
        (PendingLevelWarningAction::AcknowledgeOnly, true) => t.get("level_warning_intro_editor"),
        (PendingLevelWarningAction::AcknowledgeOnly, false) => {
            t.get("level_warning_intro_editor_low")
        }
        (PendingLevelWarningAction::PreviewPlaybackState(_), true) => {
            t.get("level_warning_intro_preview")
        }
        (PendingLevelWarningAction::PreviewPlaybackState(_), false) => {
            t.get("level_warning_intro_preview_low")
        }
        (PendingLevelWarningAction::ExportLevel, true) => t.get("level_warning_intro_export"),
        (PendingLevelWarningAction::ExportLevel, false) => t.get("level_warning_intro_export_low"),
    }
}

fn describe_level_warning(warning: LevelWarning, t: &'static I18n) -> String {
    t.fmt_name_count(warning.message_key(), warning.object_name, warning.count)
}

fn partition_level_warnings(warnings: &[LevelWarning]) -> (Vec<LevelWarning>, Vec<LevelWarning>) {
    warnings
        .iter()
        .copied()
        .partition(|warning| warning.severity() == LevelWarningSeverity::High)
}

fn render_level_warning_section(
    ui: &mut egui::Ui,
    title: String,
    warnings: &[LevelWarning],
    t: &'static I18n,
) {
    if warnings.is_empty() {
        return;
    }

    ui.strong(title);
    ui.add_space(4.0);
    for warning in warnings {
        render_level_warning_row(ui, *warning, t);
    }
}

fn render_level_warning_row(ui: &mut egui::Ui, warning: LevelWarning, t: &'static I18n) {
    let (badge_fill, badge_text) = level_warning_badge_colors(warning);

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(t.get(level_warning_badge_key(warning)))
                .small()
                .strong()
                .color(badge_text)
                .background_color(badge_fill),
        );
        ui.label(describe_level_warning(warning, t));
    });
}

fn level_warning_badge_key(warning: LevelWarning) -> &'static str {
    match warning.severity() {
        LevelWarningSeverity::High => "level_warning_badge_high",
        LevelWarningSeverity::Low => "level_warning_badge_low",
    }
}

fn level_warning_badge_colors(warning: LevelWarning) -> (egui::Color32, egui::Color32) {
    match warning.severity() {
        LevelWarningSeverity::High => (
            egui::Color32::from_rgb(120, 38, 45),
            egui::Color32::from_rgb(255, 236, 238),
        ),
        LevelWarningSeverity::Low => (
            egui::Color32::from_rgb(105, 87, 28),
            egui::Color32::from_rgb(255, 247, 214),
        ),
    }
}

fn has_new_level_warning(previous: &[LevelWarning], current: &[LevelWarning]) -> bool {
    current.iter().any(|warning| {
        previous
            .iter()
            .find(|prev| prev.kind == warning.kind)
            .is_none_or(|prev| warning.count > prev.count)
    })
}

#[cfg(test)]
mod tests {
    use super::{level_warning_badge_key, partition_level_warnings};
    use crate::domain::level_warning::{LevelWarning, LevelWarningKind};

    #[test]
    fn partitions_warnings_by_severity_preserving_relative_order() {
        let warnings = vec![
            LevelWarning {
                kind: LevelWarningKind::MissingDessertPlaces,
                object_name: "DessertPlaces",
                count: 0,
            },
            LevelWarning {
                kind: LevelWarningKind::MissingLevelManager,
                object_name: "LevelManager",
                count: 0,
            },
            LevelWarning {
                kind: LevelWarningKind::MultipleDessertPlaces,
                object_name: "DessertPlaces",
                count: 2,
            },
            LevelWarning {
                kind: LevelWarningKind::MissingWorldObject,
                object_name: "World-tagged background",
                count: 0,
            },
        ];

        let (high, low) = partition_level_warnings(&warnings);

        assert_eq!(
            high,
            vec![
                LevelWarning {
                    kind: LevelWarningKind::MissingLevelManager,
                    object_name: "LevelManager",
                    count: 0,
                },
                LevelWarning {
                    kind: LevelWarningKind::MissingWorldObject,
                    object_name: "World-tagged background",
                    count: 0,
                },
            ]
        );
        assert_eq!(
            low,
            vec![
                LevelWarning {
                    kind: LevelWarningKind::MissingDessertPlaces,
                    object_name: "DessertPlaces",
                    count: 0,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleDessertPlaces,
                    object_name: "DessertPlaces",
                    count: 2,
                },
            ]
        );
    }

    #[test]
    fn maps_badge_keys_from_warning_severity() {
        assert_eq!(
            level_warning_badge_key(LevelWarning {
                kind: LevelWarningKind::MissingLevelManager,
                object_name: "LevelManager",
                count: 0,
            }),
            "level_warning_badge_high"
        );
        assert_eq!(
            level_warning_badge_key(LevelWarning {
                kind: LevelWarningKind::MissingDessertPlaces,
                object_name: "DessertPlaces",
                count: 0,
            }),
            "level_warning_badge_low"
        );
    }
}
