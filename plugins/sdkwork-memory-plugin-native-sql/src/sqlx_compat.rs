//! Portable SQLx Any queries with PostgreSQL-compatible numbered placeholders.

use std::collections::{HashMap, VecDeque};
use std::sync::{OnceLock, RwLock};

pub(crate) use ::sqlx::{any, Any, AnyPool, Error, Row, Transaction};

/// Bound on the number of distinct normalized query shapes held in memory.
///
/// Filter inputs (metadata predicate trees, variable-length IN lists) can
/// produce unbounded distinct shapes, so the cache evicts its oldest entry
/// (FIFO, no read-path lock traffic) when the bound is reached and re-normalizes
/// that shape if it returns. Leaking entries and asserting on overflow are both
/// gone — an attacker-controlled filter shape can no longer grow memory without
/// bound or panic the process.
const MAX_CACHED_QUERY_SHAPES: usize = 2048;

struct NormalizedSqlCache {
    map: HashMap<String, &'static str>,
    order: VecDeque<String>,
}

static NORMALIZED_SQL: OnceLock<RwLock<NormalizedSqlCache>> = OnceLock::new();

// sqlx 0.9 declares `Database::Arguments` without a lifetime parameter
// (`type Arguments: Arguments<Database = Self>`), so these return types must not
// carry an `<'q>` on the associated type. The statement lifetime stays on
// `Query<'q, ...>` / `QueryScalar<'q, ...>`.
pub(crate) fn query<'q, DB>(
    sql: &str,
) -> ::sqlx::query::Query<'q, DB, <DB as ::sqlx::Database>::Arguments>
where
    DB: ::sqlx::Database,
{
    ::sqlx::query(normalized_sql(sql))
}

pub(crate) fn query_scalar<'q, DB, O>(
    sql: &str,
) -> ::sqlx::query::QueryScalar<'q, DB, O, <DB as ::sqlx::Database>::Arguments>
where
    DB: ::sqlx::Database,
    (O,): for<'row> ::sqlx::FromRow<'row, DB::Row>,
{
    ::sqlx::query_scalar(normalized_sql(sql))
}

fn normalized_sql(sql: &str) -> &'static str {
    let cache = NORMALIZED_SQL.get_or_init(|| {
        RwLock::new(NormalizedSqlCache {
            map: HashMap::new(),
            order: VecDeque::new(),
        })
    });
    if let Some(value) = cache.read().expect("SQL cache read lock").map.get(sql) {
        return value;
    }

    let normalized = number_placeholders(sql);
    let mut write = cache.write().expect("SQL cache write lock");
    if let Some(value) = write.map.get(sql) {
        return value;
    }
    if write.map.len() >= MAX_CACHED_QUERY_SHAPES {
        // LRU eviction: drop the least recently used shape (its leaked string
        // stays alive for the process — bounded by the same limit — and the
        // shape is re-normalized if it returns).
        while write.map.len() >= MAX_CACHED_QUERY_SHAPES {
            if let Some(oldest) = write.order.pop_front() {
                write.map.remove(&oldest);
            }
        }
    }
    let value = Box::leak(normalized.into_boxed_str());
    write.map.insert(sql.to_string(), value);
    write.order.push_back(sql.to_string());
    value
}

fn number_placeholders(sql: &str) -> String {
    let chars = sql.as_bytes();
    let mut output = String::with_capacity(sql.len() + 16);
    let mut index = 0usize;
    let mut placeholder = 1usize;
    let mut state = SqlLexState::Normal;

    while index < chars.len() {
        match state {
            SqlLexState::Normal => {
                if chars[index] == b'\'' {
                    state = SqlLexState::SingleQuoted;
                    output.push('\'');
                    index += 1;
                } else if chars[index] == b'"' {
                    state = SqlLexState::DoubleQuoted;
                    output.push('"');
                    index += 1;
                } else if chars[index..].starts_with(b"--") {
                    state = SqlLexState::LineComment;
                    output.push_str("--");
                    index += 2;
                } else if chars[index..].starts_with(b"/*") {
                    state = SqlLexState::BlockComment;
                    output.push_str("/*");
                    index += 2;
                } else if chars[index] == b'$' {
                    if let Some(end) = dollar_quote_tag_end(chars, index) {
                        let tag = sql[index..=end].to_string();
                        output.push_str(&tag);
                        index = end + 1;
                        state = SqlLexState::DollarQuoted(tag);
                    } else {
                        output.push('$');
                        index += 1;
                    }
                } else if chars[index] == b'?'
                    && !matches!(chars.get(index + 1), Some(b'|') | Some(b'&'))
                {
                    output.push('$');
                    output.push_str(&placeholder.to_string());
                    placeholder += 1;
                    index += 1;
                } else {
                    output.push(chars[index] as char);
                    index += 1;
                }
            }
            SqlLexState::SingleQuoted => {
                output.push(chars[index] as char);
                if chars[index] == b'\'' {
                    if chars.get(index + 1) == Some(&b'\'') {
                        output.push('\'');
                        index += 2;
                    } else {
                        index += 1;
                        state = SqlLexState::Normal;
                    }
                } else {
                    index += 1;
                }
            }
            SqlLexState::DoubleQuoted => {
                output.push(chars[index] as char);
                if chars[index] == b'"' {
                    if chars.get(index + 1) == Some(&b'"') {
                        output.push('"');
                        index += 2;
                    } else {
                        index += 1;
                        state = SqlLexState::Normal;
                    }
                } else {
                    index += 1;
                }
            }
            SqlLexState::LineComment => {
                output.push(chars[index] as char);
                if chars[index] == b'\n' {
                    state = SqlLexState::Normal;
                }
                index += 1;
            }
            SqlLexState::BlockComment => {
                if chars[index..].starts_with(b"*/") {
                    output.push_str("*/");
                    index += 2;
                    state = SqlLexState::Normal;
                } else {
                    output.push(chars[index] as char);
                    index += 1;
                }
            }
            SqlLexState::DollarQuoted(ref tag) => {
                if sql[index..].starts_with(tag) {
                    output.push_str(tag);
                    index += tag.len();
                    state = SqlLexState::Normal;
                } else {
                    output.push(chars[index] as char);
                    index += 1;
                }
            }
        }
    }
    output
}

fn dollar_quote_tag_end(chars: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == b'_') {
        index += 1;
    }
    (chars.get(index) == Some(&b'$')).then_some(index)
}

#[derive(Clone)]
enum SqlLexState {
    Normal,
    SingleQuoted,
    DoubleQuoted,
    LineComment,
    BlockComment,
    DollarQuoted(String),
}

#[cfg(test)]
mod tests {
    use super::number_placeholders;

    #[test]
    fn numbers_bind_markers_without_touching_literals_or_comments() {
        let sql =
            "SELECT '?' AS literal, value FROM sample -- ? comment\nWHERE a = ? AND b = ? /* ? */";
        assert_eq!(
            number_placeholders(sql),
            "SELECT '?' AS literal, value FROM sample -- ? comment\nWHERE a = $1 AND b = $2 /* ? */"
        );
    }

    #[test]
    fn preserves_postgres_json_and_dollar_quoted_operators() {
        let sql = "SELECT payload ?| array['a'] FROM sample WHERE id = ?; $$ ? $$";
        assert_eq!(
            number_placeholders(sql),
            "SELECT payload ?| array['a'] FROM sample WHERE id = $1; $$ ? $$"
        );
    }
}
