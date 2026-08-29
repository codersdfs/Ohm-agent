//! Command palette state (P5 split from command_palette.rs).

use super::dispatch::rank_command;
use super::{CommandEntry, COMMANDS};
use crate::tui::filter::FilteredList;

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    pub filtered: Vec<usize>,
    /// Shared filter logic (kept in sync with `filtered` and `selected`).
    filter_list: FilteredList<CommandEntry>,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        let mut s = Self {
            visible: false,
            query: String::new(),
            selected: 0,
            filtered: Vec::new(),
            filter_list: FilteredList::new(),
        };
        s.recompute_filter();
        s
    }

    /// Open palette, optionally seeding the search query (e.g. `"/"`).
    pub fn open(&mut self, seed_query: &str) {
        self.visible = true;
        self.query = seed_query.to_string();
        self.selected = 0;
        self.recompute_filter();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.selected = 0;
        self.recompute_filter();
    }

    pub fn recompute_filter(&mut self) {
        // Delegate to the shared FilteredList, then sync our public fields.
        self.filter_list
            .recompute(COMMANDS, &self.query, rank_command);
        self.filtered = self.filter_list.filtered.clone();
        self.selected = self.filter_list.selected;
    }

    pub(crate) fn move_sel(&mut self, delta: isize) {
        // Sync filter_list state from our public fields, then delegate.
        self.filter_list.selected = self.selected;
        self.filter_list.scroll = 0; // CommandPalette doesn't use scroll
        self.filter_list.move_selection_circular(delta, 10);
        self.selected = self.filter_list.selected;
    }

    pub fn selected_id(&self) -> Option<&'static str> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| COMMANDS.get(i))
            .map(|e| e.id)
    }
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::command_palette::CommandEntry;

    #[test]
    fn open_seeds_query_and_filters() {
        let mut s = CommandPaletteState::new();
        s.open("/");
        assert!(s.visible);
        assert_eq!(s.query, "/");
        assert!(!s.filtered.is_empty());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn selection_clamps_when_filter_shrinks() {
        let mut s = CommandPaletteState::new();
        s.open("");
        s.selected = 6; // last of 7
        s.query = "cle".into();
        s.recompute_filter();
        assert_eq!(s.filtered.len(), 1);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn selected_id_returns_canonical() {
        let mut s = CommandPaletteState::new();
        s.open("");
        s.selected = 1;
        assert_eq!(s.selected_id(), Some("/clear"));
    }

    #[test]
    fn close_resets() {
        let mut s = CommandPaletteState::new();
        s.open("/gate");
        assert!(s.visible);
        s.close();
        assert!(!s.visible);
        assert!(s.query.is_empty());
    }

    // Silence unused-import lint for CommandEntry in this module's scope.
    #[allow(dead_code)]
    fn _type_check(_e: &CommandEntry) {}
}
