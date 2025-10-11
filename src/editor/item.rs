use std::{collections::HashSet, vec};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use indexmap::IndexMap;

use qualia::{CheckpointId, ObjectShapeWithId, Queryable, Store};
use tui::{
    layout::Constraint,
    style::{Color, Style},
    text::{Span, Spans},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{types::Item, utils::add_item};
use crate::{types::ItemSize, AHResult};

use super::sheet::Row;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemColumnWidth {
    Expand,
    Shrink,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemColumnKind {
    Choice,
    FullText,
}

pub struct ItemColumn {
    pub header: String,
    pub width: ItemColumnWidth,
    pub kind: ItemColumnKind,
    pub display: fn(&Item) -> AHResult<String>,
    pub insert_char: Option<fn(&mut Item, usize, char) -> usize>,
    pub delete_char: Option<fn(&mut Item, usize)>,
    pub searchable: bool,
}

fn render_item_columns(columns: &Vec<ItemColumn>, item: &Item) -> (Vec<String>, Vec<usize>) {
    columns
        .iter()
        .enumerate()
        .map(|(_, c)| {
            let content = (c.display)(item).unwrap_or("".into());
            let width = content.graphemes(true).count();
            (content, width)
        })
        .unzip()
}

fn item_name_from_search(search: &Option<String>) -> String {
    if let Some(search) = search {
        let (word_indices, words): (Vec<_>, Vec<_>) = search.split_word_bound_indices().unzip();
        let mut item_name_parts = Vec::new();
        item_name_parts.push(search[0..word_indices[0]].to_string());

        for (i, word) in words.iter().enumerate() {
            item_name_parts.push(word[0..1].to_ascii_uppercase().to_string());
            item_name_parts.push(word[1..].to_string());

            let next_word_start = if i == words.len() - 1 {
                search.len()
            } else {
                word_indices[i + 1]
            };

            item_name_parts.push(search[word_indices[i] + word.len()..next_word_start].to_string());
        }

        item_name_parts.join("")
    } else {
        "".to_string()
    }
}

#[derive(Debug)]
struct ItemRenderEntry<C> {
    item: Item,
    contents: C,
    column_widths: Vec<usize>,
}

impl<T> std::cmp::PartialEq for ItemRenderEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
    }
}
impl<T> std::cmp::Eq for ItemRenderEntry<T> {}

impl<C> std::cmp::Ord for ItemRenderEntry<C> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.item.location.name, self.item.bin_no, &self.item.name).cmp(&(
            &other.item.location.name,
            other.item.bin_no,
            &other.item.name,
        ))
    }
}
impl<C> std::cmp::PartialOrd for ItemRenderEntry<C> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct ItemColumnRenderedSet<'columns, 'row> {
    columns: &'columns Vec<ItemColumn>,
    checkpoint: CheckpointId,
    entries: IndexMap<i64, ItemRenderEntry<Row<'row>>>,
    search: Option<String>,
}

impl<'columns, 'row> ItemColumnRenderedSet<'columns, 'row> {
    fn new(columns: &'columns Vec<ItemColumn>) -> Self {
        Self {
            columns,
            checkpoint: 0,
            entries: IndexMap::new(),
            search: None,
        }
    }

    fn regenerate_if_needed(
        &mut self,
        last_fetched_items: &IndexMap<i64, Item>,
        last_updated_checkpoint: CheckpointId,
        search: Option<String>,
    ) {
        if search == self.search && last_updated_checkpoint == self.checkpoint {
            return;
        }

        let non_empty_search = search
            .as_ref()
            .and_then(|s| if s.is_empty() { None } else { Some(s) });

        let mut all_entries: IndexMap<i64, ItemRenderEntry<_>> = last_fetched_items
            .iter()
            .map(|(id, item)| {
                let (column_contents, column_widths) = render_item_columns(self.columns, item);

                (
                    *id,
                    ItemRenderEntry {
                        item: item.clone(),
                        contents: column_contents,
                        column_widths,
                    },
                )
            })
            .collect();

        all_entries.sort_by(|_, a, _, b| a.cmp(b));

        let (mut filtered_entries, mut unused_entries): (IndexMap<_, _>, IndexMap<_, _>) =
            if let Some(search) = non_empty_search {
                let matcher = SkimMatcherV2::default();

                let mut unused_entries = IndexMap::new();

                let mut scored_result: Vec<_> = all_entries
                    .into_iter()
                    .filter_map(|(object_id, e)| {
                        let column_results: Vec<_> = e
                            .contents
                            .iter()
                            .enumerate()
                            .map(|(i, c)| {
                                if !self.columns[i].searchable {
                                    return (c, 0, vec![]);
                                }

                                match matcher.fuzzy_indices(&c, search) {
                                    None => (c, 0, vec![]),
                                    Some((score, indices)) => (c, score, indices),
                                }
                            })
                            .collect();

                        let total_score: i64 =
                            column_results.iter().map(|(_, score, _)| score).sum();

                        if total_score == 0 {
                            unused_entries.insert(object_id, e);
                            return None;
                        }

                        Some((
                            total_score,
                            object_id,
                            ItemRenderEntry {
                                contents: Row::new(column_results.into_iter().map(
                                    |(c, _, indices)| {
                                        let mut spans: Vec<_> =
                                            c.chars().map(|c| Span::raw(c.to_string())).collect();

                                        for idx in &indices {
                                            spans[*idx] = Span::styled(
                                                spans[*idx].content.clone(),
                                                Style::default().bg(Color::Indexed(58)),
                                            );
                                        }

                                        Spans::from(spans)
                                    },
                                )),
                                item: e.item,
                                column_widths: e.column_widths,
                            },
                        ))
                    })
                    .collect();

                scored_result.sort_by_key(|(score, _, _)| -score);

                (
                    scored_result
                        .into_iter()
                        .map(|(_, object_id, i)| (object_id, i))
                        .collect(),
                    unused_entries,
                )
            } else {
                (
                    all_entries
                        .into_iter()
                        .map(|(object_id, e)| {
                            (
                                object_id,
                                ItemRenderEntry {
                                    contents: Row::new(e.contents),
                                    item: e.item,
                                    column_widths: e.column_widths,
                                },
                            )
                        })
                        .collect(),
                    IndexMap::new(),
                )
            };

        let reordered_entries = if self.entries.is_empty() {
            filtered_entries
        } else {
            // First, build the list of new items using the order of the old items.
            // This brings in modifications (by pulling from the new set of items) and
            // deletions (where item.remove()) will return None.
            let mut reordered_entries: IndexMap<_, _> = self
                .entries
                .keys()
                .filter_map(|id| {
                    filtered_entries
                        .shift_remove(id)
                        .map(|e| (*id, e))
                        .or_else(|| {
                            if search == self.search {
                                unused_entries.shift_remove(id).map(|e| {
                                    (
                                        *id,
                                        ItemRenderEntry {
                                            contents: Row::new(e.contents),
                                            item: e.item,
                                            column_widths: e.column_widths,
                                        },
                                    )
                                })
                            } else {
                                None
                            }
                        })
                })
                .collect();

            // All that remains in `filtered_entries` is new items.
            for (object_id, entry) in filtered_entries.into_iter() {
                let insert_pos = reordered_entries
                    .values()
                    .collect::<Vec<_>>()
                    .binary_search(&&entry)
                    .map_or_else(|e| e, |o| o);
                reordered_entries.insert(object_id, entry);
                reordered_entries.move_index(reordered_entries.len() - 1, insert_pos);
            }

            reordered_entries
        };

        self.checkpoint = last_updated_checkpoint;
        self.entries = reordered_entries;
        self.search = search;
    }

    fn max_column_width(&self, column: usize) -> usize {
        std::iter::once(self.columns[column].header.len())
            .chain(self.entries.iter().map(|(_, r)| r.column_widths[column]))
            .max()
            .unwrap()
    }

    fn add_item(&mut self, after_index: usize, item: &Item) {
        let (column_contents, column_widths) = render_item_columns(self.columns, item);

        let (inserted_index, _) = self.entries.insert_full(
            item.get_object_id().unwrap(),
            ItemRenderEntry {
                item: item.clone(),
                contents: Row::new(column_contents),
                column_widths,
            },
        );

        self.entries.move_index(inserted_index, after_index + 1);
    }

    fn edit_item<T>(&mut self, index: usize, editor: impl FnOnce(&mut Item) -> T) -> (i64, T) {
        let (object_id, entry) = self.entries.get_index_mut(index).unwrap();

        let value = editor(&mut entry.item);
        let (column_contents, column_widths) = render_item_columns(self.columns, &entry.item);
        entry.column_widths = column_widths;
        entry.contents = Row::new(column_contents);

        (*object_id, value)
    }
}

pub struct ItemColumnViewModel<'columns, 'row> {
    store: Store,
    last_fetched_items: IndexMap<i64, Item>,
    columns: &'columns Vec<ItemColumn>,
    last_updated_checkpoint: CheckpointId,
    last_rendered_set: ItemColumnRenderedSet<'columns, 'row>,
    edited_items: HashSet<i64>,
}

impl<'columns, 'row> ItemColumnViewModel<'columns, 'row> {
    pub fn new(store: Store, columns: &'columns Vec<ItemColumn>) -> Self {
        Self {
            store,
            columns,
            last_fetched_items: IndexMap::new(),
            last_updated_checkpoint: 0,
            last_rendered_set: ItemColumnRenderedSet::new(&columns),
            edited_items: HashSet::new(),
        }
    }

    pub fn refresh(&mut self) -> AHResult<()> {
        self.last_updated_checkpoint = self.store.last_checkpoint_id()?;

        self.last_fetched_items = self
            .store
            .query(Item::q())
            .iter_converted::<Item>(&self.store)?
            .map(|i| (i.get_object_id().unwrap(), i))
            .collect();

        Ok(())
    }

    fn refresh_if_needed(&mut self) -> AHResult<bool> {
        if self.store.modified_since(self.last_updated_checkpoint)? {
            self.refresh()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn render(
        &mut self,
        search: &Option<String>,
    ) -> AHResult<(Vec<String>, Vec<Constraint>, Vec<&Row<'_>>)> {
        self.refresh_if_needed()?;
        self.last_rendered_set.regenerate_if_needed(
            &self.last_fetched_items,
            self.last_updated_checkpoint,
            search.clone(),
        );

        Ok((
            self.columns.iter().map(|c| c.header.clone()).collect(),
            self.columns
                .iter()
                .enumerate()
                .map(|(i, c)| match c.width {
                    ItemColumnWidth::Shrink => {
                        Constraint::Length(self.last_rendered_set.max_column_width(i) as u16)
                    }
                    ItemColumnWidth::Expand => {
                        Constraint::Min(self.last_rendered_set.max_column_width(i) as u16)
                    }
                })
                .collect::<Vec<_>>(),
            self.last_rendered_set
                .entries
                .values()
                .map(|e| &e.contents)
                .collect(),
        ))
    }

    pub fn rightmost_column_index(&self) -> usize {
        self.columns.len() - 1
    }

    pub fn column_index_saturating_add(&self, column_index: usize, offset: isize) -> usize {
        column_index
            .saturating_add_signed(offset)
            .min(self.rightmost_column_index())
    }

    pub fn column_allows_char_selection(&self, column_index: usize) -> bool {
        self.columns[column_index].kind == ItemColumnKind::FullText
    }

    pub fn get_column_len(&self, row_index: usize, column_index: usize) -> Option<usize> {
        if !self.column_allows_char_selection(column_index) {
            return None;
        }

        let (_, ItemRenderEntry { column_widths, .. }) =
            self.last_rendered_set.entries.get_index(row_index).unwrap();

        Some(column_widths[column_index])
    }

    pub fn insert_item(&mut self, after_index: usize, search: &Option<String>) -> AHResult<()> {
        let (after_object_id, _) = self
            .last_rendered_set
            .entries
            .get_index(after_index)
            .unwrap();
        let after_item: Item = self
            .store
            .query(Item::q().id(*after_object_id))
            .one_converted(&self.store)
            .unwrap();
        let last_location = after_item.location.clone();

        let item_name = item_name_from_search(search);

        let item = add_item(
            &mut self.store,
            item_name,
            &last_location,
            None,
            ItemSize::M,
        )?;

        self.last_rendered_set.add_item(after_index, &item);

        Ok(())
    }

    pub fn delete_item(&mut self, row_index: usize) -> AHResult<String> {
        let (object_id, ItemRenderEntry { item, .. }) =
            self.last_rendered_set.entries.get_index(row_index).unwrap();

        let checkpoint = self.store.checkpoint()?;
        checkpoint.query(Item::q().id(*object_id)).delete()?;
        checkpoint.commit(format!("delete item: {}", item.name))?;

        Ok(item.name.clone())
    }

    pub fn insert_char(&mut self, row: usize, cell: usize, i: usize, c: char) -> usize {
        let column_insert_char = match self.columns[cell].insert_char {
            Some(f) => f,
            None => return i,
        };

        let (object_id, new_cursor) = self
            .last_rendered_set
            .edit_item(row, |item| column_insert_char(item, i, c));

        self.edited_items.insert(object_id);

        new_cursor
    }

    pub fn delete_char(&mut self, row: usize, cell: usize, i: usize) {
        let column_delete_char = match self.columns[cell].delete_char {
            Some(f) => f,
            None => return,
        };

        let (object_id, _) = self
            .last_rendered_set
            .edit_item(row, |item| column_delete_char(item, i));

        self.edited_items.insert(object_id);
    }

    pub fn persist_pending_edits(&mut self) -> AHResult<usize> {
        if self.edited_items.len() == 0 {
            return Ok(0);
        }

        for object_id in self.edited_items.iter() {
            let edited_item = self.last_rendered_set.entries[object_id].item.clone();
            let edited_item_name = edited_item.name.clone();
            let checkpoint = self.store.checkpoint()?;
            checkpoint
                .query(Item::q().id(*object_id))
                .set(edited_item.into())?;
            checkpoint.commit(format!("update item: {}", edited_item_name))?;
        }

        let updated = self.edited_items.len();
        self.edited_items.clear();

        Ok(updated)
    }

    pub fn persist_current_pending_edit(&mut self, row: usize) -> AHResult<Option<String>> {
        if self.edited_items.len() == 0 {
            return Ok(None);
        }

        let (object_id, entry) = self.last_rendered_set.entries.get_index(row).unwrap();

        if let Some(_) = self.edited_items.take(object_id) {
            let edited_item = entry.item.clone();
            let edited_item_name = edited_item.name.clone();
            let checkpoint = self.store.checkpoint()?;
            checkpoint
                .query(Item::q().id(*object_id))
                .set(edited_item.into())?;
            checkpoint.commit(format!("update item: {}", edited_item_name))?;

            Ok(Some(edited_item_name))
        } else {
            Ok(None)
        }
    }

    pub fn undo(&mut self) -> AHResult<Option<String>> {
        self.persist_pending_edits()?;

        let description = self.store.undo()?;

        self.last_updated_checkpoint = 0;

        Ok(description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_name_returns_empty_for_none() {
        assert_eq!(item_name_from_search(&None), "".to_string());
    }

    #[test]
    fn item_name_uppercases_leading_letters() {
        assert_eq!(
            item_name_from_search(&Some("abc".to_string())),
            "Abc".to_string()
        );
        assert_eq!(
            item_name_from_search(&Some("abc def".to_string())),
            "Abc Def".to_string()
        );
    }
}
