use std::{
    convert::TryFrom,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crossterm::event::{Event, KeyCode};
use qualia::{query::QueryNode, CachedCollection, Object, ObjectShape, Queryable, Store, Q};
use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget},
    Frame,
};

use crate::types::Item;
use crate::AHResult;

use super::sheet::{Row, Sheet, SheetState};

pub struct App {
    store: Store,
    running: Arc<AtomicBool>,
    editor_table_state: EditorTableState,
}

impl App {
    pub fn new(store: Store, running: Arc<AtomicBool>) -> Self {
        Self {
            store,
            running,
            editor_table_state: EditorTableState::default(),
        }
    }

    pub fn render_to<B: Backend>(&mut self, f: &mut Frame<'_, B>) {
        let outer_frame = Block::default().borders(Borders::TOP).title(" Pachinko ");
        let inner_size = outer_frame.inner(f.size());
        f.render_widget(outer_frame, f.size());

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(inner_size);

        f.render_stateful_widget(
            EditorTable::new(&self.store, Item::q()).columns(vec![
                EditorColumn::new("Location", EditorColumnWidth::Shrink, |store, o| {
                    Ok(Item::try_convert(o.clone(), &store)?
                        .format_with_store(store)?
                        .format_location())
                }),
                EditorColumn::new("Size", EditorColumnWidth::Shrink, |store, o| {
                    Ok(Item::try_convert(o.clone(), &store)?.size)
                }),
                EditorColumn::new("Name", EditorColumnWidth::Expand, |store, o| {
                    Ok(Item::try_convert(o.clone(), &store)?.name)
                }),
            ]),
            chunks[0],
            &mut self.editor_table_state,
        );

        f.render_widget(
            Paragraph::new("Status").block(Block::default().borders(Borders::TOP)),
            chunks[1],
        );
    }

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::Key(e) => match e.code {
                KeyCode::Char('q') => {
                    self.running.store(false, Ordering::SeqCst);
                }
                KeyCode::Down => {
                    self.editor_table_state.move_down();
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

struct EditorColumn {
    header: String,
    display: fn(&Store, &Object) -> AHResult<String>,
    width: EditorColumnWidth,
}

impl EditorColumn {
    fn new(
        header: impl Into<String>,
        width: EditorColumnWidth,
        display: fn(&Store, &Object) -> AHResult<String>,
    ) -> Self {
        Self {
            header: header.into(),
            width,
            display,
        }
    }
}

struct EditorTable<'store> {
    store: &'store Store,
    rows: CachedCollection,
    columns: Vec<EditorColumn>,
}

impl<'store> EditorTable<'store> {
    fn new(store: &'store Store, query: impl Into<QueryNode>) -> Self {
        Self {
            store,
            rows: store.cached_query(query).unwrap(),
            columns: vec![],
        }
    }

    fn columns(mut self, columns: Vec<EditorColumn>) -> Self {
        self.columns = columns;

        self
    }
}

#[derive(Default)]
struct EditorTableState {
    table_state: SheetState,
}

impl EditorTableState {
    fn move_down(&mut self) {
        self.table_state
            .set_offset(self.table_state.get_offset() + 1)
    }
}

impl<'store> StatefulWidget for EditorTable<'store> {
    type State = EditorTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let store = self.store;
        let columns = self.columns;

        let rows: Vec<Vec<_>> = self
            .rows
            .iter()
            .map(|o| {
                columns
                    .iter()
                    .map(|c| (c.display)(store, o).unwrap_or("".to_string()))
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
                .header(Row::new(columns.iter().map(|c| {
                    Span::styled(
                        c.header.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                })))
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
            area,
            buf,
            &mut state.table_state,
        );
    }
}

impl<'store> Widget for EditorTable<'store> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = EditorTableState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
