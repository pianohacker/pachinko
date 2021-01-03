use anyhow::{anyhow, bail, Context};
use clap::Clap;
use qualia::{object, ObjectShape, Store, Q};

use crate::AHResult;

#[derive(ObjectShape)]
pub struct Item {
    pub name: String,
    pub location_id: i64,
    pub bin_no: i64,
    pub size: String,
}

impl Item {
    pub fn format_with_store(&self, store: &Store) -> AHResult<FormattedItem> {
        let matching_locations = store.query(Q.equal("type", "location").id(self.location_id));

        if matching_locations.len()? != 1 {
            bail!(
                "location id \"{}\" did not match exactly one location",
                self.location_id,
            );
        }

        let location = matching_locations.iter_as::<Location>()?.next().unwrap();
        let bin_no = if location.num_bins > 1 {
            Some(self.bin_no)
        } else {
            None
        };

        Ok(FormattedItem {
            location_name: location.name,
            bin_no,
            name: self.name.clone(),
            size: self.size.clone(),
        })
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub struct FormattedItem {
    pub location_name: String,
    pub bin_no: Option<i64>,
    pub name: String,
    pub size: String,
}

impl std::fmt::Display for FormattedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let item_location = if let Some(bin_no) = self.bin_no {
            format!("{}/{}", self.location_name, bin_no)
        } else {
            self.location_name.clone()
        };

        write!(f, "{}: {} ({})", item_location, self.name, self.size)
    }
}

pub fn parse_bin_number(s: &str) -> AHResult<i64> {
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

pub struct ItemLocation {
    pub location: String,
    pub bin: Option<i64>,
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
pub enum ItemSize {
    S,
    M,
    L,
    X,
}

impl std::str::FromStr for ItemSize {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> AHResult<Self> {
        match s {
            "S" => Ok(ItemSize::S),
            "M" => Ok(ItemSize::M),
            "L" => Ok(ItemSize::L),
            "X" => Ok(ItemSize::X),
            _ => Err(anyhow!("attempt to convert size from not \"[SMLX]\"")),
        }
    }
}

impl ToString for ItemSize {
    fn to_string(&self) -> std::string::String {
        match self {
            ItemSize::S => "S",
            ItemSize::M => "M",
            ItemSize::L => "L",
            ItemSize::X => "X",
        }
        .to_string()
    }
}

impl From<ItemSize> for i64 {
    fn from(size: ItemSize) -> i64 {
        match size {
            ItemSize::S => 2,
            ItemSize::M => 3,
            ItemSize::L => 4,
            ItemSize::X => 6,
        }
    }
}

#[derive(ObjectShape)]
pub struct Location {
    #[object_field("object-id")]
    pub id: i64,
    pub name: String,
    pub num_bins: i64,
}
