use std::{
    convert::TryFrom,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};
use qualia::{
    query::QueryNode, CachedMapping, CheckpointId, Object, ObjectShape, Queryable, Store, Q,
};
use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget},
    Frame,
};

use crate::types::Item;
use crate::AHResult;

use super::sheet::{Row, Sheet, SheetState};

pub struct App {
    store: Store,
    items: Vec<Item>,
    last_updated_checkpoint: CheckpointId,
    running: Arc<AtomicBool>,
    search: String,
    search_in_progress: bool,
    search_forward: bool,
    empty_alt_in_progress: bool,
    editor_table_state: EditorTableState,
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
            search_forward: true,
            empty_alt_in_progress: false,
            editor_table_state: EditorTableState::default(),
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

        let outer_frame = Block::default().title(Span::styled(
            format!("{:width$}", "Pachinko", width = f.size().width as usize),
            Style::default().add_modifier(Modifier::REVERSED),
        ));
        let inner_size = outer_frame.inner(f.size());
        f.render_widget(outer_frame, f.size());

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(inner_size);

        f.render_stateful_widget(
            EditorTable::new(&self.items).columns(vec![
                EditorColumn::new("Location", EditorColumnWidth::Shrink, |o| {
                    Ok(o.format().format_location())
                }),
                EditorColumn::new("Size", EditorColumnWidth::Shrink, |o| Ok(o.size.clone())),
                EditorColumn::new("Name", EditorColumnWidth::Expand, |o| Ok(o.name.clone())),
            ]),
            chunks[0],
            &mut self.editor_table_state,
        );

        let status = if self.search.is_empty() {
            "".to_string()
        } else {
            format!("Search: {}", self.search)
        };

        f.render_widget(
            Paragraph::new(status).block(Block::default().borders(Borders::TOP)),
            chunks[1],
        );
    }

    fn select_search(&mut self, offset: usize) {
        let char_matchers: Vec<String> = self
            .search
            .chars()
            .map(|c| format!("{}.*", regex::escape(&c.to_string())))
            .collect();
        let re = regex::Regex::new(&("(?i).*".to_string() + &char_matchers.join(""))).unwrap();

        let effective_offset = offset
            * if self.search_forward {
                1
            } else {
                self.items.len().saturating_sub(1)
            };

        let start_i =
            (self.editor_table_state.selected().unwrap_or(0) + effective_offset) % self.items.len();

        let indices: Vec<usize> = if self.search_forward {
            ((start_i)..self.items.len()).chain(0..(start_i)).collect()
        } else {
            (0..=(start_i))
                .rev()
                .chain(((start_i + 1)..self.items.len()).rev())
                .collect()
        };

        for i in indices {
            if re.is_match(&self.items[i].name) {
                self.editor_table_state.set_selected(i);
                break;
            }
        }
    }

    pub fn handle(&mut self, ev: Event) {
        if let Event::Key(ke) = ev {
            if ke.modifiers.contains(KeyModifiers::ALT) && ke.kind == KeyEventKind::Press {
                if let KeyCode::Char(c) = ke.code {
                    self.empty_alt_in_progress = false;
                    if !self.search_in_progress {
                        self.search = "".to_string();
                    }

                    self.search.push(c);
                    self.search_in_progress = true;

                    self.select_search(0);
                    return;
                }
            }

            if ke.code == KeyCode::Modifier(ModifierKeyCode::LeftAlt)
                || ke.code == KeyCode::Modifier(ModifierKeyCode::RightAlt)
            {
                match ke.kind {
                    KeyEventKind::Press => {
                        self.empty_alt_in_progress = true;
                        self.search_forward =
                            ke.code == KeyCode::Modifier(ModifierKeyCode::RightAlt);
                    }
                    KeyEventKind::Release => {
                        if self.empty_alt_in_progress {
                            self.select_search(1);
                        }
                        self.search_in_progress = false;
                    }
                    _ => {}
                }
                return;
            }
        }

        match ev {
            Event::Key(e) => {
                if e.kind == KeyEventKind::Press || e.kind == KeyEventKind::Repeat {
                    // Backup in case we're on a non-enhanced terminal.
                    self.search = "".to_string();
                    self.search_in_progress = false;

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
    display: fn(&O) -> AHResult<String>,
    width: EditorColumnWidth,
}

impl<O> EditorColumn<O> {
    fn new(
        header: impl Into<String>,
        width: EditorColumnWidth,
        display: fn(&O) -> AHResult<String>,
    ) -> Self {
        Self {
            header: header.into(),
            width,
            display,
        }
    }
}

struct EditorTable<'o, O> {
    objects: &'o Vec<O>,
    columns: Vec<EditorColumn<O>>,
}

impl<'o, O> EditorTable<'o, O> {
    fn new(objects: &'o Vec<O>) -> Self {
        Self {
            objects,
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
        self.table_state.select(
            self.table_state
                .selected()
                .map_or(Some(0), |s| Some(s.saturating_sub(1))),
        );
    }

    fn move_down(&mut self) {
        self.table_state
            .select(self.table_state.selected().map_or(Some(0), |s| Some(s + 1)));
    }

    fn scroll_up(&mut self, delta: usize) {
        self.table_state.scroll_up(delta)
    }

    fn scroll_down(&mut self, delta: usize) {
        self.table_state.scroll_down(delta)
    }

    fn selected(&self) -> Option<usize> {
        self.table_state.selected()
    }

    fn set_selected(&mut self, i: usize) {
        self.table_state.select(Some(i));
    }
}

impl<'o, O> StatefulWidget for EditorTable<'o, O> {
    type State = EditorTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let columns = self.columns;

        let rows: Vec<Vec<_>> = self
            .objects
            .iter()
            .map(|o| {
                columns
                    .iter()
                    .map(|c| (c.display)(o).unwrap_or("".to_string()))
                    .collect()
            })
            .collect();

        let column_widths = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                std::iter::once(c.header.len())
                    .chain(rows.iter().map(|r| r[i].len()))
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

impl<'o, O> Widget for EditorTable<'o, O> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = EditorTableState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
