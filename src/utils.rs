use anyhow::bail;
use qualia::{Object, Store, Q};
use std::collections::HashMap;

use crate::types::{Item, ItemSize, Location};
use crate::AHResult;

fn _choose_bin(store: &Store, location_id: i64, num_bins: i64) -> AHResult<i64> {
    let all_location_items = store.query(Q.equal("type", "item").equal("location_id", location_id));

    let mut bin_fullnesses: HashMap<i64, i64> = (1..=num_bins).map(|bin_no| (bin_no, 0)).collect();
    all_location_items
        .iter_converted::<Item>(&store)?
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
        .find_map(|bin_no| {
            if bin_fullnesses[&bin_no] <= *min_fullness {
                Some(bin_no)
            } else {
                None
            }
        })
        .unwrap())
}

pub fn add_item(
    store: &mut Store,
    name: String,
    location: &Location,
    bin_no: Option<i64>,
    size: ItemSize,
) -> AHResult<Item> {
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
        None => _choose_bin(&store, location.object_id.unwrap(), location.num_bins)?,
    };

    let checkpoint = store.checkpoint()?;
    let mut item = Item {
        object_id: None,
        name,
        location: location.clone(),
        bin_no: bin_number,
        size: size.to_string(),
        rest: Object::new(),
    };
    checkpoint.add_with_id(&mut item)?;
    checkpoint.commit(format!("add item {}", item.name))?;

    Ok(item)
}
