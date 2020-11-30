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
                let bin_number: u64 = parts[1]
                    .parse()
                    .ok()
                    .and_then(|x| if x > 0 { Some(x) } else { None })
                    .context("bin number must be a number > 0")?;

                Ok(Self {
                    location: parts[0].to_string(),
                    bin: Some(bin_number as i64),
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

fn run_add(opts: AddOpts) -> AHResult<()> {
    let mut store = opts.common.open_store()?;

    // eprintln!("{:#?}", store.all().iter()?.collect::<Vec<Object>>());

    let matching_locations = store.query(Box::new(query::And(vec![
        Box::new(query::PropEqual {
            name: "type".to_string(),
            value: "location".into(),
        }),
        Box::new(query::PropLike {
            name: "name".to_string(),
            value: opts.location.location.clone().into(),
        }),
    ])));

    if matching_locations.len()? != 1 {
        bail!(
            "location name \"{}\" did not match exactly one location",
            opts.location.location
        );
    }

    let location = matching_locations.iter()?.next().unwrap();

    let num_bins = location.get_number("location", "num_bins")?;

    if opts.location.bin.unwrap() > num_bins {
        bail!(
            "location {} only has {} bins",
            opts.location.location,
            num_bins
        );
    }

    let location_id = location.get_number("location", "object-id")?;

    let mut item = Object::new();
    item.insert("type".to_string(), "item".to_string().into());
    item.insert("name".to_string(), (&opts.name).into());
    item.insert("location_id".to_string(), location_id.into());
    item.insert("bin_no".to_string(), opts.location.bin.unwrap().into());
    item.insert("size".to_string(), (&opts.size.to_string()).into());

    store.add(item)?;

    println!(
        "{}/{}: {} ({})",
        location.get_str("location", "name")?,
        opts.location.bin.unwrap(),
        opts.name,
        opts.size.to_string(),
    );

    Ok(())
}

#[derive(Clap)]
struct AddLocationOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap()]
    name: String,
    #[clap()]
    num_bins: u64,
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
    location.insert("num_bins".to_string(), (opts.num_bins as i64).into());

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

fn main() -> AHResult<()> {
    let opt = Opts::parse();

    match opt.subcmd {
        SubCommand::Add(o) => run_add(o),
        SubCommand::Items(o) => run_items(o),
        SubCommand::AddLocation(o) => run_add_location(o),
        SubCommand::Locations(o) => run_locations(o),
    }
}
