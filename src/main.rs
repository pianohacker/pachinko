// Copyright (c) 2020 Jesse Weaver.
//
// This file is part of pachinko.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{anyhow, bail, Context, Result as AHResult};
use clap::Clap;
use qualia::query;
use qualia::{Object, Store};
use std::collections::HashMap;
use std::env;

trait ObjectGetHelpers {
    fn get_str(&self, object_type: &str, field: &str) -> AHResult<String>;
    fn get_number(&self, object_type: &str, field: &str) -> AHResult<i64>;
}

impl ObjectGetHelpers for Object {
    fn get_str(&self, object_type: &str, field: &str) -> AHResult<String> {
        Ok(self
            .get(field)
            .ok_or(anyhow!("{} object missing {}", object_type, field))?
            .as_str()
            .ok_or(anyhow!("{} object's {} not a string", object_type, field))?
            .clone())
    }

    fn get_number(&self, object_type: &str, field: &str) -> AHResult<i64> {
        self.get(field)
            .ok_or(anyhow!("{} object missing {}", object_type, field))?
            .as_number()
            .ok_or(anyhow!("{} object's {} not a number", object_type, field))
    }
}

#[derive(Clap)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Add an item")]
    Add(AddOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Add a location")]
    AddLocation(AddLocationOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Show existing items")]
    Items(CommonOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Show existing locations")]
    Locations(CommonOpts),

    #[clap(version = env!("CARGO_PKG_VERSION"), about = "Quickly add several items to a location")]
    Quickadd(QuickaddOpts),
}

#[derive(Clap)]
struct CommonOpts {
    #[clap(long, env = "PACHINKO_STORE_PATH")]
    store_path: Option<String>,
}

impl CommonOpts {
    fn open_store(&self) -> AHResult<Store> {
        let store_path = match &self.store_path {
            Some(s) => s.clone(),
            None => {
                let data_dir_path = dirs::data_dir().ok_or(anyhow!(
                    "Could not determine your home directory; is $HOME set?"
                ))?;

                format!("{}/pachinko.qualia", data_dir_path.to_str().unwrap(),)
            }
        };

        Store::open(store_path).context("failed to open store")
    }
}

trait WithCommonOpts {
    fn common_opts(&self) -> &CommonOpts;
}

fn parse_bin_number(s: &str) -> AHResult<i64> {
    Ok(s.parse::<i64>()
        .context("failed to parse bin number")
        .and_then(|x| {
            if x > 0 {
                Ok(x)
            } else {
                Err(anyhow!("must be greater than zero"))
            }
        })?)
}

struct ItemLocation {
    location: String,
    bin: Option<i64>,
}

impl std::str::FromStr for ItemLocation {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> AHResult<Self> {
        let parts: Vec<&str> = s.split("/").collect();

        match parts.len() {
            1 => Ok(Self {
                location: parts[0].to_string(),
                bin: None,
            }),
            2 => {
                let bin_number = parse_bin_number(parts[1])?;

                Ok(Self {
                    location: parts[0].to_string(),
                    bin: Some(bin_number),
                })
            }
            _ => {
                bail!("item location must be in format LOCATION or LOCATION/BIN");
            }
        }
    }
}

#[derive(Clap)]
#[clap(rename_all = "screaming_snake")]
enum ItemSizeOpt {
    S,
    M,
    L,
    X,
}

impl std::str::FromStr for ItemSizeOpt {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> AHResult<Self> {
        match s {
            "S" => Ok(ItemSizeOpt::S),
            "M" => Ok(ItemSizeOpt::M),
            "L" => Ok(ItemSizeOpt::L),
            "X" => Ok(ItemSizeOpt::X),
            _ => Err(anyhow!("attempt to convert size from not \"[SMLX]\"")),
        }
    }
}

impl ToString for ItemSizeOpt {
    fn to_string(&self) -> std::string::String {
        match self {
            ItemSizeOpt::S => "S",
            ItemSizeOpt::M => "M",
            ItemSizeOpt::L => "L",
            ItemSizeOpt::X => "X",
        }
        .to_string()
    }
}

impl From<ItemSizeOpt> for i64 {
    fn from(size: ItemSizeOpt) -> i64 {
        match size {
            ItemSizeOpt::S => 2,
            ItemSizeOpt::M => 3,
            ItemSizeOpt::L => 4,
            ItemSizeOpt::X => 6,
        }
    }
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
    size: ItemSizeOpt,
}

impl WithCommonOpts for AddOpts {
    fn common_opts(&self) -> &CommonOpts {
        &self.common
    }
}

fn _resolve_location(store: &Store, location: &ItemLocation) -> AHResult<Object> {
    let matching_locations = store.query(Box::new(query::And(vec![
        Box::new(query::PropEqual {
            name: "type".to_string(),
            value: "location".into(),
        }),
        Box::new(query::PropLike {
            name: "name".to_string(),
            value: location.location.clone().into(),
        }),
    ])));

    if matching_locations.len()? != 1 {
        bail!(
            "location name \"{}\" did not match exactly one location",
            location.location
        );
    }

    Ok(matching_locations.iter()?.next().unwrap())
}

fn _choose_bin(store: &Store, location_id: i64, num_bins: i64) -> AHResult<i64> {
    let all_location_items = store.query(Box::new(query::And(vec![
        Box::new(query::PropEqual {
            name: "type".to_string(),
            value: "item".into(),
        }),
        Box::new(query::PropEqual {
            name: "location_id".to_string(),
            value: location_id.into(),
        }),
    ])));

    let mut bin_fullnesses: HashMap<i64, i64> = (1..=num_bins).map(|bin_no| (bin_no, 0)).collect();
    all_location_items
        .iter()?
        .try_for_each(|item| -> AHResult<()> {
            let bin_number = item.get_number("item", "bin_no")?;
            let size: ItemSizeOpt = item.get_str("item", "size")?.parse::<ItemSizeOpt>()?;

            *bin_fullnesses.get_mut(&bin_number).unwrap() += i64::from(size);

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
    location: &Object,
    bin_no: Option<i64>,
    size: ItemSizeOpt,
) -> AHResult<()> {
    let num_bins = location.get_number("location", "num_bins")?;

    let bin_number = match bin_no {
        Some(n) => {
            if n > num_bins {
                bail!(
                    "location {} only has {} bins",
                    location.get_str("location", "name")?,
                    num_bins
                );
            }
            n
        }
        None => _choose_bin(
            &store,
            location.get_number("location", "object-id")?,
            num_bins,
        )?,
    };

    let location_id = location.get_number("location", "object-id")?;

    let mut item = Object::new();
    item.insert("type".to_string(), "item".to_string().into());
    item.insert("name".to_string(), (&name).into());
    item.insert("location_id".to_string(), location_id.into());
    item.insert("bin_no".to_string(), bin_number.into());
    item.insert("size".to_string(), size.to_string().into());

    store.add(item)?;

    println!(
        "{}/{}: {} ({})",
        location.get_str("location", "name")?,
        bin_number,
        name,
        size.to_string(),
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

    let mut location = Object::new();
    location.insert("type".to_string(), "location".to_string().into());
    location.insert("name".to_string(), opts.name.into());
    location.insert("num_bins".to_string(), opts.num_bins?.into());

    store.add(location)?;

    Ok(())
}

fn run_items(opts: CommonOpts) -> AHResult<()> {
    let store = opts.open_store()?;

    let mut items = store
        .query(Box::new(query::PropEqual {
            name: "type".to_string(),
            value: "item".into(),
        }))
        .iter()?
        .map(|item| {
            let matching_locations = store.query(Box::new(query::PropEqual {
                name: "object-id".to_string(),
                value: item.get_number("item", "location_id")?.into(),
            }));

            if matching_locations.len()? != 1 {
                bail!(
                    "location id \"{}\" did not match exactly one location",
                    item.get_number("item", "location_id")?
                );
            }

            let location = matching_locations.iter()?.next().unwrap();

            Ok((
                location.get_str("location", "name")?,
                item.get_number("item", "bin_no")?,
                item.get_str("item", "name")?,
                item.get_str("item", "size")?,
            ))
        })
        .collect::<AHResult<Vec<(_, _, _, _)>>>()?;

    items.sort();

    for (location_name, bin_number, name, size) in items {
        println!("{}/{}: {} ({})", location_name, bin_number, name, size,);
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

fn run_locations(opts: CommonOpts) -> AHResult<()> {
    let store = opts.open_store()?;

    for location in store
        .query(Box::new(query::PropEqual {
            name: "type".to_string(),
            value: "location".to_string().into(),
        }))
        .iter()?
    {
        println!(
            "{} ({} bins)",
            location.get_str("location", "name")?,
            location.get_number("location", "num_bins")?,
        );
    }

    Ok(())
}

fn run_quickadd(opts: QuickaddOpts) -> AHResult<()> {
    let mut store = opts.common.open_store()?;

    // eprintln!("{:#?}", store.all().iter()?.collect::<Vec<Object>>());

    let location = _resolve_location(&store, &opts.location)?;

    let bin_number_display = match opts.location.bin {
        Some(bin_no) => format!("/{}", bin_no),
        None => "".to_string(),
    };
    let prompt = location.get_str("location", "name")? + &bin_number_display + "> ";

    let mut rl = rustyline::Editor::<()>::new();

    while let Ok(line) = rl.readline(&prompt) {
        let mut name = line.trim().to_string();
        let mut size = ItemSizeOpt::S;

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

fn main() -> AHResult<()> {
    let opt = Opts::parse();

    match opt.subcmd {
        SubCommand::Add(o) => run_add(o),
        SubCommand::Items(o) => run_items(o),
        SubCommand::AddLocation(o) => run_add_location(o),
        SubCommand::Locations(o) => run_locations(o),
        SubCommand::Quickadd(o) => run_quickadd(o),
    }
}
