// Copyright (c) 2020 Jesse Weaver.
//
// This file is part of pachinko.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

mod types;

use anyhow::{anyhow, bail, Context, Result as AHResult};
use clap::{AppSettings, Clap};
use qualia::object;
use qualia::{Object, Store, Q};
use rustyline::Editor;
use shell_words;
use std::collections::HashMap;
use std::env;

use crate::types::{parse_bin_number, Item, ItemLocation, ItemSize, Location};

#[derive(Clap)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Add an item", visible_alias = "a")]
    Add(AddOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Add a location")]
    AddLocation(AddLocationOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Run several commands from an interactive console", visible_alias = "c")]
    Console(CommonOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Delete an item", visible_alias = "d")]
    Delete(DeleteOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Show existing items", visible_alias = "i")]
    Items(ItemsOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Show existing locations")]
    Locations(CommonOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Quickly add several items to a location", visible_alias = "qa")]
    Quickadd(QuickaddOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Undo the last action", visible_alias = "u")]
    Undo(CommonOpts),
}

impl SubCommand {
    fn invoke(self) -> AHResult<()> {
        match self {
            SubCommand::Add(o) => run_add(o),
            SubCommand::AddLocation(o) => run_add_location(o),
            SubCommand::Delete(o) => run_delete(o),
            SubCommand::Console(o) => run_console(o),
            SubCommand::Items(o) => run_items(o),
            SubCommand::Locations(o) => run_locations(o),
            SubCommand::Quickadd(o) => run_quickadd(o),
            SubCommand::Undo(o) => run_undo(o),
        }
    }
}

#[derive(Clap, Debug)]
struct CommonOpts {
    #[clap(long, env = "PACHINKO_STORE_PATH")]
    store_path: Option<String>,
}

impl CommonOpts {
    fn open_store(&self) -> AHResult<Store> {
        let store_path = match &self.store_path {
            Some(s) => s.clone(),
            None => {
                let data_dir_path = dirs::data_dir()
                    .ok_or(anyhow!(
                        "Could not determine your home directory; is $HOME set?"
                    ))?
                    .join("pachinko");

                if !data_dir_path.is_dir() {
                    std::fs::create_dir_all(&data_dir_path)?;
                }

                format!("{}/pachinko.qualia", data_dir_path.to_str().unwrap(),)
            }
        };

        Store::open(store_path).context("failed to open store")
    }
}

trait WithCommonOpts {
    fn common_opts(&self) -> &CommonOpts;
}

#[derive(Clap)]
struct AddOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap()]
    location: ItemLocation,
    #[clap()]
    name: String,
    #[clap(arg_enum, default_value = "S")]
    size: ItemSize,
}

impl WithCommonOpts for AddOpts {
    fn common_opts(&self) -> &CommonOpts {
        &self.common
    }
}

fn _resolve_location(store: &Store, location: &ItemLocation) -> AHResult<Location> {
    let matching_locations = store.query(
        Q.equal("type", "location")
            .like("name", location.location.clone()),
    );

    if matching_locations.len()? != 1 {
        bail!(
            "location name \"{}\" did not match exactly one location",
            location.location
        );
    }

    Ok(matching_locations.iter_as()?.next().unwrap())
}

fn _choose_bin(store: &Store, location_id: i64, num_bins: i64) -> AHResult<i64> {
    let all_location_items = store.query(Q.equal("type", "item").equal("location_id", location_id));

    let mut bin_fullnesses: HashMap<i64, i64> = (1..=num_bins).map(|bin_no| (bin_no, 0)).collect();
    all_location_items
        .iter_as::<Item>()?
        .try_for_each(|item| -> AHResult<()> {
            let size: ItemSize = item.size.parse::<ItemSize>()?;

            *bin_fullnesses.get_mut(&item.bin_no).unwrap() += i64::from(size);

            Ok(())
        })?;

    let min_fullness = bin_fullnesses
        .iter()
        .map(|(_, fullness)| fullness)
        .min()
        .unwrap_or(&0);

    Ok((1..=num_bins)
        .filter_map(|bin_no| {
            if bin_fullnesses[&bin_no] <= *min_fullness {
                Some(bin_no)
            } else {
                None
            }
        })
        .next()
        .unwrap())
}

fn _add_item(
    store: &mut Store,
    name: String,
    location: &Location,
    bin_no: Option<i64>,
    size: ItemSize,
) -> AHResult<()> {
    let bin_number = match bin_no {
        Some(n) => {
            if n > location.num_bins {
                bail!(
                    "location {} only has {} bins",
                    location.name,
                    location.num_bins
                );
            }
            n
        }
        None => _choose_bin(&store, location.id, location.num_bins)?,
    };

    let checkpoint = store.checkpoint()?;
    checkpoint.add(object!(
        "type" => "item",
        "name" => (&name),
        "location_id" => location.id,
        "bin_no" => bin_number,
        "size" => size.to_string(),
    ))?;
    checkpoint.commit(format!("add item {}", name))?;

    println!(
        "{}",
        Item {
            location_id: location.id,
            bin_no: bin_number,
            name,
            size: size.to_string(),
        }
        .format_with_store(store)?
    );

    Ok(())
}

fn run_add(opts: AddOpts) -> AHResult<()> {
    let mut store = opts.common.open_store()?;

    // eprintln!("{:#?}", store.all().iter()?.collect::<Vec<Object>>());

    let location = _resolve_location(&store, &opts.location)?;

    _add_item(
        &mut store,
        opts.name,
        &location,
        opts.location.bin,
        opts.size,
    )?;

    Ok(())
}

#[derive(Clap)]
struct AddLocationOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap()]
    name: String,
    #[clap(parse(from_str = parse_bin_number))]
    num_bins: AHResult<i64>,
}

impl WithCommonOpts for AddLocationOpts {
    fn common_opts(&self) -> &CommonOpts {
        &self.common
    }
}

fn run_add_location(opts: AddLocationOpts) -> AHResult<()> {
    let mut store = opts.common.open_store()?;

    let checkpoint = store.checkpoint()?;
    checkpoint.add(object!(
        "type" => "location",
        "name" => &opts.name,
        "num_bins" => opts.num_bins?,
    ))?;
    checkpoint.commit(format!("add location {}", &opts.name))?;

    Ok(())
}

fn _format_items(
    store: &Store,
    items: &qualia::Collection,
) -> AHResult<impl Iterator<Item = impl std::fmt::Display>> {
    let mut formatted_items = items
        .iter_as::<Item>()?
        .map(|item| item.format_with_store(store))
        .collect::<AHResult<Vec<_>>>()?;
    formatted_items.sort();

    Ok(formatted_items.into_iter())
}

#[derive(Clap, Debug)]
struct ItemsOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap()]
    name_pattern: Option<String>,
}

impl WithCommonOpts for ItemsOpts {
    fn common_opts(&self) -> &CommonOpts {
        &self.common
    }
}

fn run_items(opts: ItemsOpts) -> AHResult<()> {
    let store = opts.common_opts().open_store()?;

    let mut query = Q.equal("type", "item");

    if let Some(name_pattern) = opts.name_pattern {
        query = query.like("name", &name_pattern);
    }

    for formatted_item in _format_items(&store, &store.query(query))? {
        println!("{}", formatted_item);
    }

    Ok(())
}

#[derive(Clap)]
#[clap(setting = AppSettings::NoBinaryName)]
struct ConsoleOpts {
    #[clap(subcommand)]
    subcmd: ConsoleSubCommand,
}

#[derive(Clap)]
enum ConsoleSubCommand {
    #[clap(flatten)]
    Base(SubCommand),

    #[clap(about = "Quit the console")]
    Quit,
}

fn run_console(opts: CommonOpts) -> AHResult<()> {
    // Make sure the store can be opened before we try to run any commands.
    opts.open_store().unwrap();

    let mut rl = Editor::<()>::new();

    while let Ok(line) = rl.readline("pachinko> ") {
        let continue_console = || -> AHResult<bool> {
            let words = shell_words::split(&line)?;

            if words[0] == "help" {
                <ConsoleOpts as clap::IntoApp>::into_app()
                    .help_template("Available commands:\n{subcommands}")
                    .print_help()?;

                return Ok(true);
            }

            let console_opts = ConsoleOpts::try_parse_from(words)?;

            match console_opts.subcmd {
                ConsoleSubCommand::Quit => Ok(false),
                ConsoleSubCommand::Base(SubCommand::Console(_)) => Ok(true),
                ConsoleSubCommand::Base(sc) => sc.invoke().map(|_| true),
            }
        }()
        .unwrap_or_else(|e| {
            println!("Error: {}", e);

            true
        });

        if !continue_console {
            break;
        }
    }

    Ok(())
}

#[derive(Clap)]
struct DeleteOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap(short, long)]
    all: bool,
    #[clap()]
    name_pattern: String,
}

impl WithCommonOpts for DeleteOpts {
    fn common_opts(&self) -> &CommonOpts {
        &self.common
    }
}

fn run_delete(opts: DeleteOpts) -> AHResult<()> {
    let mut store = opts.common.open_store()?;

    let checkpoint = store.checkpoint()?;
    let matching_items = checkpoint.query(Q.equal("type", "item").like("name", &opts.name_pattern));

    if matching_items.len()? > 1 && !opts.all {
        let formatted_items: Vec<_> = _format_items(&checkpoint, &matching_items)?
            .map(|item| format!("    {}", item))
            .collect();

        bail!(
            "found multiple matching items (use --all to delete multiple items):\n{}",
            formatted_items.join("\n")
        );
    }

    for formatted_item in _format_items(&checkpoint, &matching_items)? {
        println!("Deleted {}", formatted_item);
    }

    matching_items.delete()?;

    checkpoint.commit(format!("delete items matching {}", &opts.name_pattern))?;

    Ok(())
}

fn run_locations(opts: CommonOpts) -> AHResult<()> {
    let store = opts.open_store()?;

    for location in store
        .query(Q.equal("type", "location"))
        .iter_as::<Location>()?
    {
        if location.num_bins > 1 {
            println!("{} ({} bins)", location.name, location.num_bins);
        } else {
            println!("{}", location.name);
        }
    }

    Ok(())
}

#[derive(Clap)]
struct QuickaddOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap()]
    location: ItemLocation,
}

fn run_quickadd(opts: QuickaddOpts) -> AHResult<()> {
    let mut store = opts.common.open_store()?;

    // eprintln!("{:#?}", store.all().iter()?.collect::<Vec<Object>>());

    let location = _resolve_location(&store, &opts.location)?;

    let bin_number_display = match opts.location.bin {
        Some(bin_no) => format!("/{}", bin_no),
        None => "".to_string(),
    };
    let prompt = location.name.clone() + &bin_number_display + "> ";

    let mut rl = Editor::<()>::new();

    while let Ok(line) = rl.readline(&prompt) {
        let mut name = line.trim().to_string();
        let mut size = ItemSize::S;

        if let Some(cap) = regex::Regex::new(r"^(.*?)\s+([SMLX])$")?.captures(line.trim()) {
            name = cap[1].to_string();
            size = cap[2].parse()?;
        }

        _add_item(
            &mut store,
            name.to_string(),
            &location,
            opts.location.bin,
            size,
        )?;
    }

    Ok(())
}

fn run_undo(opts: CommonOpts) -> AHResult<()> {
    let mut store = opts.open_store()?;

    match store.undo()? {
        Some(description) => println!("Undid: {}", description),
        None => println!("Nothing to undo"),
    }

    Ok(())
}

fn main() -> AHResult<()> {
    Opts::parse().subcmd.invoke()
}
