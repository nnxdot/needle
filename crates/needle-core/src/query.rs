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
}

const FIELDS: &[(&str, &str, bool)] = &[
    ("title", "t.title", false),
    ("artist", "t.artist", false),
    ("album", "t.album", false),
    ("album_artist", "t.album_artist", false),
    ("genre", "t.genre", false),
    ("format", "t.format", false),
    ("path", "t.path", false),
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
        return Ok(Query { sql: "1".into(), parameters: vec![], order: "t.album_artist COLLATE NOCASE, t.album COLLATE NOCASE, t.disc, t.track_number, t.title COLLATE NOCASE, t.id".into(), limit: 500_000, explanation: "All available tracks in your library".into() });
    }
    let tokens = tokenize(trimmed)?;
    let is_expression = tokens.iter().any(|t| matches!(t, Token::Op(_) | Token::Left))
        || tokens.first().is_some_and(|t|matches!(t,Token::Word(w) if ["order","limit","shuffle","missing"].contains(&w.to_lowercase().as_str())))
        || tokens.windows(2).any(|pair|matches!((&pair[0],&pair[1]),(Token::Word(field),Token::Word(op)) if op.eq_ignore_ascii_case("contains")&&field_info(field).is_ok()));
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
    if parser.eat_word("shuffle") {
        order = "random()".into();
    } else if parser.eat_word("order") {
        parser.expect_word("by")?;
        let field = parser.take_word()?;
        let (_, sql_field, _) = field_info(&field)?;
        let descending = parser.eat_word("desc");
        if !descending {
            parser.eat_word("asc");
        }
        order = format!(
            "{} {}, t.id",
            sql_field,
            if descending { "DESC" } else { "ASC" }
        );
    }
    if parser.eat_word("limit") {
        if let Some(Token::Number(n)) = parser.take() {
            if n < 1.0 || n > 500_000.0 || n.fract() != 0.0 {
                bail!("Limit must be an integer between 1 and 500,000")
            }
            limit = n as usize;
        } else {
            bail!("Expected a number after limit")
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
    })
}

fn field_info(name: &str) -> Result<(&'static str, &'static str, bool)> {
    FIELDS.iter().copied().find(|(field, _, _)| *field == name.to_lowercase()).ok_or_else(|| anyhow::anyhow!("Unknown field “{name}”. Try artist, album, genre, year, bpm, rating, duration, or play_count."))
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
        if name == "missing" {
            return Ok("t.missing = 1".into());
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
            if name != "recent" && name != "played" {
                bail!("Unknown function “{name}”. Available: recent(30d), played(7d), exists(bpm).")
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
            let mut sql = "EXISTS (SELECT 1 FROM listens h WHERE h.track_id=t.id AND h.qualified=1 AND h.started_at>=?".to_string();
            if let Some(end) = year_range {
                self.bind(Value::Integer(end));
                sql.push_str(" AND h.started_at<?");
            }
            sql.push(')');
            return Ok(sql);
        }
        let (_, col, numeric) = field_info(&name)?;
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
            let number = match value {
                Token::Number(n) => n,
                _ => bail!("{name} requires a number"),
            };
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
                Ok(format!("{col} LIKE ? ESCAPE '\\'"))
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
