use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
    vec,
};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use indexmap::IndexMap;
use lazy_static::lazy_static;
use qualia::{CheckpointId, ObjectShapeWithId, Queryable, Store};
use tui::{
    backend::Backend,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear},
    Frame,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{types::Item, utils::add_item};
use crate::{types::ItemSize, AHResult};

use super::sheet::{Row, Sheet, SheetSelection, SheetState};

struct ItemRenderEntry<C> {
    item: Item,
    contents: C,
    column_widths: Vec<usize>,
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
            .map(|(id, o)| {
                let column_contents: Vec<_> = self
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(_, c)| (c.display)(o).unwrap_or("".into()))
                    .collect();
                let column_widths = column_contents
                    .iter()
                    .map(|text| text.graphemes(true).count())
                    .collect::<Vec<_>>();

                (
                    *id,
                    ItemRenderEntry {
                        item: o.clone(),
                        contents: column_contents,
                        column_widths,
                    },
                )
            })
            .collect();

        all_entries.sort_by(|_, a, _, b| {
            (&a.item.location.name, a.item.bin_no, &a.item.name).cmp(&(
                &b.item.location.name,
                b.item.bin_no,
                &b.item.name,
            ))
        });

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
                    filtered_entries.remove(id).map(|e| (*id, e)).or_else(|| {
                        if search == self.search {
                            unused_entries.remove(id).map(|e| {
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
                    .binary_search_by(|e| {
                        (&e.item.location.name, e.item.bin_no, &e.item.name).cmp(&(
                            &entry.item.location.name,
                            entry.item.bin_no,
                            &entry.item.name,
                        ))
                    })
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

    fn add_entry(&mut self, after_index: usize, item: &Item) {
        let column_contents: Vec<_> = self
            .columns
            .iter()
            .enumerate()
            .map(|(_, c)| (c.display)(item).unwrap_or("".into()))
            .collect();

        let column_widths = column_contents
            .iter()
            .map(|text| text.graphemes(true).count())
            .collect::<Vec<_>>();

        let (inserted_index, _) = self.entries.insert_full(
            item.get_object_id().unwrap(),
            ItemRenderEntry {
                item: item.clone(),
                contents: Row::new(column_contents),
                column_widths,
            },
        );
        dbg!(inserted_index, after_index);
        self.entries.move_index(inserted_index, after_index + 1);
    }
}

struct ItemColumnViewModel<'columns, 'row> {
    store: Store,
    last_fetched_items: IndexMap<i64, Item>,
    columns: &'columns Vec<ItemColumn>,
    last_updated_checkpoint: CheckpointId,
    last_rendered_set: ItemColumnRenderedSet<'columns, 'row>,
    search: Option<String>,
    edited_items: IndexMap<i64, Item>,
}

impl<'columns, 'row> ItemColumnViewModel<'columns, 'row> {
    fn new(store: Store, columns: &'columns Vec<ItemColumn>) -> Self {
        Self {
            store,
            columns,
            last_fetched_items: IndexMap::new(),
            search: None,
            last_updated_checkpoint: 0,
            last_rendered_set: ItemColumnRenderedSet::new(&columns),
            edited_items: IndexMap::new(),
        }
    }

    fn refresh(&mut self) -> AHResult<()> {
        self.last_updated_checkpoint = self.store.last_checkpoint_id()?;

        self.last_fetched_items = self
            .store
            .query(Item::q())
            .iter_converted::<Item>(&self.store)?
            .map(|i| (i.get_object_id().unwrap(), i))
            .collect();

        self.last_rendered_set.regenerate_if_needed(
            &self.last_fetched_items,
            self.last_updated_checkpoint,
            self.search.clone(),
        );

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

    fn render(
        &mut self,
        search: &Option<String>,
    ) -> AHResult<(Vec<String>, Vec<Constraint>, Vec<&Row>)> {
        self.search = search.clone();
        self.refresh_if_needed()?;

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

    fn rightmost_column_index(&self) -> usize {
        self.columns.len() - 1
    }

    fn column_index_saturating_add(&self, column_index: usize, offset: isize) -> usize {
        column_index
            .saturating_add_signed(offset)
            .min(self.rightmost_column_index())
    }

    fn column_allows_char_selection(&self, column_index: usize) -> bool {
        self.columns[column_index].kind == ItemColumnKind::FullText
    }

    fn get_column_len(&self, row_index: usize, column_index: usize) -> Option<usize> {
        if !self.column_allows_char_selection(column_index) {
            return None;
        }

        let (_, ItemRenderEntry { column_widths, .. }) =
            self.last_rendered_set.entries.get_index(row_index).unwrap();

        Some(column_widths[column_index])
    }

    fn insert_item(&mut self, after_index: usize, search: &Option<String>) -> AHResult<()> {
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

        let item_name = if let Some(search) = search {
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

                item_name_parts
                    .push(search[word_indices[i] + word.len()..next_word_start].to_string());
            }

            item_name_parts.join("")
        } else {
            "".to_string()
        };

        let item = add_item(
            &mut self.store,
            item_name,
            &last_location,
            None,
            ItemSize::M,
        )?;

        self.last_rendered_set.add_entry(after_index, &item);

        Ok(())
    }

    fn delete_item(&mut self, row_index: usize) -> AHResult<String> {
        let (object_id, ItemRenderEntry { item, .. }) =
            self.last_rendered_set.entries.get_index(row_index).unwrap();

        let checkpoint = self.store.checkpoint()?;
        checkpoint.query(Item::q().id(*object_id)).delete()?;
        checkpoint.commit(format!("delete item: {}", item.name))?;

        Ok(item.name.clone())
    }

    fn insert_char(&mut self, row: usize, cell: usize, i: usize, c: char) -> usize {
        let column_insert_char = match self.columns[cell].insert_char {
            Some(f) => f,
            None => return i,
        };

        let (object_id, ItemRenderEntry { item, .. }) =
            self.last_rendered_set.entries.get_index_mut(row).unwrap();

        self.edited_items
            .entry(*object_id)
            .or_insert_with(|| item.clone());

        column_insert_char(item, i, c)
    }

    fn delete_char(&mut self, row: usize, cell: usize, i: usize) {
        let column_delete_char = match self.columns[cell].delete_char {
            Some(f) => f,
            None => return,
        };

        let (object_id, ItemRenderEntry { item, .. }) =
            self.last_rendered_set.entries.get_index_mut(row).unwrap();

        self.edited_items
            .entry(*object_id)
            .or_insert_with(|| item.clone());

        column_delete_char(item, i)
    }

    fn persist_pending_edits(&mut self) -> bool {
        if self.edited_items.len() == 0 {
            return false;
        }

        for object_id in self.edited_items.keys() {
            let checkpoint = self.store.checkpoint().unwrap();
            checkpoint
                .query(Item::q().id(*object_id))
                .set(self.last_fetched_items[object_id].clone().into())
                .unwrap();
            checkpoint
                .commit(format!(
                    "update item: {}",
                    self.last_fetched_items[object_id].name
                ))
                .unwrap();
        }

        self.edited_items.clear();

        true
    }

    fn undo(&mut self) -> AHResult<Option<String>> {
        self.persist_pending_edits();

        let description = self.store.undo()?;

        self.last_updated_checkpoint = 0;

        Ok(description)
    }
}

pub struct App<'a, 'b> {
    item_column_view_model: ItemColumnViewModel<'a, 'b>,
    running: Arc<AtomicBool>,
    search: Option<String>,
    search_in_progress: bool,
    sheet_state: SheetState,
    last_table_size: Option<Rect>,
    last_action_time: Instant,
    action_description: Option<(Instant, String)>,
    help_shown: bool,
}

lazy_static! {
    static ref ITEM_COLUMNS: Vec<ItemColumn> = vec![
        ItemColumn {
            header: "Location".to_string(),
            width: ItemColumnWidth::Shrink,
            kind: ItemColumnKind::Choice,
            display: |i| Ok(i.format().format_location()),
            insert_char: None,
            delete_char: None,
            searchable: true,
        },
        ItemColumn {
            header: "Size".to_string(),
            width: ItemColumnWidth::Shrink,
            kind: ItemColumnKind::Choice,
            display: |i: &Item| {
                Ok(match i.size.parse()? {
                    ItemSize::S => "Sm",
                    ItemSize::M => "Md",
                    ItemSize::L => "Lg",
                    ItemSize::X => "XL",
                }
                .to_string())
            },
            insert_char: Some(|item, _, c| {
                match c.to_ascii_lowercase() {
                    's' | 'm' | 'l' | 'x' => item.size = c.to_ascii_uppercase().to_string(),
                    _ => {}
                };

                0
            }),
            delete_char: None,
            searchable: false,
        },
        ItemColumn {
            header: "Name".to_string(),
            width: ItemColumnWidth::Expand,
            kind: ItemColumnKind::FullText,
            display: |i| Ok(i.name.clone()),
            insert_char: Some(|item, i, c| {
                item.name.insert(
                    item.name
                        .grapheme_indices(true)
                        .nth(i)
                        .map_or(item.name.len(), |(offset, _)| offset),
                    c,
                );

                item.name
                    .grapheme_indices(true)
                    .nth(i + 1)
                    .map_or(item.name.len(), |(offset, _)| offset)
            }),
            delete_char: Some(|item, i| {
                let (from, to) = {
                    let mut grapheme_indices = item.name.grapheme_indices(true).skip(i);

                    let from = match grapheme_indices.next() {
                        Some((i, _)) => i,
                        None => return,
                    };

                    (from, grapheme_indices.next().map(|(i, _)| i))
                };

                item.name.drain(from..to.unwrap_or(item.name.len()));
            }),
            searchable: true,
        },
    ];
}

impl<'a, 'b> App<'a, 'b> {
    pub fn new(store: Store, running: Arc<AtomicBool>) -> Self {
        let mut sheet_state = SheetState::default();
        sheet_state.select(SheetSelection::Char(0, 2, 0));

        Self {
            item_column_view_model: ItemColumnViewModel::new(store, &*ITEM_COLUMNS),
            running,
            search: None,
            search_in_progress: false,
            sheet_state,
            last_table_size: None,
            last_action_time: Instant::now(),
            action_description: None,
            help_shown: false,
        }
    }

    pub fn render_to<B: Backend>(&mut self, f: &mut Frame<'_, B>) {
        let status = if let Some(search) = &self.search {
            format!(" - search: \"{}\"", search)
        } else {
            "".to_string()
        };

        let title = format!("Pachinko{}", status);
        let title_width = f.size().width as usize;
        let action_description = if let Some((at, description)) = &self.action_description {
            if Instant::now().saturating_duration_since(*at).as_secs() < 5 {
                Some(description.clone())
            } else {
                None
            }
        } else {
            None
        }
        .unwrap_or("F1 for help".to_string());

        let outer_frame = Block::default().title(Span::styled(
            format!(
                " {} {:>width$} ",
                title,
                action_description,
                width = title_width - title.len() - 3,
            ),
            Style::default().add_modifier(Modifier::REVERSED),
        ));
        let inner_size = outer_frame.inner(f.size());

        f.render_widget(outer_frame, f.size());

        self.last_table_size = Some(inner_size);

        let (header, column_widths, displayed_rows) =
            self.item_column_view_model.render(&self.search).unwrap();

        let selected_column = self.sheet_state.selection().column();

        f.render_stateful_widget(
            Sheet::new(displayed_rows)
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238)),
                )
                .highlight_cell_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(242)),
                )
                .highlight_i_style(
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .bg(Color::Indexed(242)),
                )
                .header(
                    Row::new(header.iter().enumerate().map(|(i, h)| {
                        Span::styled(
                            h,
                            if selected_column == Some(i) {
                                Style::default().add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            },
                        )
                    }))
                    .style(Style::default().add_modifier(Modifier::REVERSED)),
                )
                .widths(&column_widths)
                .column_spacing(1),
            inner_size.inner(&Margin {
                horizontal: 1,
                vertical: 0,
            }),
            &mut self.sheet_state,
        );

        if self.help_shown {
            let help_frame = Block::default()
                .title(Span::styled(
                    " Help ",
                    Style::default()
                        .bg(Color::Black)
                        .add_modifier(Modifier::REVERSED),
                ))
                .borders(Borders::ALL);
            let help_frame_size = f.size().inner(&Margin {
                horizontal: 1,
                vertical: 1,
            });
            let help_size = help_frame.inner(help_frame_size);

            f.render_widget(help_frame, help_frame_size);

            f.render_widget(Clear, help_size);

            let help_rows: Vec<_> = [
                &["F1", "Show/hide this help screen"],
                &["F5", "Refresh the list of items"],
                &["F12", "Quit"],
                &["Up/Down", "Move between rows"],
                &["Left/Right", "Move through text"],
                &["Alt+Left/Right", "Move between columns"],
                &["Alt+Backspace", "Undo the last change"],
                &["Alt+Delete", "Delete the current item"],
                &["Alt+Enter", "Create a new item"],
            ]
            .iter()
            .map(|r| Row::new(r.into_iter().map(|c| c.to_string()).collect::<Vec<_>>()))
            .collect();
            f.render_widget(
                Sheet::new(help_rows.iter()).widths(&[Constraint::Length(16), Constraint::Min(0)]),
                help_size.inner(&Margin {
                    horizontal: 1,
                    vertical: 0,
                }),
            );
        }
    }

    //     fn insert_item(&mut self) {
    //         if let Some(insertion_point) = self.insertion_point() {
    //             let location = self.items[insertion_point - 1].location.clone();

    //             self.items.insert(
    //                 insertion_point,
    //                 Item {
    //                     object_id: None,
    //                     name: "".to_string(),
    //                     location,
    //                     bin_no: 1,
    //                     size: "S".to_string(),
    //                 },
    //             );
    //             self.select_row(insertion_point);
    //         }
    //     }

    fn reset_idle(&mut self) {
        self.last_action_time = Instant::now();
    }

    fn check_idle(&mut self) -> bool {
        if Instant::now() - self.last_action_time > Duration::from_millis(1000) {
            if self.item_column_view_model.persist_pending_edits() {
                return true;
            }
        }

        false
    }

    pub fn handle(&mut self, ev: Event) -> bool {
        if self.handle_internal(ev) {
            self.reset_idle();
            return true;
        } else {
            return self.check_idle();
        }
    }

    fn handle_internal(&mut self, ev: Event) -> bool {
        if let Event::Key(ke) = ev {
            if ke.modifiers.contains(KeyModifiers::CONTROL) && ke.kind == KeyEventKind::Press {
                if let KeyCode::Char(c) = ke.code {
                    if !self.search_in_progress {
                        self.search = Some("".to_string());
                    }

                    self.search.get_or_insert_with(|| "".to_string()).push(c);
                    self.search_in_progress = true;
                    self.reset_selection();

                    return true;
                }
            }

            if ke.code == KeyCode::Modifier(ModifierKeyCode::LeftControl)
                || ke.code == KeyCode::Modifier(ModifierKeyCode::RightControl)
            {
                match ke.kind {
                    KeyEventKind::Press => {
                        self.search = Some("".to_string());
                        self.search_in_progress = true;
                        self.reset_selection();
                    }
                    KeyEventKind::Release => {
                        self.search_in_progress = false;

                        if let Some(search) = &self.search {
                            if search.is_empty() {
                                self.search = None;
                            }
                        }
                    }
                    _ => {
                        return false;
                    }
                }

                return true;
            }
        }

        match ev {
            Event::Key(e) => {
                if e.kind == KeyEventKind::Press || e.kind == KeyEventKind::Repeat {
                    match e.code {
                        KeyCode::F(1) => {
                            self.help_shown = !self.help_shown;
                        }
                        KeyCode::F(5) => {
                            self.item_column_view_model.refresh().unwrap();
                        }
                        KeyCode::F(12) => {
                            self.running.store(false, Ordering::SeqCst);
                        }
                        KeyCode::Backspace if e.modifiers == KeyModifiers::ALT => {
                            if let Some(description) = self.item_column_view_model.undo().unwrap() {
                                self.action_description =
                                    Some((Instant::now(), format!("undid {}", description)));
                            }
                        }
                        KeyCode::Enter if e.modifiers == KeyModifiers::ALT => {
                            self.item_column_view_model
                                .insert_item(
                                    self.sheet_state.selection().row().unwrap_or(0),
                                    &self.search,
                                )
                                .unwrap();

                            self.sheet_state
                                .map_selection(|s| s.map_row_or(0, |r| r + 1));
                        }
                        KeyCode::Delete if e.modifiers == KeyModifiers::ALT => {
                            if let Some(row) = self.sheet_state.selection().row() {
                                let item_name =
                                    self.item_column_view_model.delete_item(row).unwrap();
                                self.action_description =
                                    Some((Instant::now(), format!("deleted: {}", item_name)));
                            }
                        }
                        KeyCode::Up => {
                            self.move_up();
                        }
                        KeyCode::Down => {
                            self.move_down();
                        }
                        KeyCode::Left if e.modifiers == KeyModifiers::ALT => {
                            self.move_to_cell_rel(-1);
                        }
                        KeyCode::Right if e.modifiers == KeyModifiers::ALT => {
                            self.move_to_cell_rel(1);
                        }
                        KeyCode::Esc => {
                            self.back_out();
                        }
                        KeyCode::PageUp => {
                            if let Some(table_size) = self.last_table_size {
                                self.scroll_up((table_size.height as usize).saturating_sub(3));
                            }
                        }
                        KeyCode::PageDown => {
                            if let Some(table_size) = self.last_table_size {
                                self.scroll_down((table_size.height as usize).saturating_sub(3));
                            }
                        }
                        // KeyCode::Enter if e.modifiers.contains(KeyModifiers::SHIFT) => {
                        //     self.insert_item();
                        // }
                        KeyCode::Home => {
                            self.move_char_first();
                        }
                        KeyCode::End => {
                            self.move_char_end();
                        }
                        KeyCode::Left => {
                            self.move_char_left();
                        }
                        KeyCode::Right => {
                            self.move_char_right();
                        }
                        KeyCode::Backspace => {
                            if let SheetSelection::Char(row, cell, i) = self.sheet_state.selection()
                            {
                                if i > 0 {
                                    let new_i = i - 1;
                                    self.item_column_view_model.delete_char(row, cell, new_i);

                                    self.sheet_state
                                        .select(SheetSelection::Char(row, cell, new_i));
                                }
                            }
                        }
                        KeyCode::Delete => {
                            if let SheetSelection::Char(row, cell, i) = self.sheet_state.selection()
                            {
                                self.item_column_view_model.delete_char(row, cell, i);
                            }
                        }
                        KeyCode::Char(orig_c) => {
                            let c = if e.modifiers.contains(KeyModifiers::SHIFT) {
                                orig_c.to_ascii_uppercase()
                            } else {
                                orig_c
                            };

                            match self.sheet_state.selection() {
                                SheetSelection::Char(row, cell, i) => {
                                    let new_i =
                                        self.item_column_view_model.insert_char(row, cell, i, c);

                                    self.sheet_state
                                        .select(SheetSelection::Char(row, cell, new_i));
                                }
                                SheetSelection::Cell(row, cell) => {
                                    self.item_column_view_model.insert_char(row, cell, 0, c);
                                }
                                _ => {}
                            }
                        }
                        _ => {
                            return false;
                        }
                    }
                }
            }
            Event::Mouse(e) => match e.kind {
                crossterm::event::MouseEventKind::ScrollUp => {
                    self.scroll_up(3);
                }
                crossterm::event::MouseEventKind::ScrollDown => {
                    self.scroll_down(3);
                }
                _ => {
                    return false;
                }
            },
            Event::Resize(..) => {}
            _ => {
                return false;
            }
        }

        true
    }

    pub fn handle_idle(&mut self) -> bool {
        self.check_idle()
    }

    fn move_up(&mut self) {
        use SheetSelection::*;

        let default_row = self.sheet_state.get_offset();
        self.sheet_state.map_selection(|s| match s {
            None => Row(default_row),
            Row(r) => Row(r.saturating_sub(1)),
            Cell(r, c) => Cell(r.saturating_sub(1), c),
            Char(r, c, _) => Char(r.saturating_sub(1), c, 0),
        });
    }

    fn move_down(&mut self) {
        use SheetSelection::*;

        let default_row = self.sheet_state.get_offset();
        self.sheet_state.map_selection(|s| match s {
            None => Row(default_row),
            Row(r) => Row(r + 1),
            Cell(r, c) => Cell(r + 1, c),
            Char(r, c, _) => Char(r + 1, c, 0),
        });
    }

    fn move_to_cell_rel(&mut self, offset: isize) {
        use SheetSelection::*;

        let item_column_view_model = &self.item_column_view_model;
        let default_column = if offset < 0 {
            item_column_view_model.rightmost_column_index()
        } else {
            0
        };

        self.sheet_state.map_selection(|s| {
            let (new_row, new_col) = match s {
                None => (0, default_column),
                Row(r) => (r, default_column),
                Cell(r, c) | Char(r, c, _) => (
                    r,
                    item_column_view_model.column_index_saturating_add(c, offset),
                ),
            };

            if item_column_view_model.column_allows_char_selection(new_col) {
                Char(new_row, new_col, 0)
            } else {
                Cell(new_row, new_col)
            }
        });
    }

    fn reset_selection(&mut self) {
        self.sheet_state.select(SheetSelection::Char(0, 2, 0));
    }

    fn back_out(&mut self) {
        use SheetSelection::*;
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) => None,
            Cell(r, _) | Char(r, _, _) => Row(r),
        });
    }

    fn move_char_first(&mut self) {
        use SheetSelection::*;
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) | Cell(_, _) => s,
            Char(r, c, i) => Char(r, c, 0),
        });
    }

    fn move_char_end(&mut self) {
        use SheetSelection::*;
        let item_column_view_model = &self.item_column_view_model;
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) | Cell(_, _) => s,
            Char(r, c, _) => Char(
                r,
                c,
                item_column_view_model.get_column_len(r, c).unwrap_or(0),
            ),
        });
    }

    fn move_char_left(&mut self) {
        use SheetSelection::*;
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) | Cell(_, _) => s,
            Char(r, c, i) => Char(r, c, i.saturating_sub(1)),
        });
    }

    fn move_char_right(&mut self) {
        use SheetSelection::*;
        let item_column_view_model = &self.item_column_view_model;
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) | Cell(_, _) => s,
            Char(r, c, i) => Char(
                r,
                c,
                (i + 1).min(item_column_view_model.get_column_len(r, c).unwrap_or(0)),
            ),
        });
    }

    fn scroll_up(&mut self, delta: usize) {
        self.sheet_state.scroll_up(delta)
    }

    fn scroll_down(&mut self, delta: usize) {
        self.sheet_state.scroll_down(delta)
    }

    fn insertion_point(&self) -> Option<usize> {
        use SheetSelection::*;
        match self.sheet_state.selection() {
            Row(r) | Cell(r, _) => Option::Some(r + 1),
            _ => Option::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ItemColumnWidth {
    Expand,
    Shrink,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ItemColumnKind {
    Choice,
    FullText,
}

struct ItemColumn {
    header: String,
    width: ItemColumnWidth,
    kind: ItemColumnKind,
    display: fn(&Item) -> AHResult<String>,
    insert_char: Option<fn(&mut Item, usize, char) -> usize>,
    delete_char: Option<fn(&mut Item, usize)>,
    searchable: bool,
}
