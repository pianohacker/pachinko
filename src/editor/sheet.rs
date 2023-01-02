// Based on `table.rs` from `tui`, under the following license:
//
// The MIT License (MIT)
//
// Copyright (c) 2016 Florian Dehau
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Text,
    widgets::{Block, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

/// A [`Cell`] contains the [`Text`] to be displayed in a [`Row`] of a [`Sheet`].
///
/// It can be created from anything that can be converted to a [`Text`].
/// ```rust
/// # use tui::widgets::Cell;
/// # use tui::style::{Style, Modifier};
/// # use tui::text::{Span, Spans, Text};
/// # use std::borrow::Cow;
/// Cell::from("simple string");
///
/// Cell::from(Span::from("span"));
///
/// Cell::from(Spans::from(vec![
///     Span::raw("a vec of "),
///     Span::styled("spans", Style::default().add_modifier(Modifier::BOLD))
/// ]));
///
/// Cell::from(Text::from("a text"));
///
/// Cell::from(Text::from(Cow::Borrowed("hello")));
/// ```
///
/// You can apply a [`Style`] on the entire [`Cell`] using [`Cell::style`] or rely on the styling
/// capabilities of [`Text`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a> Cell<'a> {
    /// Set the `Style` of this cell.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<'a, T> From<T> for Cell<'a>
where
    T: Into<Text<'a>>,
{
    fn from(content: T) -> Cell<'a> {
        Cell {
            content: content.into(),
            style: Style::default(),
        }
    }
}

/// Holds data to be displayed in a [`Sheet`] widget.
///
/// A [`Row`] is a collection of cells. It can be created from simple strings:
/// ```rust
/// # use tui::widgets::Row;
/// Row::new(vec!["Cell1", "Cell2", "Cell3"]);
/// ```
///
/// But if you need a bit more control over individual cells, you can explicity create [`Cell`]s:
/// ```rust
/// # use tui::widgets::{Row, Cell};
/// # use tui::style::{Style, Color};
/// Row::new(vec![
///     Cell::from("Cell1"),
///     Cell::from("Cell2").style(Style::default().fg(Color::Yellow)),
/// ]);
/// ```
///
/// You can also construct a row from any type that can be converted into [`Text`]:
/// ```rust
/// # use std::borrow::Cow;
/// # use tui::widgets::Row;
/// Row::new(vec![
///     Cow::Borrowed("hello"),
///     Cow::Owned("world".to_uppercase()),
/// ]);
/// ```
///
/// By default, a row has a height of 1 but you can change this using [`Row::height`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row<'a> {
    cells: Vec<Cell<'a>>,
    height: u16,
    style: Style,
    bottom_margin: u16,
}

impl<'a> Row<'a> {
    /// Creates a new [`Row`] from an iterator where items can be converted to a [`Cell`].
    pub fn new<T>(cells: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<Cell<'a>>,
    {
        Self {
            height: 1,
            cells: cells.into_iter().map(|c| c.into()).collect(),
            style: Style::default(),
            bottom_margin: 0,
        }
    }

    /// Set the fixed height of the [`Row`]. Any [`Cell`] whose content has more lines than this
    /// height will see its content truncated.
    pub fn height(mut self, height: u16) -> Self {
        self.height = height;
        self
    }

    /// Set the [`Style`] of the entire row. This [`Style`] can be overriden by the [`Style`] of a
    /// any individual [`Cell`] or event by their [`Text`] content.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the bottom margin. By default, the bottom margin is `0`.
    pub fn bottom_margin(mut self, margin: u16) -> Self {
        self.bottom_margin = margin;
        self
    }

    /// Returns the total height of the row.
    fn total_height(&self) -> u16 {
        self.height.saturating_add(self.bottom_margin)
    }
}

/// A widget to display data in formatted columns.
///
/// It is a collection of [`Row`]s, themselves composed of [`Cell`]s:
/// ```rust
/// # use tui::widgets::{Block, Borders, Sheet, Row, Cell};
/// # use tui::layout::Constraint;
/// # use tui::style::{Style, Color, Modifier};
/// # use tui::text::{Text, Spans, Span};
/// Sheet::new(vec![
///     // Row can be created from simple strings.
///     Row::new(vec!["Row11", "Row12", "Row13"]),
///     // You can style the entire row.
///     Row::new(vec!["Row21", "Row22", "Row23"]).style(Style::default().fg(Color::Blue)),
///     // If you need more control over the styling you may need to create Cells directly
///     Row::new(vec![
///         Cell::from("Row31"),
///         Cell::from("Row32").style(Style::default().fg(Color::Yellow)),
///         Cell::from(Spans::from(vec![
///             Span::raw("Row"),
///             Span::styled("33", Style::default().fg(Color::Green))
///         ])),
///     ]),
///     // If a Row need to display some content over multiple lines, you just have to change
///     // its height.
///     Row::new(vec![
///         Cell::from("Row\n41"),
///         Cell::from("Row\n42"),
///         Cell::from("Row\n43"),
///     ]).height(2),
/// ])
/// // You can set the style of the entire Sheet.
/// .style(Style::default().fg(Color::White))
/// // It has an optional header, which is simply a Row always visible at the top.
/// .header(
///     Row::new(vec!["Col1", "Col2", "Col3"])
///         .style(Style::default().fg(Color::Yellow))
///         // If you want some space between the header and the rest of the rows, you can always
///         // specify some margin at the bottom.
///         .bottom_margin(1)
/// )
/// // As any other widget, a Sheet can be wrapped in a Block.
/// .block(Block::default().title("Sheet"))
/// // Columns widths are constrained in the same way as Layout...
/// .widths(&[Constraint::Length(5), Constraint::Length(5), Constraint::Length(10)])
/// // ...and they can be separated by a fixed spacing.
/// .column_spacing(1)
/// // If you wish to highlight a row in any specific way when it is selected...
/// .highlight_style(Style::default().add_modifier(Modifier::BOLD))
/// // ...and potentially show a symbol in front of the selection.
/// .highlight_symbol(">>");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet<'a> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Base style for the widget
    style: Style,
    /// Width constraints for each column
    widths: &'a [Constraint],
    /// Space between each column
    column_spacing: u16,
    /// Style used to render the selected row
    highlight_style: Style,
    /// Style used to render the selected cell
    highlight_cell_style: Style,
    /// Style used to render the character cursor
    highlight_i_style: Style,
    /// Symbol in front of the selected rom
    highlight_symbol: Option<&'a str>,
    /// Optional header
    header: Option<Row<'a>>,
    /// Data to display in each row
    rows: Vec<Row<'a>>,
}

impl<'a> Sheet<'a> {
    pub fn new<T>(rows: T) -> Self
    where
        T: IntoIterator<Item = Row<'a>>,
    {
        Self {
            block: None,
            style: Style::default(),
            widths: &[],
            column_spacing: 1,
            highlight_style: Style::default(),
            highlight_cell_style: Style::default(),
            highlight_i_style: Style::default(),
            highlight_symbol: None,
            header: None,
            rows: rows.into_iter().collect(),
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn header(mut self, header: Row<'a>) -> Self {
        self.header = Some(header);
        self
    }

    pub fn widths(mut self, widths: &'a [Constraint]) -> Self {
        let between_0_and_100 = |&w| match w {
            Constraint::Percentage(p) => p <= 100,
            _ => true,
        };
        assert!(
            widths.iter().all(between_0_and_100),
            "Percentages should be between 0 and 100 inclusively."
        );
        self.widths = widths;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn highlight_symbol(mut self, highlight_symbol: &'a str) -> Self {
        self.highlight_symbol = Some(highlight_symbol);
        self
    }

    pub fn highlight_style(mut self, highlight_style: Style) -> Self {
        self.highlight_style = highlight_style;
        self
    }

    pub fn highlight_cell_style(mut self, highlight_cell_style: Style) -> Self {
        self.highlight_cell_style = highlight_cell_style;
        self
    }

    pub fn highlight_i_style(mut self, highlight_i_style: Style) -> Self {
        self.highlight_i_style = highlight_i_style;
        self
    }

    pub fn column_spacing(mut self, spacing: u16) -> Self {
        self.column_spacing = spacing;
        self
    }

    fn get_columns_widths(&self, max_width: u16, has_selection: bool) -> Vec<u16> {
        let mut constraints = Vec::with_capacity(self.widths.len() * 2 + 1);
        if has_selection {
            let highlight_symbol_width =
                self.highlight_symbol.map(|s| s.width() as u16).unwrap_or(0);
            constraints.push(Constraint::Length(highlight_symbol_width));
        }
        for constraint in self.widths {
            constraints.push(*constraint);
            constraints.push(Constraint::Length(self.column_spacing));
        }
        if !self.widths.is_empty() {
            constraints.pop();
        }
        let mut chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                constraints
                    .into_iter()
                    .chain(std::iter::once(Constraint::Length(0)))
                    .collect::<Vec<_>>(),
            )
            .split(Rect {
                x: 0,
                y: 0,
                width: max_width,
                height: 1,
            });
        if has_selection {
            chunks.remove(0);
        }
        chunks.remove(chunks.len() - 1);
        chunks.iter().step_by(2).map(|c| c.width).collect()
    }

    fn get_row_bounds(
        &self,
        selected: Option<usize>,
        offset: usize,
        max_height: u16,
    ) -> (usize, usize) {
        let offset = offset.min(self.rows.len().saturating_sub(1));
        let mut start = offset;
        let mut end = offset;
        let mut height = 0;
        for item in self.rows.iter().skip(offset) {
            if height + item.height > max_height {
                break;
            }
            height += item.total_height();
            end += 1;
        }

        let selected = selected.unwrap_or(offset).min(self.rows.len() - 1);
        while selected >= end {
            height = height.saturating_add(self.rows[end].total_height());
            end += 1;
            while height > max_height {
                height = height.saturating_sub(self.rows[start].total_height());
                start += 1;
            }
        }
        while selected < start {
            start -= 1;
            height = height.saturating_add(self.rows[start].total_height());
            while height > max_height {
                end -= 1;
                height = height.saturating_sub(self.rows[end].total_height());
            }
        }
        (start, end)
    }
}

#[derive(Copy, Debug, Clone)]
pub enum SheetSelection {
    None,
    Row(usize),
    Cell(usize, usize),
    Char(usize, usize, usize),
}

impl SheetSelection {
    pub fn is_none(&self) -> bool {
        match *self {
            Self::None => true,
            _ => false,
        }
    }

    pub fn is_some(&self) -> bool {
        match *self {
            Self::None => false,
            _ => true,
        }
    }

    pub fn row(&self) -> Option<usize> {
        match *self {
            Self::None => None,
            Self::Row(r) | Self::Cell(r, _) | Self::Char(r, _, _) => Some(r),
        }
    }

    pub fn column(&self) -> Option<usize> {
        match *self {
            Self::None | Self::Row(_) => None,
            Self::Cell(_, c) | Self::Char(_, c, _) => Some(c),
        }
    }

    pub fn i(&self) -> Option<usize> {
        match *self {
            Self::Char(_, _, i) => Some(i),
            _ => None,
        }
    }

    pub fn with_row(self, row: usize) -> Self {
        match self {
            Self::None | Self::Row(_) => Self::Row(row),
            Self::Cell(_, c) => Self::Cell(row, c),
            Self::Char(_, c, i) => Self::Char(row, c, i),
        }
    }

    pub fn map_row(self, f: impl FnOnce(usize) -> usize) -> Self {
        match self {
            Self::None => self,
            Self::Row(r) => Self::Row(f(r)),
            Self::Cell(r, c) => Self::Cell(f(r), c),
            Self::Char(r, c, i) => Self::Char(f(r), c, i),
        }
    }

    pub fn map_row_or(self, default: usize, f: impl FnOnce(usize) -> usize) -> Self {
        match self {
            Self::None => Self::Row(default),
            Self::Row(r) => Self::Row(f(r)),
            Self::Cell(r, c) => Self::Cell(f(r), c),
            Self::Char(r, c, i) => Self::Char(f(r), c, i),
        }
    }

    fn normalize(&mut self, width: usize, height: usize) {
        *self = match *self {
            Self::None => Self::None,
            Self::Row(r) => Self::Row(r.min(height)),
            Self::Cell(r, c) => Self::Cell(r.min(height), c.min(width)),
            Self::Char(r, c, i) => Self::Char(r.min(height), c.min(width), i),
        };
    }

    fn normalize_char_position(&mut self, cell_len: Option<usize>) {
        *self = match *self {
            Self::None | Self::Row(_) | Self::Cell(_, _) => *self,
            Self::Char(r, c, i) => match cell_len {
                Some(l) => Self::Char(r, c, l.min(i)),
                None => Self::Cell(r, c),
            },
        };
    }
}

impl Default for SheetSelection {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Default)]
pub struct SheetState {
    offset: usize,
    selection: SheetSelection,
    last_rows_height: Option<u16>,
}

impl SheetState {
    pub fn selection(&self) -> SheetSelection {
        self.selection
    }

    pub fn select(&mut self, selection: SheetSelection) {
        self.selection = selection;
        if selection.is_none() {
            self.offset = 0;
        }
    }

    pub fn map_selection(&mut self, f: impl FnOnce(SheetSelection) -> SheetSelection) {
        self.select(f(self.selection));
    }

    pub fn get_offset(&self) -> usize {
        self.offset
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.selection = self.selection.with_row(offset);
        self.offset = offset;
    }

    pub fn scroll_up(&mut self, delta: usize) {
        self.offset = self.offset.saturating_sub(delta);

        if let Some(last_rows_height) = self.last_rows_height {
            self.selection = self
                .selection
                .map_row(|r| r.min(self.offset + last_rows_height as usize - 1));
        }
    }

    pub fn scroll_down(&mut self, delta: usize) {
        self.offset += delta;
        self.selection = self.selection.map_row(|r| r.max(self.offset));
    }
}

impl<'a> StatefulWidget for Sheet<'a> {
    type State = SheetState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.area() == 0 {
            return;
        }
        buf.set_style(area, self.style);
        let table_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        let has_selection = state.selection.is_some();
        let columns_widths = self.get_columns_widths(table_area.width, has_selection);
        let highlight_symbol = self.highlight_symbol.unwrap_or("");
        let blank_symbol = " ".repeat(highlight_symbol.width());
        let mut current_height = 0;
        let mut rows_height = table_area.height;

        // Draw header
        if let Some(ref header) = self.header {
            let max_header_height = table_area.height.min(header.total_height());
            buf.set_style(
                Rect {
                    x: table_area.left(),
                    y: table_area.top(),
                    width: table_area.width,
                    height: table_area.height.min(header.height),
                },
                header.style,
            );
            let mut col = table_area.left();
            if has_selection {
                col += (highlight_symbol.width() as u16).min(table_area.width);
            }
            for (width, cell) in columns_widths.iter().zip(header.cells.iter()) {
                render_cell(
                    buf,
                    cell,
                    Rect {
                        x: col,
                        y: table_area.top(),
                        width: *width,
                        height: max_header_height,
                    },
                    None,
                    None,
                );
                col += *width + self.column_spacing;
            }
            current_height += max_header_height;
            rows_height = rows_height.saturating_sub(max_header_height);
        }

        // Draw rows
        if self.rows.is_empty() {
            return;
        }

        state
            .selection
            .normalize(self.widths.len() - 1, self.rows.len() - 1);

        let highlight_cell_style = self.highlight_style.patch(self.highlight_cell_style);
        let highlight_i_style = self.highlight_cell_style.patch(self.highlight_i_style);

        let (start, end) = self.get_row_bounds(state.selection.row(), state.offset, rows_height);
        state.last_rows_height = Some(rows_height);
        state.offset = start;
        for (i, table_row) in self
            .rows
            .iter_mut()
            .enumerate()
            .skip(state.offset)
            .take(end - start)
        {
            let (row, col) = (table_area.top() + current_height, table_area.left());
            current_height += table_row.total_height();
            let table_row_area = Rect {
                x: col,
                y: row,
                width: table_area.width,
                height: table_row.height,
            };
            buf.set_style(table_row_area, table_row.style);
            let is_selected = state.selection.row().map(|s| s == i).unwrap_or(false);
            let table_row_start_col = if has_selection {
                let symbol = if is_selected {
                    highlight_symbol
                } else {
                    &blank_symbol
                };
                let (col, _) =
                    buf.set_stringn(col, row, symbol, table_area.width as usize, table_row.style);
                col
            } else {
                col
            };

            let mut col = table_row_start_col;
            if is_selected {
                buf.set_style(table_row_area, self.highlight_style);
            }
            for (j, (width, cell)) in columns_widths
                .iter()
                .zip(table_row.cells.iter())
                .enumerate()
            {
                render_cell(
                    buf,
                    cell,
                    Rect {
                        x: col,
                        y: row,
                        width: *width,
                        height: table_row.height,
                    },
                    if is_selected && state.selection.column() == Some(j) {
                        Some(highlight_cell_style)
                    } else {
                        None
                    },
                    if is_selected
                        && state.selection.column() == Some(j)
                        && state.selection.i().is_some()
                    {
                        Some((state.selection.i().unwrap(), highlight_i_style))
                    } else {
                        None
                    },
                );
                col += *width + self.column_spacing;
            }
        }
    }
}

fn render_cell(
    buf: &mut Buffer,
    cell: &Cell,
    area: Rect,
    highlight_style: Option<Style>,
    cursor_highlight: Option<(usize, Style)>,
) {
    buf.set_style(
        area,
        highlight_style.map_or(cell.style, |hs| cell.style.patch(hs)),
    );
    for (i, spans) in cell.content.lines.iter().enumerate() {
        if i as u16 >= area.height {
            break;
        }
        buf.set_spans(area.x, area.y + i as u16, spans, area.width);
    }

    if let Some((i, cursor_style)) = cursor_highlight {
        let cursor_rect = Rect::new(area.x + i as u16, area.y, 1, 1);

        if cursor_rect.intersects(area) {
            buf.set_style(cursor_rect, cursor_style);
        }
    }
}

impl<'a> Widget for Sheet<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = SheetState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn sheet_invalid_percentages() {
        Sheet::new(vec![]).widths(&[Constraint::Percentage(110)]);
    }
}
