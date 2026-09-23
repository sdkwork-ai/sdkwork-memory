//! Metadata filter expression language.
//!
//! Callers may attach a metadata filter to a retrieval request. This module owns the
//! *meaning* of that filter: it parses the wire form, validates it, and can evaluate it
//! against one metadata document. It deliberately does **not** know how to translate a
//! filter into SQL — that belongs to whichever store owns the query (see
//! `sdkwork-memory-plugin-native-sql`), because the translation is dialect specific.
//!
//! # Why this lives in the SPI crate
//!
//! The filter has to appear on [`crate::SearchMemoryCandidatesQuery`], so the type must be
//! visible to every party on both sides of that port: the service that builds the query and
//! the plugin that answers it. `sdkwork-memory-spi` is the only crate both sides already
//! depend on, so placing the type there adds no dependency edge. The in-memory evaluator
//! lives here too so that store implementations have an executable definition of the
//! contract to diff their SQL against.
//!
//! # Wire form
//!
//! The three API contracts declare `filters` as a free-form JSON object
//! (`{"type":"object","additionalProperties":true}`), so this module parses the JSON-object
//! form directly. It is the same shape the reference implementation accepts:
//!
//! ```text
//! {"tenant":"t1"}                          implicit equality
//! {"tenant":{"eq":"t1"}}                   explicit operator
//! {"score":{"gte":0.5}}                    ordering
//! {"tag":{"in":["a","b"]}}                 set membership
//! {"tag":["a","b"]}                        bare array is implicit `in`
//! {"owner":"*"}                            field exists
//! {"AND":[{"a":1},{"b":2}]}                logical combination
//! {"OR":[{"a":1},{"b":2}]}
//! {"NOT":[{"a":1},{"b":2}]}
//! ```
//!
//! # Fail closed
//!
//! This module never discards part of a filter to keep going. An operator it does not
//! implement, a logical key whose value is not a list, an empty `OR`/`NOT` list, a value
//! whose shape does not fit its operator — each of those is a [`MetadataFilterError`], not a
//! silently dropped condition. The reference implementation exhibits several silent
//! degradations of exactly this kind (dropping a wildcard on one backend, dropping every
//! field except three on another), and reproducing them would make a caller believe a filter
//! was applied when it was not.
//!
//! # Three-valued evaluation
//!
//! SQL compares against `NULL` with three-valued logic, and the reference implementation
//! pushes filters straight into SQL, so the observable semantics are three-valued: a
//! comparison against an absent field is *unknown*, not *false*. The consequence is not
//! cosmetic — `{"owner":{"ne":"alice"}}` does **not** match a record that has no `owner` key,
//! because `NULL <> 'alice'` is unknown. [`MetadataFilterExpression::matches`] reproduces
//! that faithfully, which is what makes it usable as an oracle for the SQL translation.
//!
//! # Documented deviations
//!
//! Each of these is a deliberate divergence from the reference implementation, chosen so
//! that a filter can never quietly do less than the caller asked:
//!
//! 1. **Unknown operator is an error.** The reference also rejects these, so this is
//!    alignment rather than deviation, but it is the rule the rest of this list extends.
//! 2. **Empty `AND`/`OR`/`NOT` list is an error.** The reference accepts an empty `AND` list
//!    as a no-op, which silently widens the result set. An empty condition list is always a
//!    caller bug here.
//! 3. **A group that carries no condition is an error.** The reference skips such a group
//!    and, if every group is empty, drops the whole clause — again silently widening.
//! 4. **`null` as a literal is an error.** A JSON `null` reaches the stored side as SQL
//!    `NULL`, so equality with it can never be true; the reference accepts the literal and
//!    compares it against the string `"None"`. Both never match, but only one says so.
//! 5. **A list literal is rejected for `eq`/`ne`/`contains`/`icontains`.** The reference
//!    stringifies the list into a single operand, producing a comparison that cannot match.
//!    Use `in`/`nin`.
//! 6. **Ordering operators require a JSON number on the stored side.** The reference lets
//!    PostgreSQL cast `"3.5"` to `numeric`, so a string-valued field can satisfy an ordering
//!    filter there. Requiring a number keeps PostgreSQL and SQLite in exact agreement, which
//!    matters more than the permissiveness.
//! 7. **A non-numeric stored value excludes the row instead of failing the query.** The
//!    reference lets `(payload->>'k')::numeric` raise, which aborts the entire request when a
//!    single record holds a non-numeric value for the filtered key. Here it evaluates to
//!    *unknown*, consistent with every other absent-value case.
//! 8. **`icontains` is case-insensitive over Unicode.** On SQLite, whose `lower()` folds
//!    ASCII only, a needle containing non-ASCII characters is refused with
//!    [`MetadataFilterError::DialectCannotExpressOperator`] rather than silently matching
//!    less than PostgreSQL would.

use serde_json::{Map, Value};
use thiserror::Error;

/// Logical conjunction key.
pub const LOGICAL_AND: &str = "AND";
/// Logical disjunction key.
pub const LOGICAL_OR: &str = "OR";
/// Logical negation key.
pub const LOGICAL_NOT: &str = "NOT";
/// Bare wildcard value asserting that a field exists.
pub const WILDCARD: &str = "*";

/// Every operator accepted inside an operator object such as `{"eq": "t1"}`.
///
/// The bare wildcard form `{"field": "*"}` is handled separately and is intentionally not
/// listed here: the reference accepts existence only in that bare form, and reading
/// `{"field":{"eq":"*"}}` as a wildcard would silently turn a literal comparison into an
/// existence test.
pub const INFIX_OPERATORS: [&str; 10] = [
    "eq",
    "ne",
    "gt",
    "gte",
    "lt",
    "lte",
    "in",
    "nin",
    "contains",
    "icontains",
];

/// A finite decimal literal.
///
/// Retained as text rather than a bare `f64` so the value handed to a query keeps the exact
/// digits the caller wrote, and so the expression tree stays `Eq` — a public `f64` field
/// would force `PartialEq` only, because `NaN != NaN`.
#[derive(Debug, Clone)]
pub struct NumericLiteral {
    text: String,
    value: f64,
}

impl NumericLiteral {
    /// Validates a decimal literal.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataFilterError::NonNumericOrderingValue`] when `text` is not a finite
    /// decimal number. `NaN`, `inf` and `-inf` are rejected because they cannot be ordered
    /// meaningfully and cannot appear in JSON.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_spi::NumericLiteral;
    ///
    /// assert_eq!(NumericLiteral::parse("0.50")?.as_str(), "0.50");
    /// assert!(NumericLiteral::parse("nan").is_err());
    /// # Ok::<(), sdkwork_memory_spi::MetadataFilterError>(())
    /// ```
    pub fn parse(text: &str) -> Result<Self, MetadataFilterError> {
        let trimmed = text.trim();
        let value =
            trimmed
                .parse::<f64>()
                .map_err(|_| MetadataFilterError::NonNumericOrderingValue {
                    value: text.to_string(),
                })?;
        if !value.is_finite() {
            return Err(MetadataFilterError::NonNumericOrderingValue {
                value: text.to_string(),
            });
        }
        Ok(Self {
            text: trimmed.to_string(),
            value,
        })
    }

    /// Returns the literal text exactly as validated.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Returns the numeric value, for in-memory comparison.
    #[must_use]
    pub fn as_f64(&self) -> f64 {
        self.value
    }
}

impl PartialEq for NumericLiteral {
    /// Compares the literal text, which is the canonical validated form.
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for NumericLiteral {}

impl std::fmt::Display for NumericLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

/// Operator for a textual comparison against one scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarFilterOperator {
    /// The field's text equals the operand.
    Eq,
    /// The field's text differs from the operand.
    Ne,
    /// The field's text contains the operand.
    Contains,
    /// The field's text contains the operand, ignoring case.
    IContains,
}

impl ScalarFilterOperator {
    /// Returns the wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Contains => "contains",
            Self::IContains => "icontains",
        }
    }

    /// Parses a wire name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "eq" => Some(Self::Eq),
            "ne" => Some(Self::Ne),
            "contains" => Some(Self::Contains),
            "icontains" => Some(Self::IContains),
            _ => None,
        }
    }
}

/// Operator for a numeric comparison against one bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderingFilterOperator {
    /// The field is greater than the bound.
    Gt,
    /// The field is greater than or equal to the bound.
    Gte,
    /// The field is less than the bound.
    Lt,
    /// The field is less than or equal to the bound.
    Lte,
}

impl OrderingFilterOperator {
    /// Returns the wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
        }
    }

    /// Parses a wire name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "gt" => Some(Self::Gt),
            "gte" => Some(Self::Gte),
            "lt" => Some(Self::Lt),
            "lte" => Some(Self::Lte),
            _ => None,
        }
    }
}

/// Operator for membership in a set of scalars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetFilterOperator {
    /// The field's text equals one of the operands.
    In,
    /// The field's text equals none of the operands.
    Nin,
}

impl SetFilterOperator {
    /// Returns the wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Nin => "nin",
        }
    }

    /// Parses a wire name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "in" => Some(Self::In),
            "nin" => Some(Self::Nin),
            _ => None,
        }
    }
}

/// A scalar operand inside a filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataFilterScalar {
    /// A string literal.
    Text(String),
    /// A numeric literal.
    Number(NumericLiteral),
    /// A boolean literal.
    Bool(bool),
}

impl MetadataFilterScalar {
    /// Returns the text the stored value is compared against.
    ///
    /// This mirrors extracting a field with PostgreSQL's `->>` operator, which renders every
    /// scalar as text: `true` becomes `"true"`, and the number `1` becomes `"1"`.
    #[must_use]
    pub fn as_comparison_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Number(number) => number.as_str().to_string(),
            Self::Bool(true) => "true".to_string(),
            Self::Bool(false) => "false".to_string(),
        }
    }
}

/// A leaf predicate over one metadata field.
///
/// The four variants make an operator/value mismatch unrepresentable, so evaluation has no
/// "wrong shape" branch and no fallback arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataFilterCondition {
    /// Compares the field's text to `value`.
    Scalar {
        /// Metadata field name.
        field: String,
        /// The comparison.
        operator: ScalarFilterOperator,
        /// Right-hand side.
        value: MetadataFilterScalar,
    },
    /// Compares the field numerically.
    Ordering {
        /// Metadata field name.
        field: String,
        /// The comparison.
        operator: OrderingFilterOperator,
        /// Right-hand side.
        bound: NumericLiteral,
    },
    /// Set membership.
    Set {
        /// Metadata field name.
        field: String,
        /// The comparison.
        operator: SetFilterOperator,
        /// Accepted values.
        values: Vec<MetadataFilterScalar>,
    },
    /// Asserts the field is present, however it is valued. Produced by the bare `"*"` form.
    Exists {
        /// Metadata field name.
        field: String,
    },
}

impl MetadataFilterCondition {
    /// Returns the operator name as it appeared on the wire.
    #[must_use]
    pub fn operator_name(&self) -> &'static str {
        match self {
            Self::Scalar { operator, .. } => operator.as_str(),
            Self::Ordering { operator, .. } => operator.as_str(),
            Self::Set { operator, .. } => operator.as_str(),
            Self::Exists { .. } => WILDCARD,
        }
    }

    /// Returns the metadata field this condition constrains.
    #[must_use]
    pub fn field(&self) -> &str {
        match self {
            Self::Scalar { field, .. }
            | Self::Ordering { field, .. }
            | Self::Set { field, .. }
            | Self::Exists { field } => field,
        }
    }
}

/// A parsed metadata filter.
///
/// Build one with [`parse_metadata_filter`] rather than by hand: the parser is what enforces
/// the operator set and the value shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataFilterExpression {
    /// A single leaf predicate.
    Condition(MetadataFilterCondition),
    /// Every child must hold. The implicit conjunction of the top-level object.
    All(Vec<MetadataFilterExpression>),
    /// At least one child must hold.
    Any(Vec<MetadataFilterExpression>),
    /// The child must not hold.
    Not(Box<MetadataFilterExpression>),
}

impl MetadataFilterExpression {
    /// Returns every metadata field the expression reads, sorted and deduplicated.
    ///
    /// Store implementations use this to report which fields a filter touches, and tests use
    /// it to assert that no branch was dropped during translation.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    /// use sdkwork_memory_spi::{parse_metadata_filter, MetadataFilterError};
    ///
    /// let Some(filter) = parse_metadata_filter(&json!({
    ///     "a": "1",
    ///     "OR": [{"b": "2"}, {"c": "3"}]
    /// }))? else {
    ///     return Ok(());
    /// };
    /// assert_eq!(filter.referenced_fields(), ["a", "b", "c"]);
    /// # Ok::<(), MetadataFilterError>(())
    /// ```
    #[must_use]
    pub fn referenced_fields(&self) -> Vec<String> {
        let mut fields = Vec::new();
        self.collect_fields(&mut fields);
        fields.sort();
        fields.dedup();
        fields
    }

    /// Evaluates the filter against one stored metadata document.
    ///
    /// `metadata_json` is the raw text of the record's metadata column; `None` means the
    /// record has no metadata value at all, which makes every comparison *unknown*.
    ///
    /// Returns `Ok(true)` only when the expression evaluates to true. An *unknown* result
    /// returns `Ok(false)`, because SQL discards a row whose predicate is unknown.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataFilterError::MetadataNotJson`] when `metadata_json` is present but
    /// is not valid JSON. That is what the SQL dialects do as well — both refuse to extract
    /// from malformed JSON rather than treating it as empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    /// use sdkwork_memory_spi::{parse_metadata_filter, MetadataFilterError};
    ///
    /// let Some(filter) = parse_metadata_filter(&json!({"tenant": {"eq": "t1"}}))? else {
    ///     return Ok(());
    /// };
    /// assert!(filter.matches(Some(r#"{"tenant":"t1"}"#))?);
    /// assert!(!filter.matches(Some(r#"{"tenant":"t2"}"#))?);
    /// // An absent field is unknown, which discards the row.
    /// assert!(!filter.matches(Some("{}"))?);
    /// assert!(!filter.matches(None)?);
    /// # Ok::<(), MetadataFilterError>(())
    /// ```
    pub fn matches(&self, metadata_json: Option<&str>) -> Result<bool, MetadataFilterError> {
        let Some(document) = metadata_json else {
            // The column is NULL, so every leaf is unknown and the whole tree is unknown.
            return Ok(false);
        };
        let parsed = serde_json::from_str::<Value>(document).map_err(|error| {
            MetadataFilterError::MetadataNotJson {
                detail: error.to_string(),
            }
        })?;
        // A present but non-object document has no readable field, which is exactly how the
        // SQL dialects behave: extracting a key from a scalar yields NULL, and testing a key
        // for existence yields false.
        let empty = Map::new();
        let lookup = parsed.as_object().unwrap_or(&empty);
        Ok(self.evaluate(Some(lookup)) == Tristate::True)
    }

    /// Evaluates under SQL three-valued logic.
    ///
    /// `document` is `None` when the metadata column itself is `NULL`, in which case every
    /// leaf is unknown and the whole expression is therefore unknown.
    fn evaluate(&self, document: Option<&Map<String, Value>>) -> Tristate {
        match self {
            Self::Condition(condition) => condition.evaluate(document),
            Self::All(children) => children
                .iter()
                .map(|child| child.evaluate(document))
                .fold(Tristate::True, Tristate::and),
            Self::Any(children) => children
                .iter()
                .map(|child| child.evaluate(document))
                .fold(Tristate::False, Tristate::or),
            Self::Not(child) => child.evaluate(document).negate(),
        }
    }

    /// Walks the tree collecting every constrained field.
    fn collect_fields(&self, fields: &mut Vec<String>) {
        match self {
            Self::Condition(condition) => fields.push(condition.field().to_string()),
            Self::All(children) | Self::Any(children) => {
                for child in children {
                    child.collect_fields(fields);
                }
            }
            Self::Not(child) => child.collect_fields(fields),
        }
    }
}

/// SQL three-valued logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tristate {
    /// The predicate holds.
    True,
    /// The predicate does not hold.
    False,
    /// The predicate compares against `NULL`; the row is discarded and negation keeps it so.
    Unknown,
}

impl Tristate {
    /// Lifts a Rust boolean into two-valued SQL logic.
    fn from_bool(value: bool) -> Self {
        if value {
            Self::True
        } else {
            Self::False
        }
    }

    /// Kleene conjunction.
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }

    /// Kleene disjunction.
    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }

    /// Kleene negation: `NOT NULL` stays `NULL`.
    fn negate(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}

impl MetadataFilterCondition {
    /// Evaluates this leaf against one metadata document.
    fn evaluate(&self, document: Option<&Map<String, Value>>) -> Tristate {
        let Some(document) = document else {
            return Tristate::Unknown;
        };
        let Some(stored) = document.get(self.field()) else {
            // An absent key makes `metadata ? 'key'` false but every comparison unknown.
            return if matches!(self, Self::Exists { .. }) {
                Tristate::False
            } else {
                Tristate::Unknown
            };
        };
        match self {
            Self::Exists { .. } => Tristate::True,
            Self::Scalar {
                operator, value, ..
            } => {
                let Some(stored_text) = json_scalar_text(stored) else {
                    return Tristate::Unknown;
                };
                let expected = value.as_comparison_text();
                Tristate::from_bool(match operator {
                    ScalarFilterOperator::Eq => stored_text == expected,
                    ScalarFilterOperator::Ne => stored_text != expected,
                    ScalarFilterOperator::Contains => stored_text.contains(expected.as_str()),
                    ScalarFilterOperator::IContains => stored_text
                        .to_lowercase()
                        .contains(expected.to_lowercase().as_str()),
                })
            }
            Self::Ordering {
                operator, bound, ..
            } => {
                let Some(stored_number) = json_number_value(stored) else {
                    return Tristate::Unknown;
                };
                let expected = bound.as_f64();
                Tristate::from_bool(match operator {
                    OrderingFilterOperator::Gt => stored_number > expected,
                    OrderingFilterOperator::Gte => stored_number >= expected,
                    OrderingFilterOperator::Lt => stored_number < expected,
                    OrderingFilterOperator::Lte => stored_number <= expected,
                })
            }
            Self::Set {
                operator, values, ..
            } => {
                let Some(stored_text) = json_scalar_text(stored) else {
                    return Tristate::Unknown;
                };
                let present = values
                    .iter()
                    .any(|candidate| candidate.as_comparison_text() == stored_text);
                Tristate::from_bool(match operator {
                    SetFilterOperator::In => present,
                    SetFilterOperator::Nin => !present,
                })
            }
        }
    }
}

/// Renders a stored JSON scalar the way `->>` does, or `None` for JSON `null`.
///
/// PostgreSQL's `->>` returns SQL `NULL` for a JSON `null` value, so a JSON `null` on the
/// stored side can never compare equal to anything. Containers are rendered as compact JSON
/// text; the reference compares them against PostgreSQL's spaced rendering, so a container
/// valued field is the one corner where exact alignment is not claimed.
fn json_scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(true) => Some("true".to_string()),
        Value::Bool(false) => Some("false".to_string()),
        Value::Number(number) => Some(number.to_string()),
        container @ (Value::Array(_) | Value::Object(_)) => Some(container.to_string()),
    }
}

/// Reads a stored JSON number for ordering comparison.
///
/// Only a JSON number qualifies. A numeric string is not coerced, which keeps PostgreSQL
/// (`jsonb_typeof(...) = 'number'`) and SQLite (`json_type(...) IN ('integer','real')`) in
/// exact agreement with this evaluator.
fn json_number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        _ => None,
    }
}

/// A rejected metadata filter.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetadataFilterError {
    /// The filter root was not a JSON object.
    #[error("metadata filter must be a JSON object, found {found}")]
    RootNotObject {
        /// The JSON type that was found instead.
        found: &'static str,
    },
    /// An operator outside the supported set was used.
    #[error("unsupported metadata filter operator {operator:?}; supported: {supported}")]
    UnsupportedOperator {
        /// The rejected operator.
        operator: String,
        /// Comma separated list of accepted operators.
        supported: String,
    },
    /// A logical key was given something other than a list.
    #[error("metadata filter {logical} requires a list of conditions")]
    LogicalNotArray {
        /// The logical key, one of `AND`, `OR`, `NOT`.
        logical: &'static str,
    },
    /// A logical key was given an empty list.
    #[error("metadata filter {logical} requires at least one condition")]
    LogicalEmpty {
        /// The logical key, one of `AND`, `OR`, `NOT`.
        logical: &'static str,
    },
    /// A condition inside a logical group was not an object.
    #[error("metadata filter {logical} entries must be objects, found {found}")]
    LogicalEntryNotObject {
        /// The logical key, one of `AND`, `OR`, `NOT`.
        logical: &'static str,
        /// The JSON type that was found instead.
        found: &'static str,
    },
    /// A condition object carried no operator.
    #[error("metadata filter condition for field {field:?} carries no operator")]
    EmptyCondition {
        /// The field whose condition was empty.
        field: String,
    },
    /// A filter object produced no evaluable condition.
    #[error("metadata filter carries no evaluable condition")]
    NoCondition,
    /// The field name was blank.
    #[error("metadata filter field name must not be blank")]
    BlankFieldName,
    /// The operator object held more than one operator.
    #[error("metadata filter for field {field:?} must use exactly one operator, found {count}")]
    MultipleOperators {
        /// The field with more than one operator.
        field: String,
        /// How many operators were supplied.
        count: usize,
    },
    /// A value did not fit its operator.
    #[error("metadata filter operator {operator:?} for field {field:?} {reason}")]
    InvalidValueShape {
        /// The field being filtered.
        field: String,
        /// The operator that received the value.
        operator: String,
        /// Why the value was rejected.
        reason: String,
    },
    /// An ordering operator received a non-numeric bound.
    #[error("metadata filter ordering bound {value:?} is not a finite decimal number")]
    NonNumericOrderingValue {
        /// The rejected bound.
        value: String,
    },
    /// An ordering operator was combined with a value shape it cannot compare.
    #[error("metadata filter ordering operator {operator:?} requires a numeric bound")]
    OrderingValueNotNumber {
        /// The operator, one of `gt`, `gte`, `lt`, `lte`.
        operator: String,
    },
    /// A literal `null` was used where it could never match.
    #[error(
        "metadata filter field {field:?} compares against null, which can never match; \
         use {{\"ne\": ...}} to exclude records or the bare \"*\" form to test presence"
    )]
    NullLiteral {
        /// The field compared against null.
        field: String,
    },
    /// The stored metadata was not valid JSON.
    #[error("stored metadata is not valid JSON: {detail}")]
    MetadataNotJson {
        /// The parser diagnostic.
        detail: String,
    },
    /// The active SQL dialect cannot express the operator with the required semantics.
    #[error("metadata filter operator {operator:?} cannot be expressed by {dialect}: {reason}")]
    DialectCannotExpressOperator {
        /// The SQL dialect that cannot express the operator.
        dialect: &'static str,
        /// The operator involved.
        operator: &'static str,
        /// Why the dialect falls short.
        reason: String,
    },
}

/// Parses a metadata filter from its wire form.
///
/// Returns `Ok(None)` when the filter object carries no condition at all, so a caller can
/// distinguish "no filter was requested" from "a filter was requested".
///
/// # Errors
///
/// Returns a [`MetadataFilterError`] for any filter that cannot be honored exactly. The
/// module documentation lists each rejection and why it is a rejection rather than a
/// tolerated no-op.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use sdkwork_memory_spi::{parse_metadata_filter, MetadataFilterError};
///
/// let filter = parse_metadata_filter(&json!({"score": {"gte": 0.5}}))?;
/// let Some(filter) = filter else {
///     return Ok(());
/// };
/// assert_eq!(filter.referenced_fields(), ["score"]);
///
/// // An empty object is not a filter.
/// assert!(parse_metadata_filter(&json!({}))?.is_none());
///
/// // An unimplemented operator is an error, never a silently ignored condition.
/// assert!(matches!(
///     parse_metadata_filter(&json!({"a": {"matches": "x"}})),
///     Err(MetadataFilterError::UnsupportedOperator { .. })
/// ));
/// # Ok::<(), MetadataFilterError>(())
/// ```
pub fn parse_metadata_filter(
    filter: &Value,
) -> Result<Option<MetadataFilterExpression>, MetadataFilterError> {
    let Some(object) = filter.as_object() else {
        return Err(MetadataFilterError::RootNotObject {
            found: json_type_name(filter),
        });
    };
    if object.is_empty() {
        return Ok(None);
    }

    let mut children = parse_object_conditions(object)?;

    // The object was non-empty and every branch contributes at least one child, so this is a
    // defensive check rather than a reachable one; it refuses to turn a parse gap into a
    // vacuously-true conjunction.
    if children.is_empty() {
        return Err(MetadataFilterError::NoCondition);
    }
    Ok(Some(if children.len() == 1 {
        children.pop().ok_or(MetadataFilterError::NoCondition)?
    } else {
        MetadataFilterExpression::All(children)
    }))
}

/// Parses an object into the conditions it carries.
///
/// Logical keys are recognized at **every** depth, not only at the top level. A nested
/// `AND`/`OR`/`NOT` is therefore a logical group wherever it appears; treating it as a
/// field name instead would turn `{"OR":[{"AND":[…]}]}` into a comparison against a metadata
/// key literally named `AND`.
fn parse_object_conditions(
    object: &Map<String, Value>,
) -> Result<Vec<MetadataFilterExpression>, MetadataFilterError> {
    let mut children: Vec<MetadataFilterExpression> = Vec::new();
    for (key, value) in object {
        match key.as_str() {
            LOGICAL_AND => {
                // The reference flattens `AND` into the surrounding conjunction. Flattening
                // is exactly equivalent because each entry is itself a conjunction, and the
                // surrounding context is already a conjunction.
                for entry in logical_entries(LOGICAL_AND, value)? {
                    children.extend(logical_group(LOGICAL_AND, entry)?);
                }
            }
            LOGICAL_OR => {
                let groups = logical_entries(LOGICAL_OR, value)?
                    .into_iter()
                    .map(|entry| {
                        logical_group(LOGICAL_OR, entry).map(MetadataFilterExpression::All)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                children.push(MetadataFilterExpression::Any(groups));
            }
            LOGICAL_NOT => {
                // The reference translates `NOT: [A, B]` into `NOT (A OR B)`: each entry is a
                // conjunction and the entries are disjoined before negation.
                let groups = logical_entries(LOGICAL_NOT, value)?
                    .into_iter()
                    .map(|entry| {
                        logical_group(LOGICAL_NOT, entry).map(MetadataFilterExpression::All)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                children.push(MetadataFilterExpression::Not(Box::new(
                    MetadataFilterExpression::Any(groups),
                )));
            }
            field => children.extend(parse_field_condition(field, value)?),
        }
    }
    Ok(children)
}

/// Validates a logical key's value and returns its entries.
fn logical_entries<'a>(
    logical: &'static str,
    value: &'a Value,
) -> Result<Vec<&'a Map<String, Value>>, MetadataFilterError> {
    let Some(entries) = value.as_array() else {
        return Err(MetadataFilterError::LogicalNotArray { logical });
    };
    if entries.is_empty() {
        return Err(MetadataFilterError::LogicalEmpty { logical });
    }
    entries
        .iter()
        .map(|entry| {
            entry
                .as_object()
                .ok_or(MetadataFilterError::LogicalEntryNotObject {
                    logical,
                    found: json_type_name(entry),
                })
        })
        .collect()
}

/// Parses one entry of a logical group.
///
/// An entry that carries no condition is refused rather than contributing nothing: the
/// reference skips such a group and, when every group is empty, drops the whole clause,
/// which silently widens the result set instead of surfacing the caller's bug.
fn logical_group(
    logical: &'static str,
    group: &Map<String, Value>,
) -> Result<Vec<MetadataFilterExpression>, MetadataFilterError> {
    if group.is_empty() {
        return Err(MetadataFilterError::LogicalEntryNotObject {
            logical,
            found: "empty object",
        });
    }
    parse_object_conditions(group)
}

/// Parses the condition(s) attached to a single field.
fn parse_field_condition(
    field: &str,
    value: &Value,
) -> Result<Vec<MetadataFilterExpression>, MetadataFilterError> {
    if field.trim().is_empty() {
        return Err(MetadataFilterError::BlankFieldName);
    }

    // Bare wildcard: presence test. Checked before the operator-object branch precisely
    // because the reference only grants wildcard meaning to the bare form.
    if value.as_str() == Some(WILDCARD) {
        return Ok(vec![MetadataFilterExpression::Condition(
            MetadataFilterCondition::Exists {
                field: field.to_string(),
            },
        )]);
    }

    if let Some(operators) = value.as_object() {
        if operators.is_empty() {
            return Err(MetadataFilterError::EmptyCondition {
                field: field.to_string(),
            });
        }
        if operators.len() > 1 {
            return Err(MetadataFilterError::MultipleOperators {
                field: field.to_string(),
                count: operators.len(),
            });
        }
        let Some((operator, operand)) = operators.iter().next() else {
            return Err(MetadataFilterError::EmptyCondition {
                field: field.to_string(),
            });
        };
        return Ok(vec![MetadataFilterExpression::Condition(
            parse_operator_condition(field, operator, operand)?,
        )]);
    }

    // A bare array is implicit `in`, mirroring the reference.
    if let Some(items) = value.as_array() {
        return Ok(vec![MetadataFilterExpression::Condition(
            parse_set_condition(field, SetFilterOperator::In, items)?,
        )]);
    }

    Ok(vec![MetadataFilterExpression::Condition(
        MetadataFilterCondition::Scalar {
            field: field.to_string(),
            operator: ScalarFilterOperator::Eq,
            value: parse_scalar(field, "eq", value)?,
        },
    )])
}

/// Builds a condition from an explicit operator.
fn parse_operator_condition(
    field: &str,
    operator: &str,
    operand: &Value,
) -> Result<MetadataFilterCondition, MetadataFilterError> {
    if let Some(scalar_operator) = ScalarFilterOperator::parse(operator) {
        if operand.is_array() {
            return Err(MetadataFilterError::InvalidValueShape {
                field: field.to_string(),
                operator: operator.to_string(),
                reason: "expects a single scalar; use \"in\" or \"nin\" for lists".to_string(),
            });
        }
        return Ok(MetadataFilterCondition::Scalar {
            field: field.to_string(),
            operator: scalar_operator,
            value: parse_scalar(field, operator, operand)?,
        });
    }
    if let Some(ordering_operator) = OrderingFilterOperator::parse(operator) {
        let Some(number) = operand.as_f64() else {
            return Err(MetadataFilterError::OrderingValueNotNumber {
                operator: operator.to_string(),
            });
        };
        return Ok(MetadataFilterCondition::Ordering {
            field: field.to_string(),
            operator: ordering_operator,
            bound: NumericLiteral::parse(&number.to_string())?,
        });
    }
    if let Some(set_operator) = SetFilterOperator::parse(operator) {
        let Some(items) = operand.as_array() else {
            return Err(MetadataFilterError::InvalidValueShape {
                field: field.to_string(),
                operator: operator.to_string(),
                reason: "requires a list value".to_string(),
            });
        };
        return parse_set_condition(field, set_operator, items);
    }
    Err(MetadataFilterError::UnsupportedOperator {
        operator: operator.to_string(),
        supported: INFIX_OPERATORS.join(", "),
    })
}

/// Builds a set-membership condition from list items.
fn parse_set_condition(
    field: &str,
    operator: SetFilterOperator,
    items: &[Value],
) -> Result<MetadataFilterCondition, MetadataFilterError> {
    if items.is_empty() {
        return Err(MetadataFilterError::InvalidValueShape {
            field: field.to_string(),
            operator: operator.as_str().to_string(),
            reason: "requires at least one value".to_string(),
        });
    }
    let values = items
        .iter()
        .map(|item| {
            if item.is_array() || item.is_object() {
                return Err(MetadataFilterError::InvalidValueShape {
                    field: field.to_string(),
                    operator: operator.as_str().to_string(),
                    reason: format!("element of type {} is not a scalar", json_type_name(item)),
                });
            }
            parse_scalar(field, operator.as_str(), item)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MetadataFilterCondition::Set {
        field: field.to_string(),
        operator,
        values,
    })
}

/// Parses a scalar operand, rejecting `null` and containers.
fn parse_scalar(
    field: &str,
    operator: &str,
    value: &Value,
) -> Result<MetadataFilterScalar, MetadataFilterError> {
    match value {
        Value::String(text) => Ok(MetadataFilterScalar::Text(text.clone())),
        Value::Bool(flag) => Ok(MetadataFilterScalar::Bool(*flag)),
        Value::Number(number) => Ok(MetadataFilterScalar::Number(NumericLiteral::parse(
            &number.to_string(),
        )?)),
        Value::Null => Err(MetadataFilterError::NullLiteral {
            field: field.to_string(),
        }),
        Value::Array(_) | Value::Object(_) => Err(MetadataFilterError::InvalidValueShape {
            field: field.to_string(),
            operator: operator.to_string(),
            reason: format!("value of type {} is not a scalar", json_type_name(value)),
        }),
    }
}

/// Names a JSON value's type for diagnostics.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
