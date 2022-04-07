use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crossterm::event::{Event, KeyCode};
use qualia::{PropValue, Store, Q};
use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Widget},
    Frame,
};

use crate::types::Item;

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

        f.render_widget(StoreTable::new(&self.store), chunks[0]);

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

struct EditorColumn {
    header: String,
    display: fn(&Store, &Item) -> String,
}

struct StoreTable<'store> {
    store: &'store Store,
}

impl<'store> StoreTable<'store> {
    fn new(store: &'store Store) -> Self {
        Self { store }
    }

    fn columns(&self) -> Vec<String> {
        let mut result = self
            .store
            .query(Q.equal("type", "item"))
            .iter()
            .expect("query to succeed")
            .flat_map(|i| i.keys().map(|k| k.clone()).collect::<Vec<_>>())
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|k| k != "location_id" && k != "object-id" && k != "item")
            .collect::<Vec<_>>();

        result.sort();
        result
    }
}

impl<'store> Widget for StoreTable<'store> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let columns = self.columns();

        Table::new(
            self.store
                .query(Q.equal("type", "item"))
                .iter()
                .unwrap()
                .map(|i| {
                    Row::new(columns.iter().map(|k| {
                        match i.get(k).unwrap_or(&PropValue::String("".to_string())) {
                            PropValue::String(s) => s.clone(),
                            PropValue::Number(i) => format!("{}", i),
                        }
                    }))
                }),
        )
        .header(Row::new(columns.clone()))
        .widths(
            &columns
                .iter()
                .map(|_| Constraint::Ratio(1, columns.len() as u32))
                .collect::<Vec<_>>(),
        )
        .render(area, buf);
    }
}
