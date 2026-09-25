//! The smart playlist builder's rules, turned into Needle's rule language.
//!
//! Each kind of rule looks at one thing about a song and offers a few ways to compare it; the
//! builder stores the labels (see [`crate::model::RuleSet`]), so what someone built opens the
//! same way again.
use crate::{
    model::{Rule, RuleSet},
    query::quote,
};
use anyhow::{Result, bail};

/// What a rule's value is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// Text, such as a name: the placeholder to show.
    Text(&'static str),
    /// A whole number: the placeholder to show.
    Number(&'static str),
    /// Nothing to type.
    None,
}

/// One kind of rule: its label, how it can compare (label and rule, `{}` for the value), and
/// what its value is.
pub struct Kind {
    pub label: &'static str,
    pub ops: &'static [(&'static str, &'static str)],
    pub value: Value,
}

pub const KINDS: &[Kind] = &[
    Kind {
        label: "Artist",
        ops: &[
            ("contains", "artist contains {}"),
            ("is", "artist = {}"),
            ("is not", "artist != {}"),
        ],
        value: Value::Text("Name"),
    },
    Kind {
        label: "Album",
        ops: &[
            ("contains", "album contains {}"),
            ("is", "album = {}"),
            ("is not", "album != {}"),
        ],
        value: Value::Text("Name"),
    },
    Kind {
        label: "Genre",
        ops: &[
            ("contains", "genre contains {}"),
            ("is", "genre = {}"),
            ("is not", "genre != {}"),
        ],
        value: Value::Text("Pop, Jazz…"),
    },
    Kind {
        label: "Title",
        ops: &[("contains", "title contains {}"), ("is", "title = {}")],
        value: Value::Text("Words"),
    },
    Kind {
        label: "Rating",
        ops: &[
            ("is at least", "rating >= {}"),
            ("is exactly", "rating = {}"),
        ],
        value: Value::Number("Stars, 1 to 5"),
    },
    Kind {
        label: "Favorite",
        ops: &[
            ("is a favorite", "favorite"),
            ("is not a favorite", "not favorite"),
        ],
        value: Value::None,
    },
    Kind {
        label: "Year",
        ops: &[
            ("is", "year = {}"),
            ("is after", "year > {}"),
            ("is before", "year < {}"),
        ],
        value: Value::Number("2020"),
    },
    Kind {
        label: "Added",
        ops: &[
            ("in the last", "recent({}d)"),
            ("not in the last", "not recent({}d)"),
        ],
        value: Value::Number("Days"),
    },
    Kind {
        label: "Played",
        ops: &[
            ("in the last", "played({}d)"),
            ("not in the last", "not played({}d)"),
        ],
        value: Value::Number("Days"),
    },
    Kind {
        label: "Plays",
        ops: &[
            ("at least", "play_count >= {}"),
            ("at most", "play_count <= {}"),
        ],
        value: Value::Number("Times"),
    },
    Kind {
        label: "Length",
        ops: &[
            ("shorter than", "duration < {}m"),
            ("longer than", "duration > {}m"),
        ],
        value: Value::Number("Minutes"),
    },
    Kind {
        label: "Format",
        ops: &[("is", "format = {}"), ("is not", "format != {}")],
        value: Value::Text("FLAC, MP3…"),
    },
    Kind {
        label: "BPM",
        ops: &[("above", "bpm > {}"), ("below", "bpm < {}")],
        value: Value::Number("120"),
    },
];

/// How the songs can be ordered: label and what it adds to the rule.
pub const ORDERS: &[(&str, &str)] = &[
    ("Library order", ""),
    ("Shuffled", " shuffle"),
    ("Most played first", " order by play_count desc"),
    ("Newest added first", " order by added_at desc"),
    ("Highest rated first", " order by rating desc"),
    ("Title, A to Z", " order by title"),
    ("Year, newest first", " order by year desc"),
];

pub fn kind(label: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.label == label)
}

/// A rule's part of the rule language; `None` while its value is still empty.
fn clause(rule: &Rule) -> Result<Option<String>> {
    let Some(kind) = kind(&rule.field) else {
        bail!("Unknown kind of rule “{}”", rule.field)
    };
    let Some((_, template)) = kind.ops.iter().find(|(label, _)| *label == rule.op) else {
        bail!("{} cannot be compared as “{}”", kind.label, rule.op)
    };
    let value = rule.value.trim();
    let filled = match kind.value {
        Value::None => return Ok(Some(template.to_string())),
        _ if value.is_empty() => return Ok(None),
        Value::Text(_) => template.replace("{}", &quote(value)),
        Value::Number(_) => {
            let Ok(number) = value.parse::<u32>() else {
                bail!("{} {} needs a whole number", kind.label, rule.op)
            };
            template.replace("{}", &number.to_string())
        }
    };
    Ok(Some(filled))
}

/// The rule for `set`, in Needle's rule language.
pub fn to_query(set: &RuleSet) -> Result<String> {
    let mut clauses = vec![];
    for rule in &set.rules {
        if let Some(clause) = clause(rule)? {
            clauses.push(clause);
        }
    }
    if clauses.is_empty() {
        bail!("Add a rule, such as Rating is at least 4")
    }
    let mut query = if set.any && clauses.len() > 1 {
        clauses
            .iter()
            .map(|c| format!("({c})"))
            .collect::<Vec<_>>()
            .join(" or ")
    } else {
        clauses.join(" and ")
    };
    if let Some((_, order)) = ORDERS.iter().find(|(label, _)| *label == set.order) {
        query.push_str(order);
    }
    if let Some(limit) = set.limit.filter(|l| *l > 0) {
        query.push_str(&format!(" limit {limit}"));
    }
    Ok(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(field: &str, op: &str, value: &str) -> Rule {
        Rule {
            field: field.into(),
            op: op.into(),
            value: value.into(),
        }
    }

    #[test]
    fn every_kind_and_way_makes_a_valid_rule() {
        for kind in KINDS {
            for (op, _) in kind.ops {
                let value = match kind.value {
                    Value::Text(_) => "Björk \"live\"",
                    Value::Number(_) => "30",
                    Value::None => "",
                };
                let set = RuleSet {
                    rules: vec![rule(kind.label, op, value)],
                    ..Default::default()
                };
                let query = to_query(&set).unwrap();
                crate::query::compile(&query, 0)
                    .unwrap_or_else(|e| panic!("{} {op}: {query}: {e}", kind.label));
            }
        }
        for (label, _) in ORDERS {
            let set = RuleSet {
                rules: vec![rule("Rating", "is at least", "4")],
                order: label.to_string(),
                limit: Some(25),
                ..Default::default()
            };
            crate::query::compile(&to_query(&set).unwrap(), 0).unwrap();
        }
    }

    #[test]
    fn joins_rules_and_skips_empty_ones() {
        let mut set = RuleSet {
            rules: vec![
                rule("Genre", "is", "K-pop"),
                rule("Artist", "contains", " "),
                rule("Played", "not in the last", "30"),
            ],
            order: "Shuffled".into(),
            limit: Some(50),
            ..Default::default()
        };
        assert_eq!(
            to_query(&set).unwrap(),
            "genre = \"K-pop\" and not played(30d) shuffle limit 50"
        );
        set.any = true;
        assert_eq!(
            to_query(&set).unwrap(),
            "(genre = \"K-pop\") or (not played(30d)) shuffle limit 50"
        );
        assert!(to_query(&RuleSet::default()).is_err());
        assert!(
            to_query(&RuleSet {
                rules: vec![rule("Year", "is", "soon")],
                ..Default::default()
            })
            .is_err()
        );
    }
}
