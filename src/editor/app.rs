use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
    vec,
};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use indexmap::{IndexMap, IndexSet};
use qualia::{CheckpointId, ObjectShapeWithId, Queryable, Store};
use tui::{
    backend::Backend,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::Block,
    Frame,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{types::Item, utils::add_item};
use crate::{types::ItemSize, AHResult};

use super::sheet::{Row, Sheet, SheetSelection, SheetState};

pub struct App {
    item_column_view_model: ItemColumnViewModel,
    running: Arc<AtomicBool>,
    search: Option<String>,
    search_in_progress: bool,
    sheet_state: SheetState,
    last_table_size: Option<Rect>,
    last_action_time: Instant,
}

struct ItemColumnViewModel {
    store: Store,
    items: IndexMap<i64, Item>,
    columns: Vec<ItemColumn>,
    last_updated_checkpoint: CheckpointId,
    inserted_object_id_chains: Vec<(i64, i64)>,
    last_rendered_object_ids: Vec<i64>,
    edited_items: IndexMap<i64, Item>,
}

impl ItemColumnViewModel {
    fn new(store: Store, columns: Vec<ItemColumn>) -> Self {
        Self {
            store,
            items: IndexMap::new(),
            columns,
            last_updated_checkpoint: 0,
            inserted_object_id_chains: Vec::new(),
            last_rendered_object_ids: Vec::new(),
            edited_items: IndexMap::new(),
        }
    }

    fn refresh_if_needed(&mut self) -> AHResult<()> {
        if self.store.modified_since(self.last_updated_checkpoint)? {
            self.last_updated_checkpoint = self.store.last_checkpoint_id()?;

            let mut items = self
                .store
                .query(Item::q())
                .iter_converted::<Item>(&self.store)?
                .map(|i| (i.get_object_id().unwrap(), i))
                .collect();

            if self.items.is_empty() {
                self.items = items;

                self.items.sort_by(|_, a, _, b| {
                    (&a.location.name, a.bin_no, &a.name).cmp(&(
                        &b.location.name,
                        b.bin_no,
                        &b.name,
                    ))
                });
            } else {
                // First, build the list of new items using the order of the old items.
                // This brings in modifications (by pulling from the new set of items) and
                // deletions (where item.remove()) will return None.
                let mut reordered_items: IndexMap<_, _> = self
                    .items
                    .keys()
                    .filter_map(|id| items.remove(id).map(|item| (*id, item)))
                    .collect();

                // All that remains in `items` is new items.
                for (object_id, item) in items.into_iter() {
                    let insert_pos = self
                        .items
                        .values()
                        .collect::<Vec<_>>()
                        .binary_search_by(|i| {
                            (&i.location.name, i.bin_no, &i.name).cmp(&(
                                &item.location.name,
                                item.bin_no,
                                &item.name,
                            ))
                        })
                        .map_or_else(|e| e, |o| o);
                    reordered_items.insert(object_id, item);
                    reordered_items.move_index(reordered_items.len() - 1, insert_pos);
                }

                self.items = reordered_items;
            }
        }

        Ok(())
    }

    fn render(
        &mut self,
        search: &Option<String>,
    ) -> AHResult<(Vec<String>, Vec<Constraint>, Vec<Row>)> {
        self.refresh_if_needed()?;

        let columns = &self.columns;
        let rows: Vec<(i64, Vec<_>)> = self
            .items
            .iter()
            .map(|(id, o)| {
                (
                    *id,
                    columns
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (c.display)(o).unwrap_or("".into()))
                        .collect(),
                )
            })
            .collect();

        let column_widths = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                std::iter::once(c.header.len())
                    .chain(rows.iter().map(|(_, r)| r[i].len()))
                    .max()
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let non_empty_search = search
            .as_ref()
            .and_then(|s| if s.is_empty() { None } else { Some(s) });

        let mut displayed_rows: IndexMap<_, _> = if let Some(search) = non_empty_search {
            let matcher = SkimMatcherV2::default();

            let mut scored_result: Vec<_> = rows
                .iter()
                .filter_map(|(object_id, row)| {
                    let column_results: Vec<_> = row
                        .into_iter()
                        .enumerate()
                        .map(|(i, c)| {
                            if !columns[i].searchable {
                                return (c, 0, vec![]);
                            }

                            matcher.fuzzy_indices(c, search).map_or_else(
                                || (c, 0, vec![]),
                                |(score, indices)| (c, score, indices),
                            )
                        })
                        .collect();

                    let total_score: i64 = column_results.iter().map(|(_, score, _)| score).sum();

                    if total_score == 0 {
                        return None;
                    }

                    Some((
                        total_score,
                        object_id,
                        Row::new(column_results.into_iter().map(|(c, _, indices)| {
                            let mut spans: Vec<_> =
                                c.chars().map(|c| Span::raw(c.to_string())).collect();

                            for idx in &indices {
                                spans[*idx] = Span::styled(
                                    spans[*idx].content.clone(),
                                    Style::default().bg(Color::Indexed(58)),
                                );
                            }

                            Spans::from(spans)
                        })),
                    ))
                })
                .collect();

            scored_result.sort_by_key(|(score, _, _)| -score);

            scored_result
                .into_iter()
                .map(|(_, object_id, i)| (*object_id, i))
                .collect()
        } else {
            rows.into_iter()
                .map(|(object_id, r)| (object_id, Row::new(r)))
                .collect()
        };

        for (after_object_id, object_id) in self.inserted_object_id_chains.iter() {
            if let Some(after_index) = displayed_rows.get_index_of(after_object_id) {
                if let Some(index) = displayed_rows.get_index_of(object_id) {
                    displayed_rows.move_index(index, after_index + 1);
                }
            }
        }

        self.last_rendered_object_ids = displayed_rows.keys().map(|id| *id).collect();

        Ok((
            columns.iter().map(|c| c.header.clone()).collect(),
            columns
                .iter()
                .enumerate()
                .map(|(i, c)| match c.width {
                    ItemColumnWidth::Shrink => Constraint::Length(column_widths[i] as u16),
                    ItemColumnWidth::Expand => Constraint::Min(column_widths[i] as u16),
                })
                .collect::<Vec<_>>(),
            displayed_rows.into_values().collect(),
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

    fn reset_view(&mut self) {
        self.inserted_object_id_chains.clear();
    }

    fn insert_item(&mut self, after_index: usize) -> AHResult<()> {
        let after_object_id = self.last_rendered_object_ids[after_index];
        let after_item: Item = self
            .store
            .query(Item::q().id(after_object_id))
            .one_converted(&self.store)
            .unwrap();
        let last_location = after_item.location.clone();

        let item = add_item(
            &mut self.store,
            "".to_string(),
            &last_location,
            None,
            ItemSize::M,
        )?;
        let new_object_id = item.get_object_id().unwrap();

        self.inserted_object_id_chains
            .push((after_item.get_object_id().unwrap(), new_object_id));

        Ok(())
    }

    fn insert_char(&mut self, row: usize, cell: usize, i: usize, c: char) -> usize {
        let column_insert_char = match self.columns[cell].insert_char {
            Some(f) => f,
            None => return i,
        };

        let object_id = self.last_rendered_object_ids[row];
        let item = self.items.get_mut(&object_id).unwrap();

        self.edited_items
            .entry(object_id)
            .or_insert_with(|| item.clone());

        column_insert_char(item, i, c)
    }

    fn delete_char(&mut self, row: usize, cell: usize, i: usize) {
        let column_delete_char = match self.columns[cell].delete_char {
            Some(f) => f,
            None => return,
        };

        let object_id = self.last_rendered_object_ids[row];
        let item = self.items.get_mut(&object_id).unwrap();

        self.edited_items
            .entry(object_id)
            .or_insert_with(|| item.clone());

        column_delete_char(item, i)
    }

    fn persist_pending_edits(&mut self) -> bool {
        if self.edited_items.len() == 0 {
            return false;
        }

        let checkpoint = self.store.checkpoint().unwrap();
        for object_id in self.edited_items.keys() {
            checkpoint
                .query(Item::q().id(*object_id))
                .set(self.items[object_id].clone().into())
                .unwrap();
        }
        let names: Vec<_> = self
            .edited_items
            .values()
            .map(|i| format!("\"{}\"", i.name))
            .collect();
        checkpoint
            .commit(format!("update items: {}", names.join(", ")))
            .unwrap();

        self.edited_items.clear();

        true
    }

    fn undo(&mut self) {
        self.store.undo().unwrap();
    }
}

impl App {
    pub fn new(store: Store, running: Arc<AtomicBool>) -> Self {
        let mut sheet_state = SheetState::default();
        sheet_state.select(SheetSelection::Char(0, 2, 0));

        Self {
            item_column_view_model: ItemColumnViewModel::new(
                store,
                vec![
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
                        display: |i: &Item| Ok(i.size.clone()),
                        insert_char: None,
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
                ],
            ),
            running,
            search: None,
            search_in_progress: false,
            sheet_state,
            last_table_size: None,
            last_action_time: Instant::now(),
        }
    }

    pub fn render_to<B: Backend>(&mut self, f: &mut Frame<'_, B>) {
        let status = if let Some(search) = &self.search {
            format!(" - search: \"{}\"", search)
        } else {
            "".to_string()
        };

        let outer_frame = Block::default().title(Span::styled(
            format!(
                "{:width$}",
                format!("Pachinko{}", status),
                width = f.size().width as usize
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
                        KeyCode::F(12) => {
                            self.running.store(false, Ordering::SeqCst);
                        }
                        KeyCode::Backspace if e.modifiers == KeyModifiers::ALT => {
                            self.item_column_view_model.undo();
                        }
                        KeyCode::Enter if e.modifiers == KeyModifiers::ALT => {
                            self.item_column_view_model
                                .insert_item(self.sheet_state.selection().row().unwrap_or(0))
                                .unwrap();

                            self.sheet_state
                                .map_selection(|s| s.map_row_or(0, |r| r + 1));
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

                            if let SheetSelection::Char(row, cell, i) = self.sheet_state.selection()
                            {
                                let new_i =
                                    self.item_column_view_model.insert_char(row, cell, i, c);

                                self.sheet_state
                                    .select(SheetSelection::Char(row, cell, new_i));
                            }
                            // self.handle_key(e);
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

    fn select_row(&mut self, row: usize) {
        self.sheet_state.map_selection(|s| s.with_row(row));
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
        self.item_column_view_model.reset_view();
    }

    fn back_out(&mut self) {
        use SheetSelection::*;
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) => None,
            Cell(r, _) | Char(r, _, _) => Row(r),
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
        self.sheet_state.map_selection(|s| match s {
            None | Row(_) | Cell(_, _) => s,
            Char(r, c, i) => Char(r, c, i + 1),
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
