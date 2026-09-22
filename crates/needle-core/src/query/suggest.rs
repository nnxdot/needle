//! Context-aware completion for the rule language.
//!
//! Offsets are UTF-8 byte offsets into the input. Only text before the cursor decides the
//! context; the word or quoted value under the cursor is replaced as a whole. Plain words
//! that are not part of the rule vocabulary produce no suggestions, so ordinary text
//! searches stay quiet; [`looks_like_rule`] tells whether the input will be parsed as a rule.
use super::{FIELDS, field_info};
use crate::database::Library;
use serde::Serialize;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    Field,
    Operator,
    Keyword,
    Function,
    Value,
    Example,
}

/// Replace `input[replace]` with `insert` to accept the suggestion. `insert` already
/// carries any separating spaces; `label` is the text to display.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Suggestion {
    pub replace: Range<usize>,
    pub insert: String,
    pub label: String,
    pub detail: String,
    pub kind: SuggestionKind,
}

/// Quote text for the rule language: double quotes, with `\` and `"` escaped.
pub fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Clone, Debug, PartialEq)]
enum Lex {
    Word(String),
    Text(String, bool),
    Op,
    Left,
    Right,
    Comma,
}

fn lex(input: &str) -> Vec<(Lex, Range<usize>)> {
    let mut out = vec![];
    let mut chars = input.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        chars.next();
        match c {
            c if c.is_whitespace() => {}
            '(' => out.push((Lex::Left, start..start + 1)),
            ')' => out.push((Lex::Right, start..start + 1)),
            ',' => out.push((Lex::Comma, start..start + 1)),
            '\'' | '"' => {
                let (mut text, mut end, mut closed) = (String::new(), input.len(), false);
                while let Some((i, ch)) = chars.next() {
                    if ch == c {
                        (end, closed) = (i + 1, true);
                        break;
                    }
                    if ch == '\\' {
                        if let Some((_, escaped)) = chars.next() {
                            text.push(escaped);
                        }
                    } else {
                        text.push(ch);
                    }
                }
                out.push((Lex::Text(text, closed), start..end));
            }
            '=' | '!' | '>' | '<' | ':' => {
                let mut end = start + 1;
                if let Some(&(i, '=')) = chars.peek() {
                    chars.next();
                    end = i + 1;
                }
                out.push((Lex::Op, start..end));
            }
            _ => {
                let mut end = start + c.len_utf8();
                while let Some(&(i, ch)) = chars.peek() {
                    if ch.is_whitespace() || "(),=!:<>\"'".contains(ch) {
                        break;
                    }
                    end = i + ch.len_utf8();
                    chars.next();
                }
                out.push((Lex::Word(input[start..end].into()), start..end));
            }
        }
    }
    out
}

/// Whether search would treat `input` as a rule rather than plain title/artist/album/genre text.
pub fn looks_like_rule(input: &str) -> bool {
    let tokens = lex(input.trim());
    let word = |i: usize| match tokens.get(i) {
        Some((Lex::Word(w), _)) => Some(w.to_lowercase()),
        _ => None,
    };
    tokens.iter().any(|(t, _)| matches!(t, Lex::Op | Lex::Left))
        || word(0).is_some_and(|w| ["order", "limit", "shuffle", "missing"].contains(&w.as_str()))
        || (1..tokens.len()).any(|i| {
            word(i).is_some_and(|w| w == "contains")
                && word(i - 1).is_some_and(|f| field_info(&f).is_ok())
        })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Condition { first: bool },
    Function(&'static str),
    FunctionArg(&'static str),
    FunctionClose,
    ExistsField,
    ExistsClose,
    Field(&'static str, bool),
    Value(&'static str, bool),
    After,
    Order,
    OrderField,
    OrderDirection,
    Suffix,
    Limit,
    Unknown,
}

fn step(state: State, token: &Lex, depth: &mut usize) -> State {
    use State::*;
    let word = match token {
        Lex::Word(w) => w.to_lowercase(),
        _ => String::new(),
    };
    let field = || {
        field_info(&word)
            .ok()
            .map(|(name, _, numeric)| (name, numeric))
    };
    match (state, token) {
        (Condition { .. }, Lex::Left) => {
            *depth += 1;
            Condition { first: false }
        }
        (Condition { first }, Lex::Word(_)) => match word.as_str() {
            "not" => Condition { first: false },
            "missing" => After,
            "recent" => Function("recent"),
            "played" => Function("played"),
            "exists" => Function("exists"),
            "order" if first => Order,
            "shuffle" if first => Suffix,
            "limit" if first => Limit,
            _ => field().map_or(Unknown, |(name, numeric)| Field(name, numeric)),
        },
        (Function("exists"), Lex::Left) => ExistsField,
        (Function(name), Lex::Left) => FunctionArg(name),
        (FunctionArg(_), Lex::Word(_)) => FunctionClose,
        (FunctionClose | ExistsClose, Lex::Right) => After,
        (ExistsField, Lex::Word(_)) if field().is_some() => ExistsClose,
        (Field(name, numeric), Lex::Op) => Value(name, numeric),
        (Field(name, false), Lex::Word(_)) if word == "contains" => Value(name, false),
        (Value(..), Lex::Word(_) | Lex::Text(_, true)) => After,
        (After, Lex::Right) if *depth > 0 => {
            *depth -= 1;
            After
        }
        (After, Lex::Word(_)) => match word.as_str() {
            "and" | "or" => Condition { first: false },
            "order" if *depth == 0 => Order,
            "shuffle" if *depth == 0 => Suffix,
            "limit" if *depth == 0 => Limit,
            _ => Unknown,
        },
        (Order, Lex::Word(_)) if word == "by" => OrderField,
        (OrderField, Lex::Word(_)) if field().is_some() => OrderDirection,
        (OrderDirection, Lex::Word(_)) if word == "asc" || word == "desc" => Suffix,
        (OrderDirection | Suffix, Lex::Word(_)) if word == "limit" => Limit,
        _ => Unknown,
    }
}

fn field_detail(name: &str) -> &'static str {
    match name {
        "title" => "Track title",
        "artist" => "Track artist",
        "album" => "Album title",
        "album_artist" => "Album artist",
        "genre" => "Genre",
        "format" => "File format, such as FLAC",
        "path" => "File path",
        "year" => "Release year",
        "bpm" => "Beats per minute",
        "rating" => "Rating from 0 to 5",
        "duration" => "Length in seconds",
        "length_seconds" => "Length in seconds (same as duration)",
        "sample_rate" => "Sample rate in Hz",
        "bit_depth" => "Bits per sample",
        "play_count" => "Qualified plays",
        "added_at" => "Unix time the track was added",
        "last_played" => "Unix time of the last qualified play",
        _ => "",
    }
}

type Candidate = (SuggestionKind, String, String, String);

fn candidate(
    kind: SuggestionKind,
    insert: impl Into<String>,
    label: impl Into<String>,
    detail: impl Into<String>,
) -> Candidate {
    (kind, insert.into(), label.into(), detail.into())
}

fn fields(suffix: &str) -> Vec<Candidate> {
    FIELDS
        .iter()
        .map(|(name, _, numeric)| {
            candidate(
                SuggestionKind::Field,
                format!("{name}{suffix}"),
                *name,
                format!(
                    "{} ({})",
                    field_detail(name),
                    if *numeric { "number" } else { "text" }
                ),
            )
        })
        .collect()
}

fn keyword(insert: &str, detail: &str) -> Candidate {
    candidate(SuggestionKind::Keyword, insert, insert, detail)
}

fn candidates(state: State, depth: usize) -> Vec<Candidate> {
    use State::*;
    use SuggestionKind::{Function as Func, Operator, Value as Val};
    let year = chrono::Utc::now().format("%Y").to_string();
    let durations = |suffix: &str| {
        [
            ("7d", "7 days"),
            ("30d", "30 days"),
            ("90d", "90 days"),
            ("12h", "12 hours"),
            ("4w", "4 weeks"),
        ]
        .map(|(d, detail)| candidate(Val, format!("{d}{suffix}"), d, detail))
        .to_vec()
    };
    match state {
        Condition { first } => {
            let mut list = fields("");
            list.extend([
                candidate(Func, "recent(30d)", "recent(30d)", "Added within a period"),
                candidate(Func, "played(7d)", "played(7d)", "Played within a period"),
                candidate(
                    Func,
                    format!("played({year})"),
                    format!("played({year})"),
                    "Played during a calendar year",
                ),
                candidate(Func, "exists(", "exists(field)", "Field has a value"),
                keyword("not", "Negate the next condition"),
                keyword("missing", "File is missing from disk"),
            ]);
            if first {
                list.extend([
                    keyword("order by", "Sort all tracks"),
                    keyword("shuffle", "Shuffle all tracks"),
                    keyword("limit", "Limit the number of tracks"),
                ]);
            }
            list
        }
        Function(name) => match name {
            "exists" => vec![candidate(Operator, "(", "(field)", "Name a field")],
            _ => vec![
                candidate(Operator, "(30d)", "(30d)", "Within 30 days"),
                candidate(Operator, "(7d)", "(7d)", "Within 7 days"),
            ],
        },
        FunctionArg(name) => {
            let mut list = durations(")");
            if name == "played" {
                let last = (year.parse::<i32>().unwrap_or(2026) - 1).to_string();
                list.push(candidate(
                    Val,
                    format!("{year})"),
                    year.clone(),
                    "This calendar year",
                ));
                list.push(candidate(
                    Val,
                    format!("{last})"),
                    last,
                    "Last calendar year",
                ));
            }
            list
        }
        FunctionClose | ExistsClose => vec![candidate(Operator, ")", ")", "Close the function")],
        ExistsField => fields(")"),
        Field(_, numeric) => {
            let ops: &[(&str, &str)] = if numeric {
                &[
                    ("=", "Equals"),
                    ("!=", "Does not equal"),
                    (">", "Greater than"),
                    (">=", "At least"),
                    ("<", "Less than"),
                    ("<=", "At most"),
                ]
            } else {
                &[
                    ("=", "Equals, ignoring case"),
                    ("!=", "Does not equal"),
                    ("contains", "Contains text, ignoring case"),
                ]
            };
            ops.iter()
                .map(|(op, detail)| candidate(Operator, *op, *op, *detail))
                .collect()
        }
        Value(name, true) => {
            let values: Vec<(String, &str)> = match name {
                "rating" => vec![
                    ("5".into(), "Five stars"),
                    ("4".into(), "Four stars"),
                    ("3".into(), "Three stars"),
                ],
                "year" => vec![(year.clone(), "This year"), ("2000".into(), "Example year")],
                "bpm" => vec![
                    ("120".into(), "Example tempo"),
                    ("null".into(), "No BPM stored"),
                ],
                "sample_rate" => vec![
                    ("44100".into(), "44.1 kHz"),
                    ("48000".into(), "48 kHz"),
                    ("96000".into(), "96 kHz"),
                ],
                "bit_depth" => vec![("16".into(), "16-bit"), ("24".into(), "24-bit")],
                "duration" | "length_seconds" => vec![("300".into(), "Five minutes")],
                "play_count" => vec![("0".into(), "Never played"), ("10".into(), "Ten plays")],
                "last_played" => vec![("null".into(), "Never played")],
                _ => vec![],
            };
            values
                .into_iter()
                .map(|(v, d)| candidate(Val, v.clone(), v, d))
                .collect()
        }
        Value(_, false) => vec![],
        After => {
            let mut list = vec![
                keyword("and", "Both conditions"),
                keyword("or", "Either condition"),
            ];
            if depth > 0 {
                list.push(candidate(Operator, ")", ")", "Close the group"));
            } else {
                list.extend([
                    keyword("order by", "Sort the results"),
                    keyword("shuffle", "Shuffle the results"),
                    keyword("limit", "Limit the number of tracks"),
                ]);
            }
            list
        }
        Order => vec![keyword("by", "Choose a sort field")],
        OrderField => fields(""),
        OrderDirection => vec![
            keyword("asc", "Ascending"),
            keyword("desc", "Descending"),
            keyword("limit", "Limit the number of tracks"),
        ],
        Suffix => vec![keyword("limit", "Limit the number of tracks")],
        Limit => ["20", "50", "100"]
            .map(|n| candidate(Val, n, n, "Maximum tracks"))
            .to_vec(),
        Unknown => vec![],
    }
}

const EXAMPLES: &[(&str, &str)] = &[
    (
        "rating >= 4 and not played(7d) shuffle limit 20",
        "Favorites you have not heard this week",
    ),
    ("recent(30d) and bpm > 120", "Fast tracks added this month"),
    (
        "format = \"FLAC\" and sample_rate >= 96000",
        "High-resolution FLAC",
    ),
    (
        "played(7d) order by play_count desc",
        "This week's most played",
    ),
    ("missing", "Files missing from disk"),
];

/// Suggestions for the rule under `cursor` (a byte offset). See the module notes.
pub fn suggest(input: &str, cursor: usize) -> Vec<Suggestion> {
    build(input, cursor, None, usize::MAX)
}

/// Like [`suggest`], and adds quoted library values after `field =`, `!=`, or `contains`
/// for artist, album, album_artist, genre, and format, most common first. At most `limit`
/// suggestions are returned.
pub fn suggest_with_library(
    library: &Library,
    input: &str,
    cursor: usize,
    limit: usize,
) -> Vec<Suggestion> {
    build(input, cursor, Some(library), limit)
}

fn build(input: &str, cursor: usize, library: Option<&Library>, limit: usize) -> Vec<Suggestion> {
    if input.len() > 8192 {
        return vec![];
    }
    let mut cursor = cursor.min(input.len());
    while !input.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let mut out: Vec<Suggestion> = vec![];
    if input.trim().is_empty() {
        out.extend(EXAMPLES.iter().map(|(rule, detail)| Suggestion {
            replace: 0..input.len(),
            insert: rule.to_string(),
            label: rule.to_string(),
            detail: detail.to_string(),
            kind: SuggestionKind::Example,
        }));
    }
    let mut tokens = lex(&input[..cursor]);
    let partial = match tokens.last() {
        Some((Lex::Word(_), r)) if r.end == cursor => tokens.pop(),
        Some((Lex::Text(_, false), _)) => tokens.pop(),
        _ => None,
    };
    let (mut state, mut depth) = (State::Condition { first: true }, 0);
    for (token, _) in &tokens {
        state = step(state, token, &mut depth);
        if state == State::Unknown {
            return vec![];
        }
    }
    let (replace, prefix) = match &partial {
        Some((token, range)) => {
            let end = lex(&input[range.start..])
                .first()
                .map_or(cursor, |(_, r)| range.start + r.end)
                .max(cursor);
            let prefix = match token {
                Lex::Text(text, _) => text.clone(),
                _ => input[range.start..cursor].to_string(),
            };
            (range.start..end, prefix)
        }
        None => (cursor..cursor, String::new()),
    };
    let is_text = matches!(partial, Some((Lex::Text(..), _)));
    let push = |out: &mut Vec<Suggestion>,
                replace: Range<usize>,
                (kind, insert, label, detail): Candidate| {
        if input[replace.clone()] == insert {
            return;
        }
        let before = input[..replace.start].chars().next_back();
        let after = input[replace.end..].chars().next();
        let lead = before.is_some_and(|c| !c.is_whitespace() && c != '(')
            && !insert.starts_with(['(', ')']);
        let trail = !insert.ends_with('(') && after.is_none_or(|c| !c.is_whitespace() && c != ')');
        out.push(Suggestion {
            insert: format!(
                "{}{insert}{}",
                if lead { " " } else { "" },
                if trail { " " } else { "" }
            ),
            replace,
            label,
            detail,
            kind,
        });
    };
    if let (State::Value(field, false), Some(library)) = (state, library)
        && crate::browse::VALUE_FIELDS.contains(&field)
    {
        for (value, count) in library
            .field_values(field, &prefix, limit)
            .unwrap_or_default()
        {
            let detail = format!("{count} track{}", if count == 1 { "" } else { "s" });
            push(
                &mut out,
                replace.clone(),
                candidate(SuggestionKind::Value, quote(&value), value, detail),
            );
        }
    }
    let lower = prefix.to_lowercase();
    if !is_text {
        for c in candidates(state, depth) {
            if c.2.to_lowercase().starts_with(&lower) {
                push(&mut out, replace.clone(), c);
            }
        }
    }
    if let Some((token @ Lex::Word(_), range)) = &partial
        && range.end == replace.end
    {
        let mut next_depth = depth;
        let next = step(state, token, &mut next_depth);
        if next != State::Unknown {
            for c in candidates(next, next_depth) {
                push(&mut out, replace.end..replace.end, c);
            }
        }
    }
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::Track, query::compile};

    fn labels(input: &str) -> Vec<String> {
        suggest(input, input.len())
            .into_iter()
            .map(|s| s.label)
            .collect()
    }
    fn apply(input: &str, s: &Suggestion) -> String {
        format!(
            "{}{}{}",
            &input[..s.replace.start],
            s.insert,
            &input[s.replace.end..]
        )
    }
    fn pick(input: &str, label: &str) -> String {
        let s = suggest(input, input.len())
            .into_iter()
            .find(|s| s.label == label)
            .unwrap_or_else(|| panic!("{label} not suggested for {input:?}: {:?}", labels(input)));
        apply(input, &s)
    }

    #[test]
    fn quoting_round_trips_through_the_parser() {
        for value in [
            "plain",
            "O'Brien",
            "say \"hi\"",
            "back\\slash",
            "Björk テスト",
            "",
            "100%_mix",
        ] {
            let q = compile(&format!("artist = {}", quote(value)), 0).unwrap();
            assert_eq!(
                q.parameters,
                vec![rusqlite::types::Value::Text(value.into())],
                "{value}"
            );
        }
    }

    #[test]
    fn start_of_rule_offers_fields_functions_and_not() {
        let all = labels("");
        assert!(all.contains(&"rating >= 4 and not played(7d) shuffle limit 20".to_string()));
        assert!(all.contains(&"artist".to_string()));
        assert!(all.contains(&"recent(30d)".to_string()));
        assert!(all.contains(&"exists(field)".to_string()));
        assert!(all.contains(&"not".to_string()));
        assert_eq!(
            suggest("", 0)
                .iter()
                .filter(|s| s.kind == SuggestionKind::Example)
                .count(),
            EXAMPLES.len()
        );
        assert_eq!(labels("ar"), ["artist"]);
        assert_eq!(labels("AL"), ["album", "album_artist"]);
        assert!(labels("re").contains(&"recent(30d)".to_string()));
        assert!(!labels("re").contains(&"rating".to_string()));
        assert!(labels("ra").contains(&"rating".to_string()));
        assert_eq!(pick("ar", "artist"), "artist ");
        assert_eq!(pick("not ", "bpm"), "not bpm ");
        assert_eq!(pick("(", "genre"), "(genre ");
        assert_eq!(pick("rating > 3 and (", "year"), "rating > 3 and (year ");
        assert_eq!(
            pick("rating > 3 or ", "played(7d)"),
            "rating > 3 or played(7d) "
        );
        assert_eq!(pick("ex", "exists(field)"), "exists(");
        assert_eq!(pick("exists(", "bpm"), "exists(bpm) ");
        assert_eq!(pick("exists(b", "bpm"), "exists(bpm) ");
        assert_eq!(pick("recent(", "7d"), "recent(7d) ");
        assert!(labels("played(").contains(&chrono::Utc::now().format("%Y").to_string()));
        assert!(!labels("recent(").contains(&chrono::Utc::now().format("%Y").to_string()));
        assert_eq!(labels("recent(30d"), ["30d", ")"]);
        assert_eq!(pick("recent(30d", ")"), "recent(30d) ");
    }

    #[test]
    fn operators_depend_on_field_type() {
        assert_eq!(labels("artist "), ["=", "!=", "contains"]);
        assert_eq!(labels("rating "), ["=", "!=", ">", ">=", "<", "<="]);
        assert_eq!(labels("artist c"), ["contains"]);
        assert!(labels("rating c").is_empty());
        assert_eq!(pick("rating", ">="), "rating >= ");
        assert_eq!(pick("album", "album_artist"), "album_artist ");
        assert_eq!(pick("album", "contains"), "album contains ");
        assert!(labels("rating contains ").is_empty());
    }

    #[test]
    fn complete_comparisons_offer_connectives_and_suffixes() {
        assert_eq!(
            labels("rating >= 4 "),
            ["and", "or", "order by", "shuffle", "limit"]
        );
        assert_eq!(labels("(rating >= 4 "), ["and", "or", ")"]);
        assert_eq!(
            labels("(rating >= 4) "),
            ["and", "or", "order by", "shuffle", "limit"]
        );
        assert_eq!(labels("artist = \"Björk\" o"), ["or", "order by"]);
        assert_eq!(pick("artist = \"Björk\"", "and"), "artist = \"Björk\" and ");
        assert_eq!(pick("rating >= 4", "and"), "rating >= 4 and ");
        assert_eq!(
            labels("missing "),
            ["and", "or", "order by", "shuffle", "limit"]
        );
        assert_eq!(
            labels("played(7d) "),
            ["and", "or", "order by", "shuffle", "limit"]
        );
        assert!(labels("rating > 4 and ").contains(&"bpm".to_string()));
    }

    #[test]
    fn order_by_offers_fields_then_directions() {
        assert_eq!(labels("rating > 3 order "), ["by"]);
        assert!(labels("rating > 3 order by ").contains(&"year".to_string()));
        assert_eq!(labels("order by y"), ["year"]);
        assert_eq!(labels("order by year "), ["asc", "desc", "limit"]);
        assert_eq!(labels("order by year d"), ["desc"]);
        assert_eq!(labels("order by year desc "), ["limit"]);
        assert_eq!(labels("shuffle "), ["limit"]);
        assert_eq!(labels("rating > 3 limit "), ["20", "50", "100"]);
        assert!(labels("rating > 3 limit 20 ").is_empty());
        let rule = pick(&pick(&pick("rating > 3 order ", "by"), "year"), "desc");
        assert_eq!(rule, "rating > 3 order by year desc ");
        assert!(compile(&rule, 0).is_ok());
    }

    #[test]
    fn plain_text_is_quiet_and_detected() {
        for plain in [
            "daft punk",
            "The Beatles ",
            "Earth Wind and Fire",
            "Artist 00420",
        ] {
            assert!(labels(plain).is_empty(), "{plain}: {:?}", labels(plain));
            assert!(!looks_like_rule(plain));
        }
        for rule in [
            "rating > 3",
            "artist contains x",
            "(a",
            "missing",
            "order by year",
            "shuffle",
            "limit 3",
            "artist = \"unfinished",
        ] {
            assert!(looks_like_rule(rule), "{rule}");
        }
        for input in [
            "daft punk",
            "rating > 3",
            "artist contains x",
            "title contains",
            "missing",
            "Earth Wind and Fire",
        ] {
            let plain = compile(input, 0)
                .map(|q| q.explanation.starts_with("Title, artist"))
                .unwrap_or(false);
            assert_eq!(looks_like_rule(input), !plain, "{input}");
        }
    }

    #[test]
    fn cursor_in_the_middle_replaces_the_whole_word() {
        let input = "artsomething = \"x\"";
        let s = suggest(input, 3);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].label, "artist");
        assert_eq!(s[0].replace, 0..12);
        assert_eq!(apply(input, &s[0]), "artist = \"x\"");
        assert!(compile(&apply(input, &s[0]), 0).is_ok());
        let input = "bpm > 1 and ra = 4";
        let s = suggest(input, 14);
        assert_eq!(s[0].label, "rating");
        assert_eq!(apply(input, &s[0]), "bpm > 1 and rating = 4");
        let unicode = "artist = \"Bjö";
        assert!(suggest(unicode, unicode.len() - 1).is_empty());
        assert!(suggest(unicode, 9999).is_empty());
        assert_eq!(suggest("ar", 9999)[0].replace, 0..2);
    }

    #[test]
    fn numeric_values_and_continuations() {
        assert_eq!(labels("rating >= "), ["5", "4", "3"]);
        assert_eq!(
            labels("sample_rate = 4"),
            ["44100", "48000"]
                .map(String::from)
                .into_iter()
                .chain(["and", "or", "order by", "shuffle", "limit"].map(String::from))
                .collect::<Vec<_>>()
        );
        assert_eq!(pick("rating >= 4", "or"), "rating >= 4 or ");
        assert!(labels("artist = ").is_empty());
    }

    #[test]
    fn library_values_are_quoted_and_ranked() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        for (id, artist) in [
            ("1", "Björk"),
            ("2", "Björk"),
            ("3", "Bob \"Bobby\" O'Neil"),
            ("4", "Air"),
            ("5", "back\\slash"),
        ] {
            library
                .upsert(&Track {
                    id: id.into(),
                    path: format!("/{id}.flac"),
                    artist: artist.into(),
                    genre: "Pop".into(),
                    ..Default::default()
                })
                .unwrap();
        }
        let s = suggest_with_library(&library, "artist = ", 9, 10);
        assert_eq!(s[0].label, "Björk");
        assert_eq!(s[0].insert, "\"Björk\" ");
        assert_eq!(s[0].detail, "2 tracks");
        assert_eq!(s[0].kind, SuggestionKind::Value);
        assert_eq!(s.len(), 4);
        let input = "artist = \"b";
        let s = suggest_with_library(&library, input, input.len(), 10);
        assert_eq!(s.len(), 3);
        for s in &s {
            let rule = apply(input, s);
            let found = library.search(&rule).unwrap();
            assert_eq!(
                found.len(),
                if s.label == "Björk" { 2 } else { 1 },
                "{rule}"
            );
            assert_eq!(found[0].artist, s.label);
        }
        let input = "genre contains po and artist != ";
        let s = suggest_with_library(&library, input, input.len(), 2);
        assert_eq!(s.len(), 2);
        let s = suggest_with_library(&library, "genre contains p", 16, 10);
        assert_eq!(s[0].insert, "\"Pop\" ");
        assert_eq!(s[0].replace, 15..16);
        let mid = "artist = \"Bj\" and rating > 3";
        let s = suggest_with_library(&library, mid, 12, 10);
        assert_eq!(s[0].replace, 9..13);
        assert_eq!(apply(mid, &s[0]), "artist = \"Björk\" and rating > 3");
        assert!(suggest_with_library(&library, "title = ", 8, 10).is_empty());
        assert!(
            suggest_with_library(&library, "rating = ", 9, 10)
                .iter()
                .all(|s| s.kind == SuggestionKind::Value)
        );
        assert!(suggest_with_library(&library, "daft punk", 9, 10).is_empty());
    }
}
