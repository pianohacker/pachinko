#[macro_use]
mod common;
use common::*;

use predicates::reflection;

struct JsonMatcher {
    expected: serde_json::Value,
}

impl predicates::Predicate<[u8]> for JsonMatcher {
    fn eval(&self, variable: &[u8]) -> bool {
        let actual: serde_json::Value = serde_json::from_slice(variable).unwrap();

        actual == self.expected
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &[u8]) -> Option<reflection::Case<'a>> {
        let actual_value: serde_json::Value = serde_json::from_slice(variable).unwrap();
        let result = self.expected == actual_value;
        if result == expected {
            Some(
                reflection::Case::new(Some(self), result)
                    .add_product(reflection::Product::new("actual value", actual_value)),
            )
        } else {
            None
        }
    }
}

impl reflection::PredicateReflection for JsonMatcher {}

impl std::fmt::Display for JsonMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "var.is_json({})", self.expected)
    }
}

fn is_json(expected_str: impl AsRef<str>) -> JsonMatcher {
    let expected_str = expected_str.as_ref();

    let expected: serde_json::Value = serde_json::from_str(expected_str).unwrap();

    JsonMatcher { expected }
}

#[test]
fn can_dump_items() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test/4", "Test item", "M"]);
    ctx.assert_pch(&["add", "huge/6", "Huge item", "M"]);
    ctx.assert_pch(&["add", "test/4", "Test blight'em", "M"]);

    ctx.assert_pch(&["dump"])
        .stderr(predicates::str::is_empty())
        .stdout(is_json(
            r#"
[
    {
        "object_id": 1,
        "name": "Test",
        "num_bins": 4,
        "type": "location"
    },
    {
        "object_id": 2,
        "name": "Tiny",
        "num_bins": 1,
        "type": "location"
    },
    {
        "object_id": 3,
        "name": "Huge",
        "num_bins": 16,
        "type": "location"
    },
    {
        "object_id": 4,
        "bin_no": 4,
        "location_id": 1,
        "name": "Test item",
        "size": "M",
        "type": "item"
    },
    {
        "object_id": 5,
        "bin_no": 6,
        "location_id": 3,
        "name": "Huge item",
        "size": "M",
        "type": "item"
    },
    {
        "object_id": 6,
        "bin_no": 4,
        "location_id": 1,
        "name": "Test blight'em",
        "size": "M",
        "type": "item"
    }
]
        "#,
        ));
}
