use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Instant},
    vec,
};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};

use lazy_static::lazy_static;
use qualia::Store;
use tui::{
    backend::Backend,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear},
    Frame,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::types::Item;
use crate::types::ItemSize;

use super::item::{ItemColumn, ItemColumnKind, ItemColumnViewModel, ItemColumnWidth};
use super::sheet::{Row, Sheet, SheetSelection, SheetState};

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
                &["Alt+S", "Save any changes to the current item"],
                &["Alt+Shift+S", "Save all changed items"],
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

    fn reset_idle(&mut self) {
        self.last_action_time = Instant::now();
    }

    fn check_idle(&mut self) -> bool {
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
                        KeyCode::Delete if e.modifiers == KeyModifiers::ALT => {
                            if let Some(row) = self.sheet_state.selection().row() {
                                let item_name =
                                    self.item_column_view_model.delete_item(row).unwrap();
                                self.action_description =
                                    Some((Instant::now(), format!("deleted: {}", item_name)));
                            }
                        }
                        KeyCode::Char('s')
                            if e.modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT =>
                        {
                            let count =
                                self.item_column_view_model.persist_pending_edits().unwrap();
                            self.action_description =
                                Some((Instant::now(), format!("saved {} changes", count)));
                        }
                        KeyCode::Char('s') if e.modifiers == KeyModifiers::ALT => {
                            if let Some(row) = self.sheet_state.selection().row() {
                                if let Some(item_name) = self
                                    .item_column_view_model
                                    .persist_current_pending_edit(row)
                                    .unwrap()
                                {
                                    self.action_description =
                                        Some((Instant::now(), format!("saved: {}", item_name)));
                                }
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
            Char(r, c, _) => Char(r, c, 0),
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
}
