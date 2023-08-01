use anyhow::{anyhow, bail, Context};
use clap::ValueEnum;
use qualia::{object, Object, ObjectShape, ObjectShapeWithId, Queryable, Store};
use std::str::FromStr;

use crate::AHResult;

#[derive(Clone, Debug, Eq, PartialEq, ObjectShape)]
#[fixed_fields("type" => "location")]
pub struct Location {
    pub object_id: Option<i64>,
    pub name: String,
    pub num_bins: i64,
}

#[derive(Clone, Debug, ObjectShape, PartialEq, Eq)]
#[fixed_fields("type" => "item")]
pub struct Item {
    pub object_id: Option<i64>,
    pub name: String,
    pub location: Location,
    pub bin_no: i64,
    pub size: String,

    #[rest_fields]
    pub rest: Object,
}

impl Item {
    pub fn format(&self) -> FormattedItem {
        let bin_no = if self.location.num_bins > 1 {
            Some(self.bin_no)
        } else {
            None
        };

        FormattedItem {
            location_name: self.location.name.clone(),
            bin_no,
            name: self.name.clone(),
            size: self.size.clone(),
        }
    }

    pub fn format_with_store(&self, _store: &Store) -> AHResult<FormattedItem> {
        Ok(self.format())
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub struct FormattedItem {
    pub location_name: String,
    pub bin_no: Option<i64>,
    pub name: String,
    pub size: String,
}

impl FormattedItem {
    pub fn format_location(&self) -> String {
        if let Some(bin_no) = self.bin_no {
            format!("{}/{}", self.location_name, bin_no)
        } else {
            self.location_name.clone()
        }
    }
}

impl std::fmt::Display for FormattedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "{}: {} ({})",
            self.format_location(),
            self.name,
            self.size
        )
    }
}

pub fn parse_bin_number(s: &str) -> AHResult<i64> {
    s.parse::<i64>()
        .context("failed to parse bin number")
        .and_then(|x| {
            if x > 0 {
                Ok(x)
            } else {
                Err(anyhow!("must be greater than zero"))
            }
        })
}

pub fn bin_number_value_parser(s: &str) -> Result<i64, String> {
    parse_bin_number(s).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct ItemLocation {
    pub location: String,
    pub bin: Option<i64>,
}

impl FromStr for ItemLocation {
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

#[derive(Copy, Clone, ValueEnum, Debug, PartialEq)]
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
        match s.to_ascii_uppercase().as_ref() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_size_parsing_should_succeed_for_lowercase_sizes() {
        assert_eq!("s".parse::<ItemSize>().unwrap(), ItemSize::S);
        assert_eq!("m".parse::<ItemSize>().unwrap(), ItemSize::M);
    }
}
