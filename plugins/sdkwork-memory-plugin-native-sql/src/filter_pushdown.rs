//! Metadata filter pushdown into native SQL.
//!
//! The SPI layer owns what a metadata filter *means*
//! (`sdkwork_memory_spi::MetadataFilterExpression`); this module owns how the two supported
//! dialects express it. The split exists because the translation is dialect specific while
//! the contract is not.
//!
//! # Pushdown, not post-filtering
//!
//! `PAGINATION_SPEC.md` section 5.1 requires data-scope predicates to reach the query rather
//! than being applied to a broad read, and section 2.2 names "over-fetch then filter in the
//! service" as a forbidden shape. The filter is therefore appended to the `WHERE` clause of
//! every query that can produce a candidate, including the full-text path — a filter that
//! only reached the `LIKE` fallback would silently stop applying whenever full text
//! succeeded.
//!
//! # Fail closed
//!
//! A translation returns [`NativeSqlStoreError::MetadataFilterUnsupported`] rather than
//! emitting a weaker predicate. The only case where a dialect genuinely falls short is
//! `icontains` with a non-ASCII needle on SQLite, whose `lower()` folds ASCII only; matching
//! it there would return fewer rows than PostgreSQL for the same filter, which is exactly
//! the silent divergence this module refuses.
//!
//! # Null handling
//!
//! Both dialects reproduce SQL three-valued logic so that they agree with each other and
//! with `MetadataFilterExpression::matches`:
//!
//! * a `NULL` metadata column makes every leaf `NULL`;
//! * a JSON `null` field value renders as SQL `NULL`, so no comparison against it is true;
//! * a missing key is `NULL` for comparisons and `false` for a presence test.
//!
//! SQLite reaches the same behaviour only with two explicit guards. Its `json_extract`
//! returns an integer for a JSON boolean, which would compare unequal to the text `true`
//! that PostgreSQL's `->>` produces, so [`metadata_text_expr`] dispatches on `json_type`.
//! Its `json_type(...) IS NOT NULL` would be false rather than unknown for a `NULL` column,
//! which only differs under `NOT`, so [`existence_predicate`] restores the `CASE` guard.
//!
//! # Binds
//!
//! Every parameter binds as text. Numeric bounds reach the query as their validated literal
//! text and are cast by the dialect, which keeps the digits the caller wrote and avoids
//! widening a bound through a floating-point round trip.

use sdkwork_memory_spi::{
    MetadataFilterCondition, MetadataFilterExpression, OrderingFilterOperator,
    ScalarFilterOperator, SetFilterOperator,
};

use crate::pool_backend::MemorySqlDialect;
use crate::privacy::escape_like_pattern;
use crate::store::NativeSqlStoreError;

/// A predicate fragment and the parameters it binds.
///
/// `binds` is in the order the placeholders appear in `sql`, so a caller appends the
/// fragment and then binds the parameters in sequence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SqlPredicate {
    /// Predicate SQL, leading with ` AND ` so it can be appended to a `WHERE` clause.
    /// Empty when no filter was supplied.
    pub(crate) sql: String,
    /// Text parameters, in placeholder order.
    pub(crate) binds: Vec<String>,
}

/// Translates a metadata filter into a SQL predicate for the active dialect.
///
/// Returns an empty predicate when `filter` is `None`, so callers can append unconditionally.
///
/// # Errors
///
/// Returns [`NativeSqlStoreError::MetadataFilterUnsupported`] when the dialect cannot
/// express an operator with the same semantics as the SPI contract. It never returns a
/// weaker predicate in place of a rejected one.
pub(crate) fn translate_metadata_filter(
    dialect: MemorySqlDialect,
    table_alias: &str,
    filter: Option<&MetadataFilterExpression>,
) -> Result<SqlPredicate, NativeSqlStoreError> {
    let Some(filter) = filter else {
        return Ok(SqlPredicate::default());
    };
    let mut predicate = SqlPredicate::default();
    predicate.sql.push_str(" AND (");
    render_expression(dialect, table_alias, filter, &mut predicate)?;
    predicate.sql.push(')');
    Ok(predicate)
}

/// Renders one expression node.
fn render_expression(
    dialect: MemorySqlDialect,
    table_alias: &str,
    expression: &MetadataFilterExpression,
    predicate: &mut SqlPredicate,
) -> Result<(), NativeSqlStoreError> {
    match expression {
        MetadataFilterExpression::Condition(condition) => {
            render_condition(dialect, table_alias, condition, predicate)
        }
        MetadataFilterExpression::All(children) => {
            render_group(dialect, table_alias, children, " AND ", predicate)
        }
        MetadataFilterExpression::Any(children) => {
            render_group(dialect, table_alias, children, " OR ", predicate)
        }
        MetadataFilterExpression::Not(child) => {
            predicate.sql.push_str("NOT (");
            render_expression(dialect, table_alias, child, predicate)?;
            predicate.sql.push(')');
            Ok(())
        }
    }
}

/// Renders a conjunction or disjunction.
fn render_group(
    dialect: MemorySqlDialect,
    table_alias: &str,
    children: &[MetadataFilterExpression],
    separator: &str,
    predicate: &mut SqlPredicate,
) -> Result<(), NativeSqlStoreError> {
    // The parser never produces an empty group, and an empty conjunction would evaluate to
    // true — widening the result set. A hand-built tree gets an explicit refusal instead.
    if children.is_empty() {
        return Err(unsupported(
            "a logical group carries no condition, which would widen the result set",
        ));
    }
    predicate.sql.push('(');
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            predicate.sql.push_str(separator);
        }
        render_expression(dialect, table_alias, child, predicate)?;
    }
    predicate.sql.push(')');
    Ok(())
}

/// Renders one leaf predicate.
fn render_condition(
    dialect: MemorySqlDialect,
    table_alias: &str,
    condition: &MetadataFilterCondition,
    predicate: &mut SqlPredicate,
) -> Result<(), NativeSqlStoreError> {
    match condition {
        MetadataFilterCondition::Exists { field } => {
            let presence = existence_predicate(dialect, table_alias, field, predicate);
            predicate.sql.push_str(&presence);
            Ok(())
        }
        MetadataFilterCondition::Scalar {
            field,
            operator,
            value,
        } => {
            let left = metadata_text_expr(dialect, table_alias, field, predicate);
            let operand = value.as_comparison_text();
            match (dialect, operator) {
                (MemorySqlDialect::Postgres, ScalarFilterOperator::Eq) => {
                    predicate.sql.push_str(&format!("{left} = ?"));
                    predicate.binds.push(operand);
                }
                (MemorySqlDialect::Postgres, ScalarFilterOperator::Ne) => {
                    predicate.sql.push_str(&format!("{left} != ?"));
                    predicate.binds.push(operand);
                }
                (MemorySqlDialect::Postgres, ScalarFilterOperator::Contains) => {
                    // PostgreSQL's LIKE is case sensitive, which is the contract.
                    predicate
                        .sql
                        .push_str(&format!(r"{left} LIKE ? ESCAPE '\'"));
                    predicate
                        .binds
                        .push(format!("%{}%", escape_like_pattern(&operand)));
                }
                (MemorySqlDialect::Postgres, ScalarFilterOperator::IContains) => {
                    predicate
                        .sql
                        .push_str(&format!(r"{left} ILIKE ? ESCAPE '\'"));
                    predicate
                        .binds
                        .push(format!("%{}%", escape_like_pattern(&operand)));
                }
                (MemorySqlDialect::Sqlite, ScalarFilterOperator::Eq) => {
                    predicate.sql.push_str(&format!("{left} = ?"));
                    predicate.binds.push(operand);
                }
                (MemorySqlDialect::Sqlite, ScalarFilterOperator::Ne) => {
                    predicate.sql.push_str(&format!("{left} != ?"));
                    predicate.binds.push(operand);
                }
                (MemorySqlDialect::Sqlite, ScalarFilterOperator::Contains) => {
                    // `instr` is case sensitive, matching the contract, and unlike LIKE it
                    // treats % and _ as ordinary characters, so the operand is not escaped.
                    predicate.sql.push_str(&format!("instr({left}, ?) > 0"));
                    predicate.binds.push(operand);
                }
                (MemorySqlDialect::Sqlite, ScalarFilterOperator::IContains) => {
                    // SQLite's lower() folds ASCII only, so a wider needle would match fewer
                    // rows than PostgreSQL. Refuse rather than diverge.
                    if !operand.is_ascii() {
                        return Err(unsupported(
                            "icontains with a non-ASCII operand cannot be expressed on SQLite, \
                             whose lower() folds ASCII only; PostgreSQL would match more rows",
                        ));
                    }
                    predicate
                        .sql
                        .push_str(&format!("instr(lower({left}), lower(?)) > 0"));
                    predicate.binds.push(operand);
                }
            }
            Ok(())
        }
        MetadataFilterCondition::Set {
            field,
            operator,
            values,
        } => {
            if values.is_empty() {
                return Err(unsupported(
                    "a set predicate carries no value, which would widen the result set",
                ));
            }
            let left = metadata_text_expr(dialect, table_alias, field, predicate);
            let placeholders = vec!["?"; values.len()].join(", ");
            match operator {
                SetFilterOperator::In => {
                    predicate
                        .sql
                        .push_str(&format!("{left} IN ({placeholders})"));
                }
                SetFilterOperator::Nin => {
                    // NOT IN over a NULL left side is NULL, which is the contract.
                    predicate
                        .sql
                        .push_str(&format!("{left} NOT IN ({placeholders})"));
                }
            }
            for value in values {
                predicate.binds.push(value.as_comparison_text());
            }
            Ok(())
        }
        MetadataFilterCondition::Ordering {
            field,
            operator,
            bound,
        } => {
            let comparison = match operator {
                OrderingFilterOperator::Gt => ">",
                OrderingFilterOperator::Gte => ">=",
                OrderingFilterOperator::Lt => "<",
                OrderingFilterOperator::Lte => "<=",
            };
            match dialect {
                MemorySqlDialect::Postgres => {
                    predicate.binds.push(field.clone());
                    predicate.binds.push(field.clone());
                    predicate.binds.push(bound.as_str().to_string());
                    predicate.sql.push_str(&format!(
                        "(CASE WHEN jsonb_typeof((({table_alias}.metadata_json)::jsonb) -> ?) = 'number' \
                         THEN ((({table_alias}.metadata_json)::jsonb) ->> ?)::numeric END) \
                         {comparison} ?::numeric"
                    ));
                }
                MemorySqlDialect::Sqlite => {
                    // A fixed SQL fragment rather than a formatted string: it carries its own
                    // `?` placeholder, which is why `field` is bound twice above.
                    let path = "('$.' || ?)";
                    predicate.binds.push(field.clone());
                    predicate.binds.push(field.clone());
                    predicate.binds.push(bound.as_str().to_string());
                    predicate.sql.push_str(&format!(
                        "(CASE WHEN json_type({table_alias}.metadata_json, {path}) \
                         IN ('integer','real') \
                         THEN CAST(json_extract({table_alias}.metadata_json, {path}) AS REAL) END) \
                         {comparison} CAST(? AS REAL)"
                    ));
                }
            }
            Ok(())
        }
    }
}

/// Builds the SQL that tests a metadata field for presence.
fn existence_predicate(
    dialect: MemorySqlDialect,
    table_alias: &str,
    field: &str,
    predicate: &mut SqlPredicate,
) -> String {
    predicate.binds.push(field.to_string());
    match dialect {
        // The function form of the `?` operator. The operator itself cannot be used here:
        // sqlx rewrites `?` into the backend's placeholder syntax, so a bare `?` operator
        // would be consumed as a parameter marker.
        MemorySqlDialect::Postgres => {
            format!("jsonb_exists(({table_alias}.metadata_json)::jsonb, ?)")
        }
        // `json_type(...) IS NOT NULL` alone would be false, not unknown, for a NULL column;
        // the CASE restores the unknown that PostgreSQL and the SPI contract produce.
        MemorySqlDialect::Sqlite => format!(
            "(CASE WHEN {table_alias}.metadata_json IS NULL THEN NULL \
             ELSE (json_type({table_alias}.metadata_json, ('$.' || ?)) IS NOT NULL) END)"
        ),
    }
}

/// Builds the SQL that renders a metadata field as `->>`-equivalent text.
///
/// Pushes the field name once per placeholder it emits.
fn metadata_text_expr(
    dialect: MemorySqlDialect,
    table_alias: &str,
    field: &str,
    predicate: &mut SqlPredicate,
) -> String {
    match dialect {
        MemorySqlDialect::Postgres => {
            predicate.binds.push(field.to_string());
            format!("(({table_alias}.metadata_json)::jsonb ->> ?)")
        }
        MemorySqlDialect::Sqlite => {
            // Bind order follows textual order: `json_type` first, then `json_extract`.
            predicate.binds.push(field.to_string());
            predicate.binds.push(field.to_string());
            let path = "('$.' || ?)";
            format!(
                "(CASE json_type({table_alias}.metadata_json, {path}) \
                 WHEN 'true' THEN 'true' \
                 WHEN 'false' THEN 'false' \
                 WHEN 'null' THEN NULL \
                 ELSE CAST(json_extract({table_alias}.metadata_json, {path}) AS TEXT) END)"
            )
        }
    }
}

/// Reports an operator the dialect cannot express exactly.
fn unsupported(message: &str) -> NativeSqlStoreError {
    NativeSqlStoreError::MetadataFilterUnsupported {
        message: message.to_string(),
    }
}
