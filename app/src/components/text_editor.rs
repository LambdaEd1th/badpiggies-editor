use dioxus::prelude::*;
use dioxus_free_icons::Icon;
use dioxus_free_icons::icons::ld_icons::LdSearch;
use dioxus_html::input_data::MouseButton;
use ropey::Rope;

use badpiggies_editor_core::worker_protocol::{WorkerRequest, WorkerResponse};

use super::context_menu_transition::ContextMenuTransition;
use crate::editor_state::EditorState;
use crate::platform::processing;

const ROW_HEIGHT: usize = 21;
const OVERSCAN_ROWS: usize = 24;
const MAX_VIEWPORT_ROWS: usize = 256;
const MAX_SCROLL_HEIGHT: usize = 16_000_000;
const DEFAULT_VIEWPORT_HEIGHT: usize = 700;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextBuffer {
    rope: Rope,
}

impl TextBuffer {
    fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    fn materialize(&self) -> String {
        self.rope.to_string()
    }

    fn line_text(&self, line_index: usize) -> Option<String> {
        if line_index >= self.line_count() {
            return None;
        }
        let mut text = self.rope.line(line_index).to_string();
        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }
        Some(text)
    }

    fn line_start_byte(&self, line_index: usize) -> Option<usize> {
        (line_index < self.line_count()).then(|| self.rope.line_to_byte(line_index))
    }

    fn byte_to_line(&self, byte_offset: usize) -> usize {
        self.rope
            .byte_to_line(byte_offset.min(self.rope.len_bytes()))
    }

    fn byte_slice(&self, start: usize, end: usize) -> Option<String> {
        if start > end || end > self.rope.len_bytes() {
            return None;
        }
        Some(
            self.rope
                .slice(self.rope.byte_to_char(start)..self.rope.byte_to_char(end))
                .to_string(),
        )
    }

    fn replace_byte_range(&self, start: usize, end: usize, replacement: &str) -> Option<Self> {
        if start > end || end > self.rope.len_bytes() {
            return None;
        }
        let mut rope = self.rope.clone();
        rope.remove(rope.byte_to_char(start)..rope.byte_to_char(end));
        rope.insert(rope.byte_to_char(start), replacement);
        Some(Self { rope })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextPoint {
    line_index: usize,
    column_utf16: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextSelection {
    start: TextPoint,
    end: TextPoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextReplacement {
    start: TextPoint,
    end: TextPoint,
    replacement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DragSelection {
    anchor: TextPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContextMenu {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy)]
struct VirtualScroll {
    content_height: usize,
    scaled: bool,
    viewport_start_row: usize,
    start_row: usize,
    end_row: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleLine {
    top: i64,
    index: usize,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinePiece {
    Text(String),
    Selected(String),
    Caret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteDirection {
    Backward,
    Forward,
}

#[component]
pub fn RtonTextEditor(
    text: String,
    on_change: EventHandler<String>,
    on_undo: EventHandler<()>,
    on_redo: EventHandler<()>,
) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let buffer = TextBuffer::new(&text);
    let mut scroll_top = use_signal(|| 0_f64);
    let viewport_height = use_signal(|| DEFAULT_VIEWPORT_HEIGHT);
    let mut caret = use_signal(|| TextPoint {
        line_index: 0,
        column_utf16: 0,
    });
    let mut selection = use_signal(|| None::<TextSelection>);
    let drag_selection = use_signal(|| None::<DragSelection>);
    let mut input_sink = use_signal(String::new);
    let mut mounted = use_signal(|| None::<MountedEvent>);
    let context_menu = ContextMenuTransition::new(
        use_signal(|| None::<ContextMenu>),
        use_signal(|| false),
        use_signal(|| 0_u64),
    );
    let mut search_visible = use_signal(|| false);
    let mut search_text = use_signal(String::new);
    let mut replace_text = use_signal(String::new);
    let mut case_sensitive = use_signal(|| false);
    let mut current_match = use_signal(|| 0_usize);
    let document_text = text.clone();
    let search_matches = use_resource(use_reactive!(|(document_text,)| async move {
        let query = search_text.read().clone();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let case_sensitive = *case_sensitive.read();
        match processing::perform(WorkerRequest::SearchText {
            text: document_text,
            query,
            case_sensitive,
        })
        .await?
        {
            WorkerResponse::TextMatches { matches } => Ok(matches
                .into_iter()
                .map(|item| (item.start, item.end))
                .collect()),
            _ => Err("Unexpected text-search response".to_string()),
        }
    }));

    {
        let buffer = buffer.clone();
        use_effect(use_reactive(&buffer, move |buffer| {
            let next_caret = clamp_point(*caret.peek(), &buffer);
            if next_caret != *caret.peek() {
                caret.set(next_caret);
            }
            let current_selection = *selection.peek();
            let next_selection =
                current_selection.and_then(|value| clamp_selection(value, &buffer));
            if next_selection != current_selection {
                selection.set(next_selection);
            }
        }));
    }

    let virtual_scroll = virtual_scroll(
        buffer.line_count(),
        *scroll_top.read(),
        *viewport_height.read(),
    );
    let visible_lines = visible_lines(&buffer, *scroll_top.read(), virtual_scroll);
    let matches = match &*search_matches.read() {
        Some(Ok(matches)) => matches.clone(),
        Some(Err(_)) => find_matches(&buffer, &search_text.read(), *case_sensitive.read()),
        None => Vec::new(),
    };
    let match_index = if matches.is_empty() {
        0
    } else {
        (*current_match.read()).min(matches.len() - 1)
    };
    let selection_snapshot = *selection.read();
    let caret_snapshot = *caret.read();
    let preview_class = if selection_snapshot.is_some() {
        "virtual-text-preview has-logical-selection"
    } else {
        "virtual-text-preview"
    };

    rsx! {
        div { class: "rton-text-editor",
            if !*search_visible.read() {
                button {
                    class: "text-editor-find-trigger",
                    aria_label: t.get("text_find_document"),
                    title: t.get("text_find_document"),
                    onclick: move |_| {
                        search_visible.set(true);
                        focus_find_input();
                    },
                    Icon { width: 15, height: 15, fill: "currentColor", icon: LdSearch }
                }
            }
            div {
                class: preview_class,
                onmousedown: move |_| context_menu.dismiss(),
                div {
                    class: "virtual-text-preview-content",
                    tabindex: "0",
                    style: "--virtual-text-row-height: {ROW_HEIGHT}px",
                    onmounted: move |event| {
                        mounted.set(Some(event.clone()));
                        async move {
                            if let Ok(rect) = event.get_client_rect().await {
                                update_viewport_height(viewport_height, rect.height());
                            }
                        }
                    },
                    onresize: move |event| {
                        if let Ok(size) = event.get_content_box_size() {
                            update_viewport_height(viewport_height, size.height);
                        }
                    },
                    onscroll: move |event| {
                        context_menu.dismiss();
                        scroll_top.set(event.scroll_top());
                    },
                    onmouseup: move |_| finish_drag(drag_selection),
                    onmouseleave: move |_| finish_drag(drag_selection),
                    textarea {
                        id: "rton-virtual-text-input-sink",
                        class: "virtual-text-input-sink",
                        aria_label: t.get("text_editor_input"),
                        spellcheck: "false",
                        autocapitalize: "off",
                        autocomplete: "off",
                        value: "{input_sink}",
                        onkeydown: {
                            let buffer = buffer.clone();
                            move |event| handle_key(
                                event,
                                &buffer,
                                caret,
                                selection,
                                input_sink,
                                search_visible,
                                on_change,
                                on_undo,
                                on_redo,
                            )
                        },
                        oninput: {
                            let buffer = buffer.clone();
                            move |event| {
                                context_menu.dismiss();
                                let replacement = event.value();
                                input_sink.set(String::new());
                                if !replacement.is_empty() {
                                    replace_selection_or_caret(
                                        &buffer,
                                        caret,
                                        selection,
                                        on_change,
                                        replacement,
                                    );
                                }
                            }
                        },
                        oncopy: {
                            let buffer = buffer.clone();
                            move |event| {
                                event.prevent_default();
                                copy_selection(&buffer, *selection.read());
                            }
                        },
                        oncut: {
                            let buffer = buffer.clone();
                            move |event| {
                                event.prevent_default();
                                cut_selection(&buffer, caret, selection, on_change);
                            }
                        },
                        onpaste: {
                            let buffer = buffer.clone();
                            move |event| {
                                if let Some(pasted) = clipboard_event_text(&event) {
                                    event.prevent_default();
                                    replace_selection_or_caret(
                                        &buffer,
                                        caret,
                                        selection,
                                        on_change,
                                        pasted,
                                    );
                                } else {
                                    input_sink.set(String::new());
                                }
                            }
                        },
                    }
                    div {
                        class: "virtual-text-virtual-space",
                        style: "height: {virtual_scroll.content_height}px",
                        for line in visible_lines {
                            div {
                                class: "virtual-text-virtual-row",
                                style: "transform: translateY({line.top}px); height: {ROW_HEIGHT}px",
                                span {
                                    class: "virtual-text-line-number",
                                    "data-line-number": "{line.index + 1}",
                                    aria_hidden: "true",
                                }
                                TextLine {
                                    key: "{line.index}-line",
                                    line_index: line.index,
                                    line_text: line.text,
                                    caret: caret_snapshot,
                                    selection: selection_snapshot,
                                    on_drag_start: move |(point, extend)| start_drag(
                                        drag_selection,
                                        caret,
                                        selection,
                                        point,
                                        extend,
                                    ),
                                    on_drag_update: move |point| update_drag(
                                        drag_selection,
                                        caret,
                                        selection,
                                        point,
                                    ),
                                    on_context_menu: move |menu| {
                                        let mounted = mounted.peek().clone();
                                        spawn(async move {
                                            context_menu.show(
                                                context_menu_from_client_position(menu, mounted).await,
                                            );
                                        });
                                    },
                                }
                            }
                        }
                    }
                }
                if let Some(menu) = context_menu.value() {
                    button {
                        class: if context_menu.is_closing() { "virtual-text-context-backdrop closing" } else { "virtual-text-context-backdrop" },
                        aria_label: t.get("text_close_context_menu"),
                        onclick: move |_| context_menu.dismiss(),
                    }
                    div {
                        class: if context_menu.is_closing() { "virtual-text-context-menu closing" } else { "virtual-text-context-menu" },
                        role: "menu",
                        style: "left: {menu.x}px; top: {menu.y}px",
                        onmousedown: move |event| {
                            event.prevent_default();
                            event.stop_propagation();
                        },
                        button {
                            role: "menuitem",
                            disabled: selection_snapshot.is_none(),
                            onclick: {
                                let buffer = buffer.clone();
                                move |_| {
                                    copy_selection(&buffer, *selection.read());
                                    context_menu.dismiss();
                                    focus_input();
                                }
                            },
                            {t.get("menu_copy")}
                        }
                        button {
                            role: "menuitem",
                            disabled: selection_snapshot.is_none(),
                            onclick: {
                                let buffer = buffer.clone();
                                move |_| {
                                    cut_selection(&buffer, caret, selection, on_change);
                                    context_menu.dismiss();
                                }
                            },
                            {t.get("menu_cut")}
                        }
                        button {
                            role: "menuitem",
                            onclick: move |_| {
                                paste_from_clipboard();
                                context_menu.dismiss();
                            },
                            {t.get("menu_paste")}
                        }
                        div { class: "virtual-text-context-menu-separator", role: "separator" }
                        button {
                            role: "menuitem",
                            onclick: {
                                let buffer = buffer.clone();
                                move |_| {
                                    select_all(&buffer, caret, selection);
                                    context_menu.dismiss();
                                }
                            },
                            {t.get("menu_select_all")}
                        }
                    }
                }
            }
            if *search_visible.read() {
                div { class: "editor-search-panel",
                    input {
                        id: "rton-editor-find-input",
                        class: "editor-search-field",
                        r#type: "search",
                        placeholder: t.get("text_find"),
                        value: "{search_text}",
                        spellcheck: "false",
                        onmounted: move |_| focus_find_input(),
                        oninput: move |event| {
                            search_text.set(event.value());
                            current_match.set(0);
                        },
                        onkeydown: {
                            let buffer = buffer.clone();
                            let matches = matches.clone();
                            move |event| {
                                event.stop_propagation();
                                if event.key().to_string() == "Enter" {
                                    event.prevent_default();
                                    select_match(
                                        &buffer,
                                        &matches,
                                        if event.modifiers().shift() { -1 } else { 1 },
                                        current_match,
                                        caret,
                                        selection,
                                    );
                                } else if event.key().to_string() == "Escape" {
                                    search_visible.set(false);
                                    focus_input();
                                }
                            }
                        },
                    }
                    button {
                        class: "editor-search-button",
                        disabled: matches.is_empty(),
                        onclick: {
                            let buffer = buffer.clone();
                            let matches = matches.clone();
                            move |_| select_match(
                                &buffer,
                                &matches,
                                -1,
                                current_match,
                                caret,
                                selection,
                            )
                        },
                        {t.get("text_previous")}
                    }
                    button {
                        class: "editor-search-button",
                        disabled: matches.is_empty(),
                        onclick: {
                            let buffer = buffer.clone();
                            let matches = matches.clone();
                            move |_| select_match(
                                &buffer,
                                &matches,
                                1,
                                current_match,
                                caret,
                                selection,
                            )
                        },
                        {t.get("text_next")}
                    }
                    label { class: "editor-search-check",
                        input {
                            r#type: "checkbox",
                            checked: *case_sensitive.read(),
                            onchange: move |event| {
                                case_sensitive.set(event.checked());
                                current_match.set(0);
                            },
                        }
                        span { {t.get("text_case_sensitive")} }
                    }
                    input {
                        class: "editor-search-field editor-replace-field",
                        placeholder: t.get("text_replace"),
                        value: "{replace_text}",
                        spellcheck: "false",
                        oninput: move |event| replace_text.set(event.value()),
                        onkeydown: move |event| event.stop_propagation(),
                    }
                    button {
                        class: "editor-search-button",
                        disabled: matches.is_empty(),
                        onclick: {
                            let buffer = buffer.clone();
                            let matches = matches.clone();
                            move |_| replace_current_match(
                                &buffer,
                                &matches,
                                match_index,
                                &replace_text.read(),
                                caret,
                                selection,
                                on_change,
                            )
                        },
                        {t.get("text_replace")}
                    }
                    button {
                        class: "editor-search-button",
                        disabled: matches.is_empty(),
                        onclick: {
                            let buffer = buffer.clone();
                            let matches = matches.clone();
                            move |_| replace_all_matches(
                                &buffer,
                                &matches,
                                &replace_text.read(),
                                caret,
                                selection,
                                on_change,
                            )
                        },
                        {t.get("text_replace_all")}
                    }
                    span { class: "editor-search-status",
                        if matches.is_empty() { "0 / 0" } else { "{match_index + 1} / {matches.len()}" }
                    }
                    button {
                        class: "editor-search-close",
                        title: t.get("text_close_search"),
                        onclick: move |_| {
                            search_visible.set(false);
                            focus_input();
                        },
                        "×"
                    }
                }
            }
        }
    }
}

async fn context_menu_from_client_position(
    menu: ContextMenu,
    mounted: Option<MountedEvent>,
) -> ContextMenu {
    let Some(event) = mounted else {
        return menu;
    };
    let Ok(rect) = event.get_client_rect().await else {
        return menu;
    };
    context_menu_relative_to_origin(menu, rect.origin.x, rect.origin.y)
}

fn context_menu_relative_to_origin(menu: ContextMenu, origin_x: f64, origin_y: f64) -> ContextMenu {
    ContextMenu {
        x: (menu.x as f64 - origin_x).round().max(0.0) as i32,
        y: (menu.y as f64 - origin_y).round().max(0.0) as i32,
    }
}

#[component]
fn TextLine(
    line_index: usize,
    line_text: String,
    caret: TextPoint,
    selection: Option<TextSelection>,
    on_drag_start: EventHandler<(TextPoint, bool)>,
    on_drag_update: EventHandler<TextPoint>,
    on_context_menu: EventHandler<ContextMenu>,
) -> Element {
    let drag_start_text = line_text.clone();
    let drag_update_text = line_text.clone();
    let drag_enter_text = line_text.clone();
    let pieces = line_pieces(selection, caret, line_index, &line_text);
    let class = if selection.is_none() && caret.line_index == line_index {
        "virtual-text-line-view active-line"
    } else {
        "virtual-text-line-view"
    };
    rsx! {
        span {
            class,
            onmousedown: move |event| {
                if !matches!(event.trigger_button(), None | Some(MouseButton::Primary)) {
                    focus_input();
                    return;
                }
                event.prevent_default();
                focus_input();
                on_drag_start.call((
                    TextPoint {
                        line_index,
                        column_utf16: click_column(event.element_coordinates().x, &drag_start_text),
                    },
                    event.modifiers().shift(),
                ));
            },
            oncontextmenu: move |event| {
                event.prevent_default();
                focus_input();
                let point = event.client_coordinates();
                on_context_menu.call(ContextMenu {
                    x: point.x.round() as i32,
                    y: point.y.round() as i32,
                });
            },
            onmousemove: move |event| on_drag_update.call(TextPoint {
                line_index,
                column_utf16: click_column(event.element_coordinates().x, &drag_update_text),
            }),
            onmouseenter: move |event| on_drag_update.call(TextPoint {
                line_index,
                column_utf16: click_column(event.element_coordinates().x, &drag_enter_text),
            }),
            for (index, piece) in pieces.into_iter().enumerate() {
                match piece {
                    LinePiece::Text(text) => rsx! { span { key: "{index}", "{text}" } },
                    LinePiece::Selected(text) => rsx! { span { key: "{index}", class: "virtual-text-selection-fragment", "{text}" } },
                    LinePiece::Caret => rsx! { span { key: "{index}", class: "virtual-text-caret", aria_hidden: "true" } },
                }
            }
        }
    }
}

fn virtual_scroll(row_count: usize, scroll_top: f64, viewport_height: usize) -> VirtualScroll {
    let logical_height = row_count.saturating_mul(ROW_HEIGHT);
    let content_height = logical_height.min(MAX_SCROLL_HEIGHT);
    let viewport_rows = viewport_height
        .div_ceil(ROW_HEIGHT)
        .clamp(1, MAX_VIEWPORT_ROWS);
    let viewport_content_height = viewport_rows.saturating_mul(ROW_HEIGHT);
    let max_start_row = row_count.saturating_sub(viewport_rows);
    let scaled = logical_height > MAX_SCROLL_HEIGHT;
    let safe_top = if scroll_top.is_finite() && scroll_top > 0.0 {
        scroll_top
    } else {
        0.0
    };
    let viewport_start_row = if scaled {
        let scrollable = content_height
            .saturating_sub(viewport_content_height)
            .max(1);
        ((safe_top / scrollable as f64).clamp(0.0, 1.0) * max_start_row as f64).floor() as usize
    } else {
        ((safe_top / ROW_HEIGHT as f64).floor() as usize).min(max_start_row)
    };
    VirtualScroll {
        content_height,
        scaled,
        viewport_start_row,
        start_row: viewport_start_row.saturating_sub(OVERSCAN_ROWS),
        end_row: viewport_start_row
            .saturating_add(viewport_rows)
            .saturating_add(OVERSCAN_ROWS)
            .min(row_count),
    }
}

fn visible_lines(buffer: &TextBuffer, scroll_top: f64, scroll: VirtualScroll) -> Vec<VisibleLine> {
    (scroll.start_row..scroll.end_row)
        .filter_map(|index| {
            let text = buffer.line_text(index)?;
            let top = if scroll.scaled {
                let display_top = scroll_top.max(0.0).round() as i128;
                let relative = index as i128 - scroll.viewport_start_row as i128;
                (display_top + relative * ROW_HEIGHT as i128).clamp(0, i64::MAX as i128) as i64
            } else {
                index.saturating_mul(ROW_HEIGHT).min(i64::MAX as usize) as i64
            };
            Some(VisibleLine { top, index, text })
        })
        .collect()
}

fn update_viewport_height(mut viewport: Signal<usize>, height: f64) {
    let next = if height.is_finite() && height > 0.0 {
        (height.ceil() as usize).clamp(ROW_HEIGHT, ROW_HEIGHT * MAX_VIEWPORT_ROWS)
    } else {
        DEFAULT_VIEWPORT_HEIGHT
    };
    if next != *viewport.read() {
        viewport.set(next);
    }
}

fn start_drag(
    mut drag: Signal<Option<DragSelection>>,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
    point: TextPoint,
    extend: bool,
) {
    let anchor = if extend {
        selection.read().map_or(*caret.read(), |value| value.start)
    } else {
        point
    };
    drag.set(Some(DragSelection { anchor }));
    caret.set(point);
    selection.set((anchor != point).then_some(TextSelection {
        start: anchor,
        end: point,
    }));
}

fn update_drag(
    drag: Signal<Option<DragSelection>>,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
    point: TextPoint,
) {
    let Some(drag) = *drag.read() else { return };
    caret.set(point);
    selection.set((drag.anchor != point).then_some(TextSelection {
        start: drag.anchor,
        end: point,
    }));
}

fn finish_drag(mut drag: Signal<Option<DragSelection>>) {
    drag.set(None);
}

fn click_column(x: f64, text: &str) -> usize {
    const LEFT_PADDING: f64 = 12.0;
    const CHAR_WIDTH: f64 = 7.8;
    if !x.is_finite() {
        return 0;
    }
    (((x - LEFT_PADDING) / CHAR_WIDTH).round())
        .max(0.0)
        .min(utf16_len(text) as f64) as usize
}

#[allow(clippy::too_many_arguments)]
fn handle_key(
    event: KeyboardEvent,
    buffer: &TextBuffer,
    caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
    mut input_sink: Signal<String>,
    mut search_visible: Signal<bool>,
    on_change: EventHandler<String>,
    on_undo: EventHandler<()>,
    on_redo: EventHandler<()>,
) {
    let key = event.key().to_string();
    let modifiers = event.modifiers();
    let command = modifiers.ctrl() || modifiers.meta();
    let shift = modifiers.shift();
    if command && key.eq_ignore_ascii_case("f") {
        event.prevent_default();
        search_visible.set(true);
        focus_find_input();
        return;
    }
    if command && key.eq_ignore_ascii_case("z") {
        event.prevent_default();
        if shift {
            on_redo.call(())
        } else {
            on_undo.call(())
        }
        return;
    }
    if command && key.eq_ignore_ascii_case("y") {
        event.prevent_default();
        on_redo.call(());
        return;
    }
    if command && key.eq_ignore_ascii_case("a") {
        event.prevent_default();
        select_all(buffer, caret, selection);
        return;
    }
    if command && key.eq_ignore_ascii_case("c") {
        event.prevent_default();
        copy_selection(buffer, *selection.read());
        return;
    }
    if command && key.eq_ignore_ascii_case("x") {
        event.prevent_default();
        cut_selection(buffer, caret, selection, on_change);
        return;
    }
    if command && key.eq_ignore_ascii_case("v") {
        input_sink.set(String::new());
        return;
    }
    if command || modifiers.alt() {
        return;
    }
    match key.as_str() {
        "Backspace" => {
            event.prevent_default();
            delete_selection_or_adjacent(
                buffer,
                caret,
                selection,
                on_change,
                DeleteDirection::Backward,
            );
        }
        "Delete" => {
            event.prevent_default();
            delete_selection_or_adjacent(
                buffer,
                caret,
                selection,
                on_change,
                DeleteDirection::Forward,
            );
        }
        "Enter" => {
            event.prevent_default();
            replace_selection_or_caret(buffer, caret, selection, on_change, "\n".to_string());
        }
        "Tab" => {
            event.prevent_default();
            replace_selection_or_caret(buffer, caret, selection, on_change, "\t".to_string());
        }
        "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" => {
            event.prevent_default();
            move_caret(buffer, caret, selection, &key, shift);
        }
        "Escape" => selection.set(None),
        _ => {}
    }
}

fn replace_selection_or_caret(
    buffer: &TextBuffer,
    caret: Signal<TextPoint>,
    selection: Signal<Option<TextSelection>>,
    on_change: EventHandler<String>,
    replacement: String,
) {
    let current = *selection.read();
    commit_replacement(
        buffer,
        caret,
        selection,
        on_change,
        TextReplacement {
            start: current.map_or(*caret.read(), |value| value.start),
            end: current.map_or(*caret.read(), |value| value.end),
            replacement,
        },
    );
}

fn commit_replacement(
    buffer: &TextBuffer,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
    on_change: EventHandler<String>,
    replacement: TextReplacement,
) {
    let (start, end) = normalize_points(replacement.start, replacement.end);
    let Some(start_byte) = point_to_byte(buffer, start) else {
        return;
    };
    let Some(end_byte) = point_to_byte(buffer, end) else {
        return;
    };
    if start_byte == end_byte && replacement.replacement.is_empty() {
        return;
    }
    let Some(next_buffer) =
        buffer.replace_byte_range(start_byte, end_byte, &replacement.replacement)
    else {
        return;
    };
    let next_caret = caret_after_replacement(start, &replacement.replacement);
    caret.set(clamp_point(next_caret, &next_buffer));
    selection.set(None);
    on_change.call(next_buffer.materialize());
    focus_input();
}

fn caret_after_replacement(start: TextPoint, replacement: &str) -> TextPoint {
    let newline_count = replacement.bytes().filter(|byte| *byte == b'\n').count();
    if newline_count == 0 {
        return TextPoint {
            line_index: start.line_index,
            column_utf16: start.column_utf16.saturating_add(utf16_len(replacement)),
        };
    }
    let tail = replacement.rsplit_once('\n').map_or("", |(_, tail)| tail);
    TextPoint {
        line_index: start.line_index.saturating_add(newline_count),
        column_utf16: utf16_len(tail.strip_suffix('\r').unwrap_or(tail)),
    }
}

fn delete_selection_or_adjacent(
    buffer: &TextBuffer,
    caret: Signal<TextPoint>,
    selection: Signal<Option<TextSelection>>,
    on_change: EventHandler<String>,
    direction: DeleteDirection,
) {
    let (start, end) = if let Some(value) = *selection.read() {
        (value.start, value.end)
    } else {
        let point = clamp_point(*caret.read(), buffer);
        match direction {
            DeleteDirection::Backward => {
                let Some(previous) = previous_point(buffer, point) else {
                    return;
                };
                (previous, point)
            }
            DeleteDirection::Forward => {
                let Some(next) = next_point(buffer, point) else {
                    return;
                };
                (point, next)
            }
        }
    };
    commit_replacement(
        buffer,
        caret,
        selection,
        on_change,
        TextReplacement {
            start,
            end,
            replacement: String::new(),
        },
    );
}

fn move_caret(
    buffer: &TextBuffer,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
    key: &str,
    extend: bool,
) {
    let current = clamp_point(*caret.read(), buffer);
    let current_selection = *selection.read();
    if !extend && let Some(value) = current_selection {
        let (start, end) = normalize_points(value.start, value.end);
        caret.set(if matches!(key, "ArrowLeft" | "ArrowUp" | "Home") {
            start
        } else {
            end
        });
        selection.set(None);
        focus_input();
        return;
    }
    let next = match key {
        "ArrowLeft" => previous_point(buffer, current).unwrap_or(current),
        "ArrowRight" => next_point(buffer, current).unwrap_or(current),
        "ArrowUp" => clamp_point(
            TextPoint {
                line_index: current.line_index.saturating_sub(1),
                column_utf16: current.column_utf16,
            },
            buffer,
        ),
        "ArrowDown" => clamp_point(
            TextPoint {
                line_index: current.line_index.saturating_add(1),
                column_utf16: current.column_utf16,
            },
            buffer,
        ),
        "Home" => TextPoint {
            line_index: current.line_index,
            column_utf16: 0,
        },
        "End" => TextPoint {
            line_index: current.line_index,
            column_utf16: line_utf16_len(buffer, current.line_index),
        },
        _ => current,
    };
    if extend {
        let anchor = selection.read().map_or(current, |value| value.start);
        caret.set(next);
        selection.set((anchor != next).then_some(TextSelection {
            start: anchor,
            end: next,
        }));
    } else {
        caret.set(next);
        selection.set(None);
    }
    focus_input();
}

fn previous_point(buffer: &TextBuffer, point: TextPoint) -> Option<TextPoint> {
    let point = clamp_point(point, buffer);
    if point.column_utf16 > 0 {
        let line = buffer.line_text(point.line_index)?;
        let byte = utf16_column_to_byte(&line, point.column_utf16);
        let previous = line[..byte].chars().next_back()?;
        return Some(TextPoint {
            line_index: point.line_index,
            column_utf16: point.column_utf16.saturating_sub(previous.len_utf16()),
        });
    }
    if point.line_index == 0 {
        return None;
    }
    let line_index = point.line_index - 1;
    Some(TextPoint {
        line_index,
        column_utf16: line_utf16_len(buffer, line_index),
    })
}

fn next_point(buffer: &TextBuffer, point: TextPoint) -> Option<TextPoint> {
    let point = clamp_point(point, buffer);
    let line = buffer.line_text(point.line_index)?;
    let length = utf16_len(&line);
    if point.column_utf16 < length {
        let byte = utf16_column_to_byte(&line, point.column_utf16);
        let next = line[byte..].chars().next()?;
        return Some(TextPoint {
            line_index: point.line_index,
            column_utf16: point.column_utf16.saturating_add(next.len_utf16()),
        });
    }
    (point.line_index + 1 < buffer.line_count()).then_some(TextPoint {
        line_index: point.line_index + 1,
        column_utf16: 0,
    })
}

fn select_all(
    buffer: &TextBuffer,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
) {
    let start = TextPoint {
        line_index: 0,
        column_utf16: 0,
    };
    let end = document_end(buffer);
    caret.set(end);
    selection.set((start != end).then_some(TextSelection { start, end }));
    focus_input();
}

fn copy_selection(buffer: &TextBuffer, selection: Option<TextSelection>) {
    let Some(text) = selected_text(buffer, selection) else {
        return;
    };
    write_clipboard(&text);
}

fn cut_selection(
    buffer: &TextBuffer,
    caret: Signal<TextPoint>,
    selection: Signal<Option<TextSelection>>,
    on_change: EventHandler<String>,
) {
    let Some(value) = *selection.read() else {
        return;
    };
    let Some(text) = selected_text(buffer, Some(value)) else {
        return;
    };
    write_clipboard(&text);
    commit_replacement(
        buffer,
        caret,
        selection,
        on_change,
        TextReplacement {
            start: value.start,
            end: value.end,
            replacement: String::new(),
        },
    );
}

fn selected_text(buffer: &TextBuffer, selection: Option<TextSelection>) -> Option<String> {
    let value = selection?;
    let (start, end) = normalize_points(value.start, value.end);
    let start = point_to_byte(buffer, start)?;
    let end = point_to_byte(buffer, end)?;
    (start < end)
        .then(|| buffer.byte_slice(start, end))
        .flatten()
}

fn line_pieces(
    selection: Option<TextSelection>,
    caret: TextPoint,
    line_index: usize,
    text: &str,
) -> Vec<LinePiece> {
    if let Some((start, end)) = line_selection_columns(selection, line_index, text) {
        return nonempty_pieces([
            LinePiece::Text(text[..start].to_string()),
            LinePiece::Selected(text[start..end].to_string()),
            LinePiece::Text(text[end..].to_string()),
        ]);
    }
    if caret.line_index == line_index {
        let column = utf16_column_to_byte(text, caret.column_utf16);
        return nonempty_pieces([
            LinePiece::Text(text[..column].to_string()),
            LinePiece::Caret,
            LinePiece::Text(text[column..].to_string()),
        ]);
    }
    if text.is_empty() {
        Vec::new()
    } else {
        vec![LinePiece::Text(text.to_string())]
    }
}

fn nonempty_pieces<const N: usize>(pieces: [LinePiece; N]) -> Vec<LinePiece> {
    pieces
        .into_iter()
        .filter(|piece| !matches!(piece, LinePiece::Text(text) | LinePiece::Selected(text) if text.is_empty()))
        .collect()
}

fn line_selection_columns(
    selection: Option<TextSelection>,
    line_index: usize,
    text: &str,
) -> Option<(usize, usize)> {
    let selection = selection?;
    let (start, end) = normalize_points(selection.start, selection.end);
    if line_index < start.line_index || line_index > end.line_index {
        return None;
    }
    let start_byte = if line_index == start.line_index {
        utf16_column_to_byte(text, start.column_utf16)
    } else {
        0
    };
    let end_byte = if line_index == end.line_index {
        utf16_column_to_byte(text, end.column_utf16)
    } else {
        text.len()
    };
    (start_byte < end_byte).then_some((start_byte, end_byte))
}

fn find_matches(buffer: &TextBuffer, query: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let source = buffer.materialize();
    if case_sensitive {
        return source
            .match_indices(query)
            .map(|(start, value)| (start, start + value.len()))
            .collect();
    }
    let lower_source = source.to_ascii_lowercase();
    let lower_query = query.to_ascii_lowercase();
    lower_source
        .match_indices(&lower_query)
        .map(|(start, value)| (start, start + value.len()))
        .filter(|(start, end)| source.is_char_boundary(*start) && source.is_char_boundary(*end))
        .collect()
}

fn select_match(
    buffer: &TextBuffer,
    matches: &[(usize, usize)],
    direction: i32,
    mut current: Signal<usize>,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
) {
    if matches.is_empty() {
        return;
    }
    let current_index = (*current.read()).min(matches.len() - 1);
    let index = if selection.read().is_none() {
        current_index
    } else if direction < 0 {
        current_index.checked_sub(1).unwrap_or(matches.len() - 1)
    } else {
        (current_index + 1) % matches.len()
    };
    current.set(index);
    let (start, end) = matches[index];
    let start = byte_to_point(buffer, start);
    let end = byte_to_point(buffer, end);
    caret.set(end);
    selection.set(Some(TextSelection { start, end }));
    scroll_to_line(start.line_index);
}

fn replace_current_match(
    buffer: &TextBuffer,
    matches: &[(usize, usize)],
    index: usize,
    replacement: &str,
    caret: Signal<TextPoint>,
    selection: Signal<Option<TextSelection>>,
    on_change: EventHandler<String>,
) {
    let Some(&(start, end)) = matches.get(index) else {
        return;
    };
    commit_replacement(
        buffer,
        caret,
        selection,
        on_change,
        TextReplacement {
            start: byte_to_point(buffer, start),
            end: byte_to_point(buffer, end),
            replacement: replacement.to_string(),
        },
    );
}

fn replace_all_matches(
    buffer: &TextBuffer,
    matches: &[(usize, usize)],
    replacement: &str,
    mut caret: Signal<TextPoint>,
    mut selection: Signal<Option<TextSelection>>,
    on_change: EventHandler<String>,
) {
    let mut next = buffer.clone();
    for &(start, end) in matches.iter().rev() {
        let Some(updated) = next.replace_byte_range(start, end, replacement) else {
            return;
        };
        next = updated;
    }
    caret.set(TextPoint {
        line_index: 0,
        column_utf16: 0,
    });
    selection.set(None);
    on_change.call(next.materialize());
}

fn clamp_selection(selection: TextSelection, buffer: &TextBuffer) -> Option<TextSelection> {
    let start = clamp_point(selection.start, buffer);
    let end = clamp_point(selection.end, buffer);
    (start != end).then_some(TextSelection { start, end })
}

fn clamp_point(point: TextPoint, buffer: &TextBuffer) -> TextPoint {
    let line_index = point.line_index.min(buffer.line_count().saturating_sub(1));
    TextPoint {
        line_index,
        column_utf16: point.column_utf16.min(line_utf16_len(buffer, line_index)),
    }
}

fn document_end(buffer: &TextBuffer) -> TextPoint {
    let line_index = buffer.line_count().saturating_sub(1);
    TextPoint {
        line_index,
        column_utf16: line_utf16_len(buffer, line_index),
    }
}

fn line_utf16_len(buffer: &TextBuffer, line_index: usize) -> usize {
    buffer
        .line_text(line_index)
        .map_or(0, |line| utf16_len(&line))
}

fn normalize_points(start: TextPoint, end: TextPoint) -> (TextPoint, TextPoint) {
    if (start.line_index, start.column_utf16) <= (end.line_index, end.column_utf16) {
        (start, end)
    } else {
        (end, start)
    }
}

fn point_to_byte(buffer: &TextBuffer, point: TextPoint) -> Option<usize> {
    let point = clamp_point(point, buffer);
    let line_start = buffer.line_start_byte(point.line_index)?;
    let line = buffer.line_text(point.line_index)?;
    Some(line_start + utf16_column_to_byte(&line, point.column_utf16))
}

fn byte_to_point(buffer: &TextBuffer, byte_offset: usize) -> TextPoint {
    let line_index = buffer.byte_to_line(byte_offset);
    let line_start = buffer.line_start_byte(line_index).unwrap_or_default();
    let line = buffer.line_text(line_index).unwrap_or_default();
    TextPoint {
        line_index,
        column_utf16: byte_column_to_utf16(&line, byte_offset.saturating_sub(line_start)),
    }
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

fn utf16_column_to_byte(text: &str, column: usize) -> usize {
    let mut utf16 = 0_usize;
    for (byte, ch) in text.char_indices() {
        if utf16.saturating_add(ch.len_utf16()) > column {
            return byte;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

fn byte_column_to_utf16(text: &str, column: usize) -> usize {
    text.char_indices()
        .take_while(|(byte, _)| *byte < column.min(text.len()))
        .map(|(_, ch)| ch.len_utf16())
        .sum()
}

fn focus_input() {
    document::eval(
        r#"requestAnimationFrame(() => {
            const input = document.getElementById('rton-virtual-text-input-sink');
            if (!input) return;
            input.value = '';
            input.focus({ preventScroll: true });
            input.setSelectionRange?.(0, 0);
        });"#,
    );
}

fn focus_find_input() {
    document::eval(
        r#"requestAnimationFrame(() => {
            const input = document.getElementById('rton-editor-find-input');
            input?.focus({ preventScroll: true });
            input?.select?.();
        });"#,
    );
}

fn scroll_to_line(line_index: usize) {
    document::eval(&format!(
        "const editor = document.querySelector('.virtual-text-preview-content'); if (editor) editor.scrollTop = Math.max(0, {} - editor.clientHeight / 2);",
        line_index.saturating_mul(ROW_HEIGHT)
    ));
}

fn write_clipboard(text: &str) {
    let Ok(text) = serde_json::to_string(text) else {
        return;
    };
    document::eval(&format!(
        r#"(() => {{
            const text = {text};
            const fallback = () => {{
                const input = document.createElement('textarea');
                input.value = text;
                input.style.position = 'fixed';
                input.style.left = '-10000px';
                document.body.appendChild(input);
                input.select();
                document.execCommand?.('copy');
                input.remove();
            }};
            navigator.clipboard?.writeText ? navigator.clipboard.writeText(text).catch(fallback) : fallback();
        }})();"#
    ));
}

fn paste_from_clipboard() {
    document::eval(
        r#"(() => {
            const input = document.getElementById('rton-virtual-text-input-sink');
            if (!input) return;
            input.focus({ preventScroll: true });
            const dispatch = (text) => {
                if (!text) return;
                input.value = text;
                input.dispatchEvent(new InputEvent('input', { bubbles: true, data: text, inputType: 'insertFromPaste' }));
            };
            navigator.clipboard?.readText?.().then(dispatch).catch(() => document.execCommand?.('paste'));
        })();"#,
    );
}

#[cfg(target_arch = "wasm32")]
fn clipboard_event_text(event: &dioxus_html::ClipboardEvent) -> Option<String> {
    use wasm_bindgen::JsCast;
    let web_event = event.data.downcast::<web_sys::Event>()?;
    let clipboard_event = web_event.dyn_ref::<web_sys::ClipboardEvent>()?;
    clipboard_event
        .clipboard_data()?
        .get_data("text/plain")
        .ok()
        .filter(|text| !text.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_event_text(_event: &dioxus_html::ClipboardEvent) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_scroll_only_materializes_visible_rows() {
        let buffer = TextBuffer::new(
            &(0..10_000)
                .map(|line| format!("{line}\n"))
                .collect::<String>(),
        );
        let scroll = virtual_scroll(buffer.line_count(), 21_000.0, 420);
        let lines = visible_lines(&buffer, 21_000.0, scroll);
        assert!(lines.len() < 100);
        assert!(lines.iter().any(|line| line.index >= 900));
    }

    #[test]
    fn utf16_points_replace_multibyte_text() {
        let buffer = TextBuffer::new("a中𐐷b");
        let start = TextPoint {
            line_index: 0,
            column_utf16: 1,
        };
        let end = TextPoint {
            line_index: 0,
            column_utf16: 4,
        };
        let updated = buffer
            .replace_byte_range(
                point_to_byte(&buffer, start).unwrap(),
                point_to_byte(&buffer, end).unwrap(),
                "文",
            )
            .unwrap();
        assert_eq!(updated.materialize(), "a文b");
    }

    #[test]
    fn case_insensitive_search_preserves_utf8_byte_offsets() {
        let buffer = TextBuffer::new("<String value=\"中Alpha\" />");
        let matches = find_matches(&buffer, "alpha", false);
        assert_eq!(matches.len(), 1);
        assert_eq!(
            buffer.byte_slice(matches[0].0, matches[0].1).as_deref(),
            Some("Alpha")
        );
    }

    #[test]
    fn context_menu_client_position_is_relative_to_editor() {
        let menu = context_menu_relative_to_origin(ContextMenu { x: 380, y: 245 }, 120.0, 45.0);
        assert_eq!(menu, ContextMenu { x: 260, y: 200 });

        let outside = context_menu_relative_to_origin(ContextMenu { x: 80, y: 20 }, 120.0, 45.0);
        assert_eq!(outside, ContextMenu { x: 0, y: 0 });
    }
}
