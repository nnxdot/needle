//! A bounded, typed expression language. SQL values are always bound parameters.
use crate::model::Track;
use anyhow::{Result, bail};
use rusqlite::types::Value;
mod suggest;
pub use suggest::{
    Suggestion, SuggestionKind, looks_like_rule, quote, suggest, suggest_with_library,
};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Word(String),
    Text(String),
    Number(f64),
    Op(String),
    Left,
    Right,
    Comma,
}

#[derive(Clone, Debug)]
pub struct Query {
    pub sql: String,
    pub parameters: Vec<Value>,
    pub order: String,
    pub limit: usize,
    pub explanation: String,
    /// `limit N per field`: keep at most N tracks for each value of this SQL column.
    pub per: Option<(String, usize)>,
    /// `shuffle by field`: spread results so neighbours differ in this track field.
    pub spread: Option<&'static str>,
}

impl Query {
    /// The `FROM` source. With `per`, a ranked subquery that already applies the rule, so its
    /// parameters bind inside it and [`Query::filter`] needs none.
    pub fn from(&self) -> String {
        match &self.per {
            None => "tracks t".into(),
            Some((column, _)) => format!(
                "(SELECT t.*, ROW_NUMBER() OVER (PARTITION BY {column} COLLATE NOCASE ORDER BY {}) AS needle_rank FROM tracks t WHERE ({})) t",
                self.order, self.sql
            ),
        }
    }
    /// The `WHERE` condition to use with [`Query::from`].
    pub fn filter(&self) -> String {
        match &self.per {
            None => self.sql.clone(),
            Some((_, n)) => format!("t.needle_rank <= {n}"),
        }
    }
}

/// Reorder tracks so that, where possible, no two neighbours share `field` (artist, album,
/// or genre). Always takes the next track from the group with the most tracks left, which
/// keeps the original order within each group.
pub fn spread(tracks: Vec<Track>, field: &str) -> Vec<Track> {
    use std::collections::{HashMap, VecDeque};
    let key = |t: &Track| match field {
        "album" => format!(
            "{}\0{}",
            t.album_artist.to_lowercase(),
            t.album.to_lowercase()
        ),
        "genre" => t.genre.to_lowercase(),
        _ => t.display_artist().to_lowercase(),
    };
    let mut order = vec![];
    let mut groups: HashMap<String, VecDeque<Track>> = HashMap::new();
    for track in tracks {
        let k = key(&track);
        if !groups.contains_key(&k) {
            order.push(k.clone());
        }
        groups.entry(k).or_default().push_back(track);
    }
    let mut result = Vec::with_capacity(groups.values().map(|g| g.len()).sum());
    let mut last: Option<String> = None;
    while !groups.is_empty() {
        let pick = order
            .iter()
            .filter(|k| groups.contains_key(*k) && Some(*k) != last.as_ref())
            .max_by_key(|k| groups[*k].len())
            .or_else(|| order.iter().find(|k| groups.contains_key(*k)))
            .cloned()
            .expect("a non-empty group");
        let group = groups.get_mut(&pick).expect("picked group");
        result.push(group.pop_front().expect("non-empty"));
        if group.is_empty() {
            groups.remove(&pick);
        }
        last = Some(pick);
    }
    result
}

const FIELDS: &[(&str, &str, bool)] = &[
    ("title", "t.title", false),
    ("artist", "t.artist", false),
    ("album", "t.album", false),
    ("album_artist", "t.album_artist", false),
    ("genre", "t.genre", false),
    ("format", "t.format", false),
    ("path", "t.path", false),
    ("folder", "needle_folder(t.path)", false),
    ("year", "t.year", true),
    ("bpm", "t.bpm", true),
    ("rating", "t.rating", true),
    ("duration", "t.duration", true),
    ("length_seconds", "t.duration", true),
    ("sample_rate", "t.sample_rate", true),
    ("bit_depth", "t.bit_depth", true),
    ("play_count", "t.play_count", true),
    ("added_at", "t.added_at", true),
    ("last_played", "t.last_played", true),
    ("track_number", "t.track_number", true),
    ("disc", "t.disc", true),
    ("bitrate", "json_extract(t.data,'$.bitrate')", true),
    ("channels", "json_extract(t.data,'$.channels')", true),
    ("replay_gain", "json_extract(t.data,'$.replay_gain')", true),
    (
        "musicbrainz_id",
        "json_extract(t.data,'$.musicbrainz_id')",
        false,
    ),
];

/// Words that, right after a field name, make the input a rule rather than plain text.
pub(crate) const FIELD_OPERATORS: &[&str] = &[
    "contains", "starts", "ends", "matches", "in", "between", "not",
];
/// Words that on their own make the input a rule.
pub(crate) const RULE_WORDS: &[&str] = &[
    "order", "limit", "shuffle", "missing", "favorite", "unplayed",
];

fn tokenize(input: &str) -> Result<Vec<Token>> {
    if input.len() > 8192 {
        bail!("Query is too long (maximum 8,192 characters)")
    }
    let chars: Vec<char> = input.chars().collect();
    let mut result = vec![];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => {
                result.push(Token::Left);
                i += 1;
            }
            ')' => {
                result.push(Token::Right);
                i += 1;
            }
            ',' => {
                result.push(Token::Comma);
                i += 1;
            }
            '\'' | '"' => {
                let quote = c;
                i += 1;
                let mut text = String::new();
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == quote {
                        i += 1;
                        closed = true;
                        break;
                    }
                    if chars[i] == '\\' {
                        i += 1;
                        if i >= chars.len() {
                            bail!("Unfinished escape in quoted text")
                        }
                    }
                    text.push(chars[i]);
                    i += 1;
                }
                if !closed {
                    bail!("Missing closing quote")
                }
                result.push(Token::Text(text));
            }
            '=' | '!' | '>' | '<' | ':' => {
                let mut op = c.to_string();
                i += 1;
                if i < chars.len() && chars[i] == '=' {
                    op.push('=');
                    i += 1;
                }
                if !["=", "==", "!=", ">", "<", ">=", "<=", ":"].contains(&op.as_str()) {
                    bail!("Unknown operator {op}")
                }
                result.push(Token::Op(op));
            }
            _ => {
                let start = i;
                while i < chars.len()
                    && !chars[i].is_whitespace()
                    && !"(),=!:<>\"'".contains(chars[i])
                {
                    i += 1;
                }
                if start == i {
                    bail!("Unexpected character at position {}", i + 1)
                }
                let word: String = chars[start..i].iter().collect();
                if let Ok(n) = word.parse::<f64>() {
                    if !n.is_finite() {
                        bail!("Number must be finite")
                    }
                    result.push(Token::Number(n));
                } else {
                    result.push(Token::Word(word));
                }
            }
        }
    }
    Ok(result)
}

pub fn compile(input: &str, now: i64) -> Result<Query> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Query { sql: "1".into(), parameters: vec![], order: "t.album_artist COLLATE NOCASE, t.album COLLATE NOCASE, t.disc, t.track_number, t.title COLLATE NOCASE, t.id".into(), limit: 500_000, explanation: "All available tracks in your library".into(), per: None, spread: None });
    }
    let tokens = tokenize(trimmed)?;
    let is_expression = tokens.iter().any(|t| matches!(t, Token::Op(_) | Token::Left))
        || tokens.first().is_some_and(|t| matches!(t, Token::Word(w) if RULE_WORDS.contains(&w.to_lowercase().as_str())))
        || tokens.windows(2).any(|pair| {
            matches!((&pair[0], &pair[1]), (Token::Word(field), Token::Word(op))
                if FIELD_OPERATORS.contains(&op.to_lowercase().as_str()) && field_info(field).is_ok())
        });
    if !is_expression {
        let mut sql = "(t.title LIKE ? ESCAPE '\\' OR t.artist LIKE ? ESCAPE '\\' OR t.album LIKE ? ESCAPE '\\' OR t.genre LIKE ? ESCAPE '\\')".to_string();
        let text = if let [Token::Text(value)] = tokens.as_slice() {
            value.clone()
        } else {
            trimmed.to_string()
        };
        let mut parameters = vec![Value::Text(like(&text)); 4];
        if text.chars().count() >= 3 {
            sql = "t.rowid IN (SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ?)".into();
            parameters = vec![Value::Text(format!("\"{}\"", text.replace('"', "\"\"")))];
        }
        return Ok(Query {
            sql,
            parameters,
            order: "t.artist COLLATE NOCASE, t.album COLLATE NOCASE, t.disc, t.track_number, t.id"
                .into(),
            limit: 500_000,
            explanation: format!("Title, artist, album, or genre contains “{text}”"),
            per: None,
            spread: None,
        });
    }
    let mut parser = Parser {
        tokens,
        index: 0,
        parameters: vec![],
        now,
        depth: 0,
    };
    let sql = if parser.word_is("order") || parser.word_is("limit") || parser.word_is("shuffle") {
        "1".into()
    } else {
        parser.or()?
    };
    let mut order =
        "t.artist COLLATE NOCASE, t.album COLLATE NOCASE, t.disc, t.track_number, t.id".to_string();
    let mut limit = 500_000;
    let mut spread_by = None;
    let mut per = None;
    if parser.eat_word("shuffle") {
        order = "random()".into();
        if parser.eat_word("by") {
            let field = parser.take_word()?.to_lowercase();
            spread_by = Some(match field.as_str() {
                "artist" | "album_artist" => "artist",
                "album" => "album",
                "genre" => "genre",
                _ => bail!("Shuffle can keep apart artist, album, or genre, not “{field}”"),
            });
        }
    } else if parser.eat_word("order") {
        parser.expect_word("by")?;
        let mut keys = vec![];
        loop {
            let field = parser.take_word()?;
            let (_, sql_field, _) = field_info(&field)?;
            let descending = parser.eat_word("desc");
            if !descending {
                parser.eat_word("asc");
            }
            keys.push(format!(
                "{sql_field} {}",
                if descending { "DESC" } else { "ASC" }
            ));
            if matches!(parser.tokens.get(parser.index), Some(Token::Comma)) {
                parser.index += 1;
            } else {
                break;
            }
        }
        order = format!("{}, t.id", keys.join(", "));
    }
    let limit_number = |parser: &mut Parser| -> Result<usize> {
        match parser.take() {
            Some(Token::Number(n)) if (1.0..=500_000.0).contains(&n) && n.fract() == 0.0 => {
                Ok(n as usize)
            }
            Some(Token::Number(_)) => bail!("Limit must be an integer between 1 and 500,000"),
            _ => bail!("Expected a number after limit"),
        }
    };
    if parser.eat_word("limit") {
        let n = limit_number(&mut parser)?;
        if parser.eat_word("per") {
            let field = parser.take_word()?;
            let (name, column, _) = field_info(&field)?;
            if !["artist", "album", "album_artist", "genre", "year", "format"].contains(&name) {
                bail!("Use limit N per artist, album, album_artist, genre, year, or format")
            }
            // Albums are only distinct together with their album artist.
            let column = if name == "album" {
                "t.album_artist || char(0) || t.album".to_string()
            } else {
                column.to_string()
            };
            per = Some((column, n));
            if parser.eat_word("limit") {
                limit = limit_number(&mut parser)?;
            }
        } else {
            limit = n;
        }
    }
    if parser.index != parser.tokens.len() {
        bail!(
            "Unexpected token {:?}. Use and/or between conditions.",
            parser.tokens[parser.index]
        )
    }
    Ok(Query {
        sql,
        parameters: parser.parameters,
        order,
        limit,
        explanation: format!("Matches rule: {trimmed}"),
        per,
        spread: spread_by,
    })
}

fn field_info(name: &str) -> Result<(&'static str, &'static str, bool)> {
    let name = match name.to_lowercase().as_str() {
        "length" => "duration".to_string(),
        "plays" => "play_count".to_string(),
        "added" => "added_at".to_string(),
        other => other.to_string(),
    };
    FIELDS.iter().copied().find(|(field, _, _)| *field == name).ok_or_else(|| anyhow::anyhow!("Unknown field “{name}”. Try artist, album, genre, year, bpm, rating, duration, or play_count."))
}
fn like(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    parameters: Vec<Value>,
    now: i64,
    depth: usize,
}
impl Parser {
    fn take(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }
    fn word_is(&self, word: &str) -> bool {
        matches!(self.tokens.get(self.index), Some(Token::Word(w)) if w.eq_ignore_ascii_case(word))
    }
    fn eat_word(&mut self, word: &str) -> bool {
        if self.word_is(word) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn expect_word(&mut self, word: &str) -> Result<()> {
        if !self.eat_word(word) {
            bail!("Expected “{word}”")
        }
        Ok(())
    }
    fn take_word(&mut self) -> Result<String> {
        match self.take() {
            Some(Token::Word(w)) => Ok(w),
            _ => bail!("Expected a field name"),
        }
    }
    fn text(&mut self, field: &str) -> Result<String> {
        match self.take() {
            Some(Token::Text(s) | Token::Word(s)) => Ok(s),
            Some(Token::Number(n)) => Ok(n.to_string()),
            _ => bail!("Expected quoted text after {field}"),
        }
    }
    /// A number for `field`. Lengths accept `3:30`, `4m`, `90s`, or `1h`; dates accept
    /// `2024-05-01` (local midnight) for added_at and last_played.
    fn number(&mut self, field: &str) -> Result<f64> {
        let length = matches!(field, "duration" | "length_seconds");
        match self.take() {
            Some(Token::Number(first)) => {
                if length && matches!(self.tokens.get(self.index), Some(Token::Op(op)) if op == ":")
                {
                    let mut total = first;
                    while matches!(self.tokens.get(self.index), Some(Token::Op(op)) if op == ":") {
                        self.index += 1;
                        match self.take() {
                            Some(Token::Number(part)) if (0.0..60.0).contains(&part) => {
                                total = total * 60. + part
                            }
                            _ => bail!("Write lengths like 3:30 or 1:02:00"),
                        }
                    }
                    return Ok(total);
                }
                Ok(first)
            }
            Some(Token::Word(word)) if length => {
                let (number, unit) = word.split_at(word.len().saturating_sub(1));
                let value: f64 = number
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Write lengths like 4m, 90s, 3:30, or 1h"))?;
                Ok(value
                    * match unit {
                        "s" => 1.,
                        "m" => 60.,
                        "h" => 3600.,
                        _ => bail!("Length units are s, m, or h"),
                    })
            }
            Some(Token::Word(word)) if matches!(field, "added_at" | "last_played") => {
                let date = chrono::NaiveDate::parse_from_str(&word, "%Y-%m-%d")
                    .map_err(|_| anyhow::anyhow!("Write dates as YYYY-MM-DD, like 2024-05-01"))?;
                let midnight = date.and_hms_opt(0, 0, 0).expect("valid midnight");
                Ok(midnight
                    .and_local_timezone(chrono::Local)
                    .earliest()
                    .map(|t| t.timestamp())
                    .unwrap_or_else(|| midnight.and_utc().timestamp()) as f64)
            }
            _ => bail!("{field} requires a number"),
        }
    }
    /// `field in ("a", "b")` or `field in (1, 2)`.
    fn in_list(&mut self, field: &str, col: &str, numeric: bool) -> Result<String> {
        if self.take() != Some(Token::Left) {
            bail!("Write in as a list, like {field} in (\"a\", \"b\")")
        }
        let mut marks = vec![];
        loop {
            if numeric {
                let n = self.number(field)?;
                marks.push(self.bind(Value::Real(n)));
            } else {
                let text = self.text(field)?;
                marks.push(self.bind(Value::Text(text)));
            }
            if marks.len() > 500 {
                bail!("A list can hold at most 500 values")
            }
            match self.take() {
                Some(Token::Comma) => continue,
                Some(Token::Right) => break,
                _ => bail!("Separate list values with commas and close with )"),
            }
        }
        Ok(format!(
            "{col}{} IN ({})",
            if numeric { "" } else { " COLLATE NOCASE" },
            marks.join(", ")
        ))
    }
    fn bind(&mut self, value: Value) -> String {
        self.parameters.push(value);
        "?".into()
    }
    fn or(&mut self) -> Result<String> {
        let mut left = self.and()?;
        while self.eat_word("or") {
            left = format!("({left} OR {})", self.and()?);
        }
        Ok(left)
    }
    fn and(&mut self) -> Result<String> {
        let mut left = self.unary()?;
        while self.eat_word("and") {
            left = format!("({left} AND {})", self.unary()?);
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<String> {
        self.depth += 1;
        if self.depth > 48 {
            bail!("Query nesting is too deep (maximum 48)")
        }
        let result = if self.eat_word("not") {
            Ok(format!("NOT ({})", self.unary()?))
        } else if matches!(self.tokens.get(self.index), Some(Token::Left)) {
            self.index += 1;
            let value = self.or()?;
            if self.take() != Some(Token::Right) {
                bail!("Missing closing parenthesis")
            }
            Ok(format!("({value})"))
        } else {
            self.predicate()
        };
        self.depth -= 1;
        result
    }
    fn predicate(&mut self) -> Result<String> {
        let name = self.take_word()?.to_lowercase();
        match name.as_str() {
            "missing" => return Ok("t.missing = 1".into()),
            "favorite" | "favourite" => return Ok("t.rating >= 4".into()),
            "unplayed" => return Ok("t.play_count = 0".into()),
            _ => {}
        }
        if matches!(self.tokens.get(self.index), Some(Token::Left)) {
            self.index += 1;
            if name == "exists" {
                let field = self.take_word()?;
                let (_, col, numeric) = field_info(&field)?;
                if self.take() != Some(Token::Right) {
                    bail!("Expected ) after exists(field)")
                }
                return Ok(if numeric {
                    format!("{col} IS NOT NULL")
                } else {
                    format!("({col} IS NOT NULL AND {col} != '')")
                });
            }
            if !["recent", "played", "skipped"].contains(&name.as_str()) {
                bail!(
                    "Unknown function “{name}”. Available: recent(30d), played(7d), skipped(30d), exists(bpm)."
                )
            }
            let token = self
                .take()
                .ok_or_else(|| anyhow::anyhow!("Expected a duration like 7d"))?;
            let mut year_range = None;
            let since = match token {
                Token::Word(duration) => {
                    let (number, unit) = duration.split_at(duration.len().saturating_sub(1));
                    let value: i64 = number
                        .parse()
                        .map_err(|_| anyhow::anyhow!("Use a duration such as 7d, 12h, or 30m"))?;
                    let multiplier = match unit {
                        "d" => 86400,
                        "h" => 3600,
                        "m" => 60,
                        "w" => 604800,
                        _ => bail!("Duration unit must be m, h, d, or w"),
                    };
                    if !(1..=36500).contains(&value) {
                        bail!("Duration is outside the supported range")
                    }
                    self.now - value * multiplier
                }
                Token::Number(year)
                    if name == "played"
                        && year.fract() == 0.0
                        && (1970.0..=9998.0).contains(&year) =>
                {
                    let start = chrono::NaiveDate::from_ymd_opt(year as i32, 1, 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    let end = chrono::NaiveDate::from_ymd_opt(year as i32 + 1, 1, 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    year_range = Some(end);
                    start
                }
                _ => bail!("Use a duration such as 7d, or played(2025) for a calendar year"),
            };
            if self.take() != Some(Token::Right) {
                bail!("Expected ) after duration")
            }
            self.bind(Value::Integer(since));
            if name == "recent" {
                return Ok("t.added_at >= ?".into());
            }
            let mut sql = format!(
                "EXISTS (SELECT 1 FROM listens h WHERE h.track_id=t.id AND h.qualified={} AND h.started_at>=?",
                if name == "skipped" { 0 } else { 1 }
            );
            if let Some(end) = year_range {
                self.bind(Value::Integer(end));
                sql.push_str(" AND h.started_at<?");
            }
            sql.push(')');
            return Ok(sql);
        }
        let (field, col, numeric) = field_info(&name)?;
        let negate = self.eat_word("not");
        let keyword = match self.tokens.get(self.index) {
            Some(Token::Word(w)) => Some(w.to_lowercase()),
            _ => None,
        };
        let special = match keyword.as_deref() {
            Some("in") => {
                self.index += 1;
                Some(self.in_list(field, col, numeric)?)
            }
            Some("between") if numeric => {
                self.index += 1;
                let low = self.number(field)?;
                self.expect_word("and")?;
                let high = self.number(field)?;
                let (low, high) = if low <= high {
                    (low, high)
                } else {
                    (high, low)
                };
                let (a, b) = (self.bind(Value::Real(low)), self.bind(Value::Real(high)));
                Some(format!("{col} BETWEEN {a} AND {b}"))
            }
            Some("starts" | "ends") if !numeric => {
                let starts = keyword.as_deref() == Some("starts");
                self.index += 1;
                self.expect_word("with")?;
                let text = self.text(field)?;
                let escaped = like(&text);
                let pattern = if starts {
                    format!("{}%", &escaped[1..escaped.len() - 1])
                } else {
                    format!("%{}", &escaped[1..escaped.len() - 1])
                };
                self.bind(Value::Text(pattern));
                Some(format!("{col} LIKE ? ESCAPE '\\'"))
            }
            Some("matches") if !numeric => {
                self.index += 1;
                let pattern = self.text(field)?;
                regex::RegexBuilder::new(&pattern)
                    .case_insensitive(true)
                    .size_limit(1 << 20)
                    .build()
                    .map_err(|e| {
                        anyhow::anyhow!("That pattern is not a valid regular expression: {e}")
                    })?;
                self.bind(Value::Text(pattern));
                Some(format!("needle_regexp(?, {col})"))
            }
            _ => None,
        };
        if let Some(sql) = special {
            return Ok(if negate { format!("NOT ({sql})") } else { sql });
        }
        if negate && keyword.as_deref() != Some("contains") {
            bail!("Use not before in, between, contains, starts with, ends with, or matches")
        }
        let op = match self.take() {
            Some(Token::Op(op)) => op,
            Some(Token::Word(w)) if w.eq_ignore_ascii_case("contains") => ":".into(),
            _ => bail!("Expected comparison after {name}, such as {name} = ..."),
        };
        let value = self
            .take()
            .ok_or_else(|| anyhow::anyhow!("Missing value after {name} {op}"))?;
        if matches!(&value, Token::Word(w) if w.eq_ignore_ascii_case("null")) {
            return match op.as_str() {
                "=" | "==" => Ok(format!("{col} IS NULL")),
                "!=" => Ok(format!("{col} IS NOT NULL")),
                _ => bail!("Use = null or != null"),
            };
        }
        if numeric {
            if op == ":" {
                bail!("{name} is numeric; use =, >, <, >=, <=, or !=")
            }
            self.index -= 1;
            let number = self.number(field)?;
            let _ = value;
            self.bind(Value::Real(number));
            Ok(format!("{col} {} ?", if op == "==" { "=" } else { &op }))
        } else {
            let text = match value {
                Token::Text(s) | Token::Word(s) => s,
                Token::Number(n) => n.to_string(),
                _ => bail!("Expected quoted text after {name}"),
            };
            if op == ":" {
                self.bind(Value::Text(like(&text)));
                let sql = format!("{col} LIKE ? ESCAPE '\\'");
                Ok(if negate { format!("NOT ({sql})") } else { sql })
            } else if ["=", "==", "!="].contains(&op.as_str()) {
                self.bind(Value::Text(text));
                Ok(format!(
                    "{col} {} ? COLLATE NOCASE",
                    if op == "==" { "=" } else { &op }
                ))
            } else {
                bail!("Text fields support =, !=, or contains")
            }
        }
    }
}

pub fn format_track(template: &str, track: &Track) -> Result<String> {
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let end = rest[start..]
            .find('}')
            .ok_or_else(|| anyhow::anyhow!("Missing closing }} in template"))?
            + start;
        let key = &rest[start + 1..end];
        let value = match key {
            "title" => track.title.clone(),
            "artist" => track.display_artist().into(),
            "album" => track.display_album().into(),
            "album_artist" => track.album_artist.clone(),
            "genre" => track.genre.clone(),
            "year" => track.year.to_string(),
            "track_number" => format!("{:02}", track.track_number),
            "format" => track.format.clone(),
            "duration" => crate::model::format_duration(track.duration),
            "rating" => track.rating.to_string(),
            "play_count" => track.play_count.to_string(),
            _ => bail!("Unknown template field {{{key}}}"),
        };
        result.push_str(&value);
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_queries_and_escaping() {
        let q = compile(
            "recent(30d) and bpm > 120 and not played(7d) order by rating desc limit 20",
            2_000_000_000,
        )
        .unwrap();
        assert_eq!(q.parameters.len(), 3);
        assert_eq!(q.limit, 20);
        assert!(q.sql.contains("EXISTS"));
        let q = compile("artist = \"O'Brien\"", 0).unwrap();
        assert!(!q.sql.contains("O'Brien"));
        assert_eq!(q.parameters, vec![Value::Text("O'Brien".into())]);
        assert!(compile("bpm > fast", 0).is_err());
        assert!(compile("(rating > 3", 0).is_err());
        assert!(compile("rating > 2; DROP TABLE tracks", 0).is_err());
        assert!(
            compile("title contains '100%_mix'", 0)
                .unwrap()
                .parameters
                .contains(&Value::Text("%100\\%\\_mix%".into()))
        );
    }
    #[test]
    fn precedence_and_depth() {
        let q = compile("artist = a or artist = b and rating >= 4", 0).unwrap();
        assert!(q.sql.starts_with("(t.artist = ? COLLATE NOCASE OR ("));
        assert!(
            compile(
                &format!("{}rating = 1{}", "(".repeat(60), ")".repeat(60)),
                0
            )
            .is_err()
        );
    }
    #[test]
    fn ordinary_titles_and_leading_zeroes_are_preserved() {
        let query = compile("Artist 00420", 0).unwrap();
        assert!(
            query
                .parameters
                .contains(&Value::Text("\"Artist 00420\"".into()))
        );
        let query = compile("Earth Wind and Fire", 0).unwrap();
        assert!(
            query
                .parameters
                .contains(&Value::Text("\"Earth Wind and Fire\"".into()))
        );
    }
}

#[cfg(test)]
mod richer_rules {
    use super::*;
    use crate::{database::Library, model::Track};

    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let rows = [
            (
                "1",
                "Army of Me",
                "Björk",
                "Post",
                "Electronic",
                1995,
                234.,
                2,
            ),
            (
                "2",
                "Hyperballad",
                "Björk",
                "Post",
                "Electronic",
                1995,
                321.,
                2,
            ),
            ("3", "Isobel", "Björk", "Post", "Electronic", 1995, 347., 2),
            (
                "4",
                "Teardrop",
                "Massive Attack",
                "Mezzanine",
                "Trip hop",
                1998,
                330.,
                2,
            ),
            (
                "5",
                "Angel",
                "Massive Attack",
                "Mezzanine",
                "Trip hop",
                1998,
                379.,
                2,
            ),
            (
                "6",
                "Glory Box",
                "Portishead",
                "Dummy",
                "Trip hop",
                1994,
                306.,
                1,
            ),
            (
                "7",
                "Sour Times",
                "Portishead",
                "Dummy",
                "Trip hop",
                1994,
                251.,
                2,
            ),
        ];
        for (id, title, artist, album, genre, year, duration, channels) in rows {
            library
                .upsert(&Track {
                    id: id.into(),
                    path: format!("C:/music/{id}.flac"),
                    title: title.into(),
                    artist: artist.into(),
                    album: album.into(),
                    album_artist: artist.into(),
                    genre: genre.into(),
                    year,
                    duration,
                    channels,
                    bitrate: 900 + year,
                    format: "FLAC".into(),
                    ..Default::default()
                })
                .unwrap();
        }
        library.rate("2", 5).unwrap();
        library.rate("6", 4).unwrap();
        (dir, library)
    }
    fn titles(library: &Library, rule: &str) -> Vec<String> {
        library
            .search(rule)
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect()
    }

    #[test]
    fn text_operators() {
        let (_dir, lib) = library();
        assert_eq!(titles(&lib, "title starts with \"s\""), ["Sour Times"]);
        assert_eq!(
            titles(&lib, "title ends with \"ball\""),
            Vec::<String>::new()
        );
        assert_eq!(titles(&lib, "title ends with \"ballad\""), ["Hyperballad"]);
        assert_eq!(
            titles(&lib, "title matches \"^(angel|teardrop)$\" order by title"),
            ["Angel", "Teardrop"]
        );
        assert_eq!(
            titles(
                &lib,
                "artist in (\"portishead\", \"björk\") and title contains \"o\" order by title"
            )
            .len(),
            4
        );
        assert_eq!(
            titles(
                &lib,
                "artist not in (\"Björk\", \"Portishead\") order by title"
            ),
            ["Angel", "Teardrop"]
        );
        assert_eq!(titles(&lib, "genre not contains \"hop\"").len(), 3);
        assert!(compile("title matches \"(unclosed\"", 0).is_err());
        assert!(compile("year starts with \"19\"", 0).is_err());
    }

    #[test]
    fn numbers_lengths_and_dates() {
        let (_dir, lib) = library();
        assert_eq!(
            titles(&lib, "year between 1998 and 1995").len(),
            5,
            "bounds in either order"
        );
        assert_eq!(
            titles(&lib, "year in (1995) and duration > 5:00 order by title"),
            ["Hyperballad", "Isobel"]
        );
        assert_eq!(titles(&lib, "length < 4m"), ["Army of Me"]);
        assert_eq!(titles(&lib, "duration >= 6m"), ["Angel"]);
        assert_eq!(titles(&lib, "channels = 1"), ["Glory Box"]);
        assert_eq!(titles(&lib, "bitrate > 2895").len(), 2);
        assert_eq!(
            titles(&lib, "favorite order by title"),
            ["Glory Box", "Hyperballad"]
        );
        assert_eq!(titles(&lib, "unplayed").len(), 7);
        assert_eq!(
            titles(&lib, "added_at > 2000-01-01").len(),
            0,
            "tracks were added at time zero"
        );
        assert!(compile("added_at > yesterday", 0).is_err());
        assert!(compile("duration > 3:75", 0).is_err());
    }

    #[test]
    fn per_group_limits_multi_sort_and_spread() {
        let (_dir, lib) = library();
        let one_each = lib.search("limit 1 per artist").unwrap();
        assert_eq!(one_each.len(), 3);
        let page = lib
            .search_page("genre = \"Trip hop\" limit 1 per album", 0, 10)
            .unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(
            titles(&lib, "order by year desc, title asc limit 3"),
            ["Angel", "Teardrop", "Army of Me"]
        );
        let spread = lib.search("shuffle by artist").unwrap();
        assert_eq!(spread.len(), 7);
        for pair in spread.windows(2) {
            assert_ne!(
                pair[0].artist, pair[1].artist,
                "neighbours share an artist: {pair:?}"
            );
        }
        assert!(compile("shuffle by year", 0).is_err());
        assert!(compile("limit 2 per bpm", 0).is_err());
    }

    #[test]
    fn spread_keeps_order_within_groups_and_handles_impossible_cases() {
        let track = |artist: &str, title: &str| Track {
            artist: artist.into(),
            title: title.into(),
            ..Default::default()
        };
        let out = spread(
            vec![
                track("a", "1"),
                track("a", "2"),
                track("a", "3"),
                track("b", "4"),
            ],
            "artist",
        );
        let order: Vec<_> = out.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            order,
            ["1", "4", "2", "3"],
            "one b can separate only one pair"
        );
        assert!(spread(vec![], "artist").is_empty());
    }
}
