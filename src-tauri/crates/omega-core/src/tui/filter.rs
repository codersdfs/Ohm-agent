use std::marker::PhantomData;
#[derive(Debug, Clone)]
pub struct FilteredList<T> {
    /// Indices into the source items, after filtering and ranking.
    pub filtered: Vec<usize>,
    /// Currently selected index within `filtered`.
    pub selected: usize,
    /// Scroll offset for viewport management (first visible item in `filtered`).
    pub scroll: usize,
    /// Preferred item to select after recompute (if still in filtered set).
    preferred: Option<usize>,
    _phantom: PhantomData<T>,
}

impl<T> FilteredList<T> {
    /// Create a new empty `FilteredList`.
    pub fn new() -> Self {
        Self {
            filtered: Vec::new(),
            selected: 0,
            scroll: 0,
            preferred: None,
            _phantom: PhantomData,
        }
    }

    /// Set the preferred item index. After `recompute`, if this item is still
    /// in the filtered set, it will be selected.
    pub fn set_preferred(&mut self, idx: Option<usize>) {
        self.preferred = idx;
    }

    /// Recompute the filtered list based on the query and ranking function.
    ///
    /// # Arguments
    ///
    /// - `items`: The source items to filter.
    /// - `query`: The search query (already trimmed by caller if needed).
    /// - `rank`: A closure that returns `Some(score)` if the item matches,
    ///   or `None` if it doesn't. Higher scores rank higher.
    pub fn recompute(&mut self, items: &[T], query: &str, rank: impl Fn(&T, &str) -> Option<i32>) {
        let q = query.trim();

        let mut ranked: Vec<(usize, i32)> = items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                let score = rank(item, q)?;
                Some((idx, score))
            })
            .collect();

        // Sort by score descending, then by original index for stability.
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        self.filtered = ranked.into_iter().map(|(i, _)| i).collect();

        if self.filtered.is_empty() {
            self.selected = 0;
            self.scroll = 0;
        } else {
            // Prefer the preferred item if it's still in the filtered set.
            if let Some(pref) = self.preferred {
                if let Some(pos) = self.filtered.iter().position(|&i| i == pref) {
                    self.selected = pos;
                } else {
                    self.selected = self.selected.min(self.filtered.len() - 1);
                }
            } else {
                self.selected = self.selected.min(self.filtered.len() - 1);
            }
            self.ensure_visible(1);
        }
    }

    /// Move the selection by `delta` (clamped, no wrap-around).
    ///
    /// `visible_count` is the number of items visible in the viewport,
    /// used to manage scroll position.
    pub fn move_selection(&mut self, delta: isize, visible_count: usize) {
        let n = self.filtered.len();
        if n == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }

        let cur = self.selected as isize;
        let next = (cur + delta).clamp(0, (n as isize) - 1) as usize;
        self.selected = next;
        self.ensure_visible(visible_count);
    }

    /// Move the selection by `delta` with circular wrap-around.
    ///
    /// `visible_count` is the number of items visible in the viewport,
    /// used to manage scroll position.
    pub fn move_selection_circular(&mut self, delta: isize, visible_count: usize) {
        let n = self.filtered.len();
        if n == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }

        let cur = self.selected as isize;
        let next = (cur + delta).rem_euclid(n as isize) as usize;
        self.selected = next;
        self.ensure_visible(visible_count);
    }

    /// Ensure the selected item is visible in the viewport.
    pub fn ensure_visible(&mut self, visible_count: usize) {
        if self.filtered.is_empty() || visible_count == 0 {
            return;
        }

        let vc = visible_count.min(self.filtered.len());

        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + vc {
            self.scroll = self.selected + 1 - vc;
        }
    }

    /// Returns the source index of the currently selected item, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.filtered.get(self.selected).copied()
    }

    /// Returns `true` if the filtered list is empty.
    pub fn is_empty(&self) -> bool {
        self.filtered.is_empty()
    }

    /// Returns the number of items in the filtered list.
    pub fn len(&self) -> usize {
        self.filtered.len()
    }
}

impl<T> Default for FilteredList<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn rank_contains(item: &String, query: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }
        let lower = item.to_lowercase();
        if lower.contains(query) {
            Some(1)
        } else {
            None
        }
    }

    fn rank_ranked(item: &String, query: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }
        let lower = item.to_lowercase();
        if !lower.contains(query) {
            return None;
        }
        let mut score = 0i32;
        if lower == query {
            score += 500;
        } else if lower.starts_with(query) {
            score += 200;
        } else {
            score += 50;
        }
        score -= (lower.len() as i32) / 50;
        Some(score)
    }

    #[test]
    fn empty_query_returns_all() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        assert_eq!(list.filtered, vec![0, 1, 2]);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn filter_substring() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "an", rank_contains);
        assert_eq!(list.filtered, vec![1]); // "banana"
    }

    #[test]
    fn no_match_returns_empty() {
        let items = vec!["apple".to_string(), "banana".to_string()];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "zzz", rank_contains);
        assert!(list.filtered.is_empty());
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn ranked_scoring_orders_by_score() {
        let items = vec![
            "gpt-4o-mini".to_string(),
            "gpt-4".to_string(),
            "gpt-4-turbo".to_string(),
        ];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "gpt-4", rank_ranked);
        // "gpt-4" should rank highest (exact match, score 500),
        // then "gpt-4o-mini" (prefix, score 200, shorter) and "gpt-4-turbo" (prefix, score 200, longer)
        // Both have same score, so stable sort keeps original order: 0 before 2
        assert_eq!(list.filtered, vec![1, 0, 2]);
    }

    #[test]
    fn selection_clamps_when_filter_shrinks() {
        let items = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.selected = 3; // last item
        list.recompute(&items, "a", rank_contains);
        assert_eq!(list.filtered, vec![0]);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn preferred_item_selected_if_in_filtered() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.selected = 0; // "apple"
        list.recompute(&items, "an", rank_contains);
        // "banana" (index 1) is in filtered set, but preferred is None
        assert_eq!(list.filtered, vec![1]);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn preferred_item_selected_after_recompute() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.selected = 0; // "apple"
        list.set_preferred(Some(1)); // prefer "banana"
        list.recompute(&items, "an", rank_contains);
        assert_eq!(list.filtered, vec![1]);
        assert_eq!(list.selected, 0); // "banana" is at position 0 in filtered
    }

    #[test]
    fn move_selection_clamped() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.move_selection(5, 10); // try to go past end
        assert_eq!(list.selected, 2);
        list.move_selection(-10, 10); // try to go before start
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn move_selection_circular_wraps() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.selected = 2; // last
        list.move_selection_circular(1, 10); // wraps to 0
        assert_eq!(list.selected, 0);
        list.move_selection_circular(-1, 10); // wraps to 2
        assert_eq!(list.selected, 2);
    }

    #[test]
    fn ensure_visible_scroll_down() {
        let items: Vec<String> = (0..20).map(|i| format!("item{}", i)).collect();
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.scroll = 0;
        list.selected = 15;
        list.ensure_visible(10);
        assert_eq!(list.scroll, 6); // 15 + 1 - 10 = 6
    }

    #[test]
    fn ensure_visible_scroll_up() {
        let items: Vec<String> = (0..20).map(|i| format!("item{}", i)).collect();
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.scroll = 10;
        list.selected = 5;
        list.ensure_visible(10);
        assert_eq!(list.scroll, 5);
    }

    #[test]
    fn selected_index_returns_source_index() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        list.selected = 1;
        assert_eq!(list.selected_index(), Some(1));
    }

    #[test]
    fn selected_index_empty_returns_none() {
        let items: Vec<String> = vec![];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        assert_eq!(list.selected_index(), None);
    }

    #[test]
    fn len_and_is_empty() {
        let items = vec!["a".to_string(), "b".to_string()];
        let mut list = FilteredList::<String>::new();
        list.recompute(&items, "", rank_contains);
        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());

        list.recompute(&items, "zzz", rank_contains);
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
    }
}
