use dioxus::prelude::*;

use crate::platform;

pub(crate) const CONTEXT_MENU_EXIT_MS: u64 = 120;

pub(crate) struct ContextMenuTransition<T: 'static> {
    pub(crate) menu: Signal<Option<T>>,
    closing: Signal<bool>,
    generation: Signal<u64>,
}

impl<T: 'static> Copy for ContextMenuTransition<T> {}

impl<T: 'static> Clone for ContextMenuTransition<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> PartialEq for ContextMenuTransition<T> {
    fn eq(&self, other: &Self) -> bool {
        self.menu == other.menu
            && self.closing == other.closing
            && self.generation == other.generation
    }
}

impl<T: 'static> ContextMenuTransition<T> {
    pub(crate) const fn new(
        menu: Signal<Option<T>>,
        closing: Signal<bool>,
        generation: Signal<u64>,
    ) -> Self {
        Self {
            menu,
            closing,
            generation,
        }
    }

    pub(crate) fn show(self, value: T) {
        show_context_menu(self.menu, self.closing, self.generation, value);
    }

    pub(crate) fn dismiss(self) {
        dismiss_context_menu(self.menu, self.closing, self.generation);
    }

    pub(crate) fn is_closing(self) -> bool {
        *self.closing.read()
    }

    pub(crate) fn value(self) -> Option<T>
    where
        T: Clone,
    {
        self.menu.read().clone()
    }
}

pub(crate) fn show_context_menu<T: 'static>(
    mut menu: Signal<Option<T>>,
    mut closing: Signal<bool>,
    mut generation: Signal<u64>,
    value: T,
) {
    let next_generation = (*generation.peek()).wrapping_add(1);
    generation.set(next_generation);
    closing.set(false);
    menu.set(Some(value));
}

pub(crate) fn dismiss_context_menu<T: 'static>(
    mut menu: Signal<Option<T>>,
    mut closing: Signal<bool>,
    mut generation: Signal<u64>,
) {
    if menu.peek().is_none() || *closing.peek() {
        return;
    }

    let close_generation = (*generation.peek()).wrapping_add(1);
    generation.set(close_generation);
    closing.set(true);
    spawn(async move {
        platform::sleep_ms(CONTEXT_MENU_EXIT_MS).await;
        if *generation.peek() == close_generation {
            menu.set(None);
            closing.set(false);
        }
    });
}
