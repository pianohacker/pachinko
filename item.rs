fn render_item(columns: &Vec<ItemColumn>, item: &Item) -> Vec<String> {
    columns
        .iter()
        .enumerate()
        .map(|(_, c)| (c.display)(item).unwrap_or("".into()))
        .collect()
}

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
            .map(|(id, item)| {
                let column_contents = render_item(self.columns, item);
                let column_widths = column_contents
                    .iter()
                    .map(|text| text.graphemes(true).count())
                    .collect::<Vec<_>>();

                (
                    *id,
                    ItemRenderEntry {
                        item: item.clone(),
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

    fn add_item(&mut self, after_index: usize, item: &Item) {
        let column_contents: Vec<_> = render_item(self.columns, item);

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

        self.entries.move_index(inserted_index, after_index + 1);
    }

    fn edit_item<T>(&mut self, index: usize, editor: impl FnOnce(&mut Item) -> T) -> (i64, T) {
        let (object_id, entry) = self.entries.get_index_mut(index).unwrap();

        let value = editor(&mut entry.item);
        entry.contents = Row::new(render_item(self.columns, &entry.item));

        (*object_id, value)
    }
}

struct ItemColumnViewModel<'columns, 'row> {
    store: Store,
    last_fetched_items: IndexMap<i64, Item>,
    columns: &'columns Vec<ItemColumn>,
    last_updated_checkpoint: CheckpointId,
    last_rendered_set: ItemColumnRenderedSet<'columns, 'row>,
    edited_items: HashSet<i64>,
}

impl<'columns, 'row> ItemColumnViewModel<'columns, 'row> {
    fn new(store: Store, columns: &'columns Vec<ItemColumn>) -> Self {
        Self {
            store,
            columns,
            last_fetched_items: IndexMap::new(),
            last_updated_checkpoint: 0,
            last_rendered_set: ItemColumnRenderedSet::new(&columns),
            edited_items: HashSet::new(),
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
        self.refresh_if_needed()?;
        self.last_rendered_set.regenerate_if_needed(
            &self.last_fetched_items,
            self.last_updated_checkpoint,
            search.clone(),
        );

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

        self.last_rendered_set.add_item(after_index, &item);

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

        let (object_id, new_cursor) = self
            .last_rendered_set
            .edit_item(row, |item| column_insert_char(item, i, c));

        self.edited_items.insert(object_id);

        new_cursor
    }

    fn delete_char(&mut self, row: usize, cell: usize, i: usize) {
        let column_delete_char = match self.columns[cell].delete_char {
            Some(f) => f,
            None => return,
        };

        let (object_id, _) = self
            .last_rendered_set
            .edit_item(row, |item| column_delete_char(item, i));

        self.edited_items.insert(object_id);
    }

    fn persist_pending_edits(&mut self) -> bool {
        if self.edited_items.len() == 0 {
            return false;
        }

        for object_id in self.edited_items.iter() {
            let edited_item = self.last_rendered_set.entries[object_id].item.clone();
            let edited_item_name = edited_item.name.clone();
            let checkpoint = self.store.checkpoint().unwrap();
            checkpoint
                .query(Item::q().id(*object_id))
                .set(edited_item.into())
                .unwrap();
            checkpoint
                .commit(format!("update item: {}", edited_item_name))
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
