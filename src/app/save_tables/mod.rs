//! Structured save data table editors — Progress, Contraption, Achievements.

mod achievements;
mod contraption;
mod progress;

pub(in crate::app) use achievements::edit_achievements;
pub(in crate::app) use contraption::edit_contraption;
pub(in crate::app) use progress::edit_progress;

use std::collections::HashSet;

use eframe::egui;

/// Remove entries at the given indices (in reverse order to keep indices valid).
pub(super) fn remove_indices<T>(vec: &mut Vec<T>, indices: &HashSet<usize>) {
    let mut sorted: Vec<usize> = indices.iter().copied().collect();
    sorted.sort_unstable();
    for idx in sorted.into_iter().rev() {
        if idx < vec.len() {
            vec.remove(idx);
        }
    }
}

/// Duplicate entries at the given indices, appending copies at the end.
pub(super) fn duplicate_indices<T: Clone>(vec: &mut Vec<T>, indices: &HashSet<usize>) {
    let mut sorted: Vec<usize> = indices.iter().copied().collect();
    sorted.sort_unstable();
    let cloned: Vec<T> = sorted
        .into_iter()
        .filter_map(|i| vec.get(i).cloned())
        .collect();
    vec.extend(cloned);
}

/// Handle row click with modifier keys for selection.
fn handle_row_click(
    modifiers: &egui::Modifiers,
    actual_idx: usize,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    filtered_indices: &[usize],
) {
    if modifiers.command {
        // Cmd/Ctrl+click: toggle
        if !selected.remove(&actual_idx) {
            selected.insert(actual_idx);
        }
    } else if modifiers.shift {
        // Shift+click: range select
        if let Some(anchor) = *last_clicked {
            // Find positions in the filtered list
            let pos_anchor = filtered_indices.iter().position(|&i| i == anchor);
            let pos_current = filtered_indices.iter().position(|&i| i == actual_idx);
            if let (Some(a), Some(b)) = (pos_anchor, pos_current) {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                for &idx in &filtered_indices[lo..=hi] {
                    selected.insert(idx);
                }
            } else {
                selected.clear();
                selected.insert(actual_idx);
            }
        } else {
            selected.clear();
            selected.insert(actual_idx);
        }
    } else {
        // Plain click: exclusive select
        selected.clear();
        selected.insert(actual_idx);
    }
    *last_clicked = Some(actual_idx);
}
