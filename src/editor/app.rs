use std::{
    convert::TryFrom,
    fmt::Display,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use qualia::{
    query::QueryNode, CachedMapping, CheckpointId, Object, ObjectShape, Queryable, Store, Q,
};
use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget},
    Frame,
};

use crate::types::{FormattedItem, Item};
use crate::AHResult;

use super::sheet::{Row, Sheet, SheetState};

pub struct App {
    store: Store,
    items: Vec<Item>,
    last_updated_checkpoint: CheckpointId,
    running: Arc<AtomicBool>,
    search: String,
    search_in_progress: bool,
    empty_alt_in_progress: bool,
    editor_table_state: EditorTableState,
    last_table_size: Option<Rect>,
}

struct DisplayItem {
    name_highlight_indices: Vec<usize>,
    item: FormattedItem,
}

impl Deref for DisplayItem {
    type Target = FormattedItem;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl App {
    pub fn new(store: Store, running: Arc<AtomicBool>) -> Self {
        Self {
            store,
            running,
            items: vec![],
            last_updated_checkpoint: 0,
            search: "".to_string(),
            search_in_progress: false,
            empty_alt_in_progress: false,
            editor_table_state: EditorTableState::default(),
            last_table_size: None,
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

    pub fn render_to<B: Backend>(&mut self, f: &mut Frame<'_, B>) {
        self.refresh_if_needed().unwrap();

        let status = if self.search.is_empty() {
            "".to_string()
        } else {
            format!(" - search: \"{}\"", self.search)
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

        let items: Vec<DisplayItem> = if self.search.is_empty() {
            self.items
                .iter()
                .map(|i| DisplayItem {
                    item: i.format(),
                    name_highlight_indices: Vec::default(),
                })
                .collect()
        } else {
            let matcher = SkimMatcherV2::default();

            let mut result: Vec<_> = self
                .items
                .iter()
                .filter_map(|i| {
                    matcher
                        .fuzzy_indices(&i.name, &self.search)
                        .map(|(score, indices)| (score, indices, i))
                })
                .collect();

            result.sort_by_key(|(score, _, i)| (-*score, i.format().format_location(), &i.name));

            result
                .into_iter()
                .map(|(_, indices, i)| DisplayItem {
                    item: i.format(),
                    name_highlight_indices: indices,
                })
                .collect()
        };

        f.render_stateful_widget(
            EditorTable::new(items).columns(vec![
                EditorColumn::new("Location", EditorColumnWidth::Shrink, |i| {
                    Ok(i.format_location().into())
                }),
                EditorColumn::new("Size", EditorColumnWidth::Shrink, |i| {
                    Ok(i.size.clone().into())
                }),
                EditorColumn::new("Name", EditorColumnWidth::Expand, |i| {
                    if i.name_highlight_indices.is_empty() {
                        Ok(i.name.clone().into())
                    } else {
                        let mut spans: Vec<_> =
                            i.name.chars().map(|c| Span::raw(c.to_string())).collect();

                        for idx in &i.name_highlight_indices {
                            spans[*idx] = Span::styled(
                                spans[*idx].content.clone(),
                                Style::default().bg(Color::Indexed(58)),
                            );
                        }

                        Ok(Spans::from(spans))
                    }
                }),
            ]),
            inner_size,
            &mut self.editor_table_state,
        );
    }

    pub fn handle(&mut self, ev: Event) {
        if let Event::Key(ke) = ev {
            if ke.modifiers.contains(KeyModifiers::CONTROL) && ke.kind == KeyEventKind::Press {
                if let KeyCode::Char(c) = ke.code {
                    if !self.search_in_progress {
                        self.search = "".to_string();
                    }
                    self.empty_alt_in_progress = false;

                    self.search.push(c);
                    self.search_in_progress = true;

                    return;
                }
            }

            if ke.code == KeyCode::Modifier(ModifierKeyCode::LeftControl)
                || ke.code == KeyCode::Modifier(ModifierKeyCode::RightControl)
            {
                match ke.kind {
                    KeyEventKind::Press => {
                        self.search_in_progress = false;

                        if self.empty_alt_in_progress {
                            self.search = "".to_string();
                        }
                    }
                    KeyEventKind::Release => {
                        self.empty_alt_in_progress = true;
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
                        KeyCode::Char('q') => {
                            self.running.store(false, Ordering::SeqCst);
                        }
                        KeyCode::Up => {
                            self.editor_table_state.move_up();
                        }
                        KeyCode::Down => {
                            self.editor_table_state.move_down();
                        }
                        KeyCode::Left => {
                            self.editor_table_state.move_left();
                        }
                        KeyCode::Right => {
                            self.editor_table_state.move_right();
                        }
                        KeyCode::Esc => {
                            self.editor_table_state.back_out();
                        }
                        KeyCode::PageUp => {
                            if let Some(table_size) = self.last_table_size {
                                self.editor_table_state
                                    .scroll_up((table_size.height as usize).saturating_sub(3));
                            }
                        }
                        KeyCode::PageDown => {
                            if let Some(table_size) = self.last_table_size {
                                self.editor_table_state
                                    .scroll_down((table_size.height as usize).saturating_sub(3));
                            }
                        }
                        KeyCode::Enter if e.modifiers.contains(KeyModifiers::SHIFT) => {
                            if let Some(insertion_point) = self.editor_table_state.insertion_point()
                            {
                                let location = self.items[insertion_point - 1].location.clone();

                                self.items.insert(
                                    insertion_point,
                                    Item {
                                        object_id: None,
                                        name: "".to_string(),
                                        location,
                                        bin_no: 1,
                                        size: "S".to_string(),
                                    },
                                );
                                self.editor_table_state.select_row(insertion_point);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Mouse(e) => match e.kind {
                crossterm::event::MouseEventKind::ScrollUp => {
                    self.editor_table_state.scroll_up(3);
                }
                crossterm::event::MouseEventKind::ScrollDown => {
                    self.editor_table_state.scroll_down(3);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

enum EditorColumnWidth {
    Expand,
    Shrink,
}

struct EditorColumn<O> {
    header: String,
    display: fn(&O) -> AHResult<Spans>,
    width: EditorColumnWidth,
}

impl<O> EditorColumn<O> {
    fn new(
        header: impl Into<String>,
        width: EditorColumnWidth,
        display: fn(&O) -> AHResult<Spans>,
    ) -> Self {
        Self {
            header: header.into(),
            width,
            display,
        }
    }
}

struct EditorTable<O> {
    objects: Vec<O>,
    columns: Vec<EditorColumn<O>>,
}

impl<O> EditorTable<O> {
    fn new(objects: impl IntoIterator<Item = O>) -> Self {
        Self {
            objects: objects.into_iter().collect(),
            columns: vec![],
        }
    }

    fn columns(mut self, columns: Vec<EditorColumn<O>>) -> Self {
        self.columns = columns;

        self
    }
}

#[derive(Default)]
struct EditorTableState {
    table_state: SheetState,
}

impl EditorTableState {
    fn move_up(&mut self) {
        self.table_state.select_row(
            self.table_state
                .selected_row()
                .map_or(Some(0), |s| Some(s.saturating_sub(1))),
        );
    }

    fn move_down(&mut self) {
        self.table_state.select_row(
            self.table_state
                .selected_row()
                .map_or(Some(0), |s| Some(s + 1)),
        );
    }

    fn select_row(&mut self, row: usize) {
        self.table_state.select_row(Some(row));
    }

    fn move_left(&mut self) {
        self.table_state
            .select_row_and_cell(match self.table_state.selected_row_and_cell() {
                None => Some((0, Some(0))),
                Some((r, None)) => Some((r, Some(usize::MAX))), // Will get reduced to actual width by table renderer
                Some((r, Some(c))) => Some((r, Some(c.saturating_sub(1)))),
            });
    }

    fn move_right(&mut self) {
        self.table_state
            .select_row_and_cell(match self.table_state.selected_row_and_cell() {
                None => Some((0, Some(0))),
                Some((r, None)) => Some((r, Some(0))),
                Some((r, Some(c))) => Some((r, Some(c + 1))),
            });
    }

    fn back_out(&mut self) {
        self.table_state
            .select_row_and_cell(match self.table_state.selected_row_and_cell() {
                None | Some((_, None)) => None,
                Some((r, Some(_))) => Some((r, None)),
            });
    }

    fn scroll_up(&mut self, delta: usize) {
        self.table_state.scroll_up(delta)
    }

    fn scroll_down(&mut self, delta: usize) {
        self.table_state.scroll_down(delta)
    }

    fn insertion_point(&self) -> Option<usize> {
        match self.table_state.selected_row_and_cell() {
            Some((r, _)) => Some(r + 1),
            _ => None,
        }
    }
}

impl<O> StatefulWidget for EditorTable<O> {
    type State = EditorTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let columns = self.columns;

        let rows: Vec<Vec<_>> = self
            .objects
            .iter()
            .map(|o| {
                columns
                    .iter()
                    .map(|c| (c.display)(o).unwrap_or("".into()))
                    .collect()
            })
            .collect();

        let column_widths = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                std::iter::once(c.header.len())
                    .chain(rows.iter().map(|r| r[i].width()))
                    .max()
                    .unwrap()
            })
            .collect::<Vec<_>>();

        StatefulWidget::render(
            Sheet::new(rows.into_iter().map(|r| Row::new(r)))
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
                .header(
                    Row::new(columns.iter().map(|c| c.header.clone()))
                        .style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)),
                )
                .widths(
                    &columns
                        .iter()
                        .enumerate()
                        .map(|(i, c)| match c.width {
                            EditorColumnWidth::Shrink => {
                                Constraint::Length(column_widths[i] as u16)
                            }
                            EditorColumnWidth::Expand => Constraint::Min(column_widths[i] as u16),
                        })
                        .collect::<Vec<_>>(),
                )
                .column_spacing(1),
            area.inner(&Margin {
                horizontal: 1,
                vertical: 0,
            }),
            buf,
            &mut state.table_state,
        );
    }
}

impl<O> Widget for EditorTable<O> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = EditorTableState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
