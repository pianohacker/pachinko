use std::{
    convert::TryFrom,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crossterm::event::{Event, KeyCode};
use qualia::{query::QueryNode, Object, ObjectShape, PropValue, Store, Q};
use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Widget},
    Frame,
};

use crate::types::Item;
use crate::AHResult;

pub struct App {
    store: Store,
    running: Arc<AtomicBool>,
}

impl App {
    pub fn new(store: Store, running: Arc<AtomicBool>) -> Self {
        Self { store, running }
    }

    pub fn render_to<B: Backend>(&self, f: &mut Frame<'_, B>) {
        let outer_frame = Block::default().borders(Borders::TOP).title(" Pachinko ");
        let inner_size = outer_frame.inner(f.size());
        f.render_widget(outer_frame, f.size());

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(inner_size);

        f.render_widget(
            EditorTable::new(&self.store).query(Item::q()).columns(vec![
                EditorColumn::new("Location", EditorColumnWidth::Shrink, |store, o| {
                    Ok(Item::try_from(o.clone())?
                        .format_with_store(store)?
                        .format_location())
                }),
                EditorColumn::new("Size", EditorColumnWidth::Shrink, |_, o| {
                    Ok(Item::try_from(o.clone())?.size)
                }),
                EditorColumn::new("Name", EditorColumnWidth::Expand, |_, o| {
                    Ok(Item::try_from(o.clone())?.name)
                }),
            ]),
            chunks[0],
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
    query: QueryNode,
    columns: Vec<EditorColumn>,
}

impl<'store> EditorTable<'store> {
    fn new(store: &'store Store) -> Self {
        Self {
            store,
            query: Q.into(),
            columns: vec![],
        }
    }

    fn query(mut self, query: impl Into<QueryNode>) -> Self {
        self.query = query.into();

        self
    }

    fn columns(mut self, columns: Vec<EditorColumn>) -> Self {
        self.columns = columns;

        self
    }
}

impl<'store> Widget for EditorTable<'store> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let query = self.query;
        let store = self.store;
        let columns = self.columns;

        let rows: Vec<Vec<_>> = self
            .store
            .query(query)
            .iter()
            .unwrap()
            .map(|o| {
                columns
                    .iter()
                    .map(|c| (c.display)(store, &o).unwrap_or("".to_string()))
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

        Table::new(rows.into_iter().map(|r| Row::new(r)))
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
                        EditorColumnWidth::Shrink => Constraint::Length(column_widths[i] as u16),
                        EditorColumnWidth::Expand => Constraint::Min(column_widths[i] as u16),
                    })
                    .collect::<Vec<_>>(),
            )
            .column_spacing(1)
            .render(area, buf);
    }
}
