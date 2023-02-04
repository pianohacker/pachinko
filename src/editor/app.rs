use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    vec,
};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use indexmap::IndexMap;
use qualia::{object, CheckpointId, ObjectShapeWithId, Queryable, Store};
use tui::{
    backend::Backend,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::Block,
    Frame,
};

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
}

struct ItemColumnViewModel {
    store: Store,
    items: Vec<Item>,
    columns: Vec<ItemColumn>,
    last_updated_checkpoint: CheckpointId,
    inserted_object_id_chains: Vec<(i64, i64)>,
    last_rendered_object_ids: Vec<i64>,
}

impl ItemColumnViewModel {
    fn new(store: Store, columns: Vec<ItemColumn>) -> Self {
        Self {
            store,
            items: vec![],
            columns,
            last_updated_checkpoint: 0,
            inserted_object_id_chains: Vec::new(),
            last_rendered_object_ids: Vec::new(),
        }
    }

    fn refresh_if_needed(&mut self) -> AHResult<()> {
        if self.store.modified_since(self.last_updated_checkpoint)? {
            self.last_updated_checkpoint = self.store.last_checkpoint_id()?;
            self.items = self
                .store
                .query(Item::q())
                .iter_converted(&self.store)?
                .collect();
            self.items
                .sort_by_key(|i| (i.location.name.clone(), i.bin_no, i.name.clone()))
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
            .map(|o| {
                (
                    o.get_object_id().unwrap(),
                    columns
                        .iter()
                        .map(|c| (c.display)(o).unwrap_or("".into()))
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
                    ItemColumn::new(
                        "Location",
                        ItemColumnWidth::Shrink,
                        ItemColumnKind::Choice,
                        |i| Ok(i.format().format_location()),
                    ),
                    ItemColumn::new(
                        "Size",
                        ItemColumnWidth::Shrink,
                        ItemColumnKind::Choice,
                        |i: &Item| Ok(i.size.clone()),
                    )
                    .searchable(false),
                    ItemColumn::new(
                        "Name",
                        ItemColumnWidth::Expand,
                        ItemColumnKind::FullText,
                        |i| Ok(i.name.clone()),
                    ),
                ],
            ),
            running,
            search: None,
            search_in_progress: false,
            sheet_state,
            last_table_size: None,
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

    pub fn handle(&mut self, ev: Event) {
        if let Event::Key(ke) = ev {
            if ke.modifiers.contains(KeyModifiers::CONTROL) && ke.kind == KeyEventKind::Press {
                if let KeyCode::Char(c) = ke.code {
                    if !self.search_in_progress {
                        self.search = Some("".to_string());
                    }

                    self.search.get_or_insert_with(|| "".to_string()).push(c);
                    self.search_in_progress = true;
                    self.reset_selection();

                    return;
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
                    _ => {}
                }
                return;
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
                        KeyCode::Char(_) => {
                            // self.handle_key(e);
                        }
                        _ => {}
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
                _ => {}
            },
            _ => {}
        }
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
    searchable: bool,
}

impl ItemColumn {
    fn new(
        header: impl Into<String>,
        width: ItemColumnWidth,
        kind: ItemColumnKind,
        display: fn(&Item) -> AHResult<String>,
    ) -> Self {
        Self {
            header: header.into(),
            width,
            kind,
            display,
            searchable: true,
        }
    }

    fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }
}
