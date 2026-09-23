//! Contract tests for the metadata filter expression language.
//!
//! These lock in three things a reviewer cannot check by reading the parser alone: the
//! accepted wire forms, the rejections, and the three-valued evaluation semantics that the
//! SQL translation has to reproduce.

use sdkwork_memory_spi::{
    parse_metadata_filter, MetadataFilterCondition, MetadataFilterError, MetadataFilterExpression,
    MetadataFilterScalar, NumericLiteral, OrderingFilterOperator, ScalarFilterOperator,
    SetFilterOperator, INFIX_OPERATORS, LOGICAL_AND, LOGICAL_NOT, LOGICAL_OR, WILDCARD,
};
use serde_json::{json, Value};

/// Parses a filter that is expected to carry at least one condition.
fn parse_ok(filter: Value) -> MetadataFilterExpression {
    parse_metadata_filter(&filter)
        .expect("filter parses")
        .expect("filter carries a condition")
}

/// Parses a filter that is expected to be rejected.
fn parse_err(filter: Value) -> MetadataFilterError {
    parse_metadata_filter(&filter).expect_err("filter is rejected")
}

/// Evaluates a filter against one stored metadata document.
fn matches(filter: Value, document: Option<&str>) -> bool {
    parse_ok(filter)
        .matches(document)
        .expect("document is valid JSON")
}

/// Builds a scalar equality condition for structural assertions.
fn eq_condition(field: &str, text: &str) -> MetadataFilterExpression {
    MetadataFilterExpression::Condition(MetadataFilterCondition::Scalar {
        field: field.to_string(),
        operator: ScalarFilterOperator::Eq,
        value: MetadataFilterScalar::Text(text.to_string()),
    })
}

// ---------------------------------------------------------------------------------------
// Accepted wire forms
// ---------------------------------------------------------------------------------------

#[test]
fn an_empty_object_is_not_a_filter() {
    assert!(parse_metadata_filter(&json!({}))
        .expect("an empty object parses")
        .is_none());
}

#[test]
fn implicit_equality_compares_the_field_text() {
    assert!(matches(json!({"tenant": "t1"}), Some(r#"{"tenant":"t1"}"#)));
    assert!(!matches(
        json!({"tenant": "t1"}),
        Some(r#"{"tenant":"t2"}"#)
    ));
}

#[test]
fn explicit_equality_and_inequality_agree_with_the_implicit_form() {
    assert_eq!(
        parse_ok(json!({"tenant": "t1"})),
        parse_ok(json!({"tenant": {"eq": "t1"}}))
    );
    assert!(matches(
        json!({"tenant": {"ne": "t1"}}),
        Some(r#"{"tenant":"t2"}"#)
    ));
    assert!(!matches(
        json!({"tenant": {"ne": "t1"}}),
        Some(r#"{"tenant":"t1"}"#)
    ));
}

#[test]
fn numbers_compare_by_their_json_text() {
    // A stored number and a stored numeric string render identically through `->>`, so an
    // implicit-equality filter matches both.
    assert!(matches(json!({"n": 1}), Some(r#"{"n":1}"#)));
    assert!(matches(json!({"n": 1}), Some(r#"{"n":"1"}"#)));
    // The stored text is what is compared, so 1.0 does not render as 1.
    assert!(!matches(json!({"n": 1}), Some(r#"{"n":1.0}"#)));
}

#[test]
fn booleans_compare_by_their_lowercase_json_text() {
    assert!(matches(json!({"flag": true}), Some(r#"{"flag":true}"#)));
    assert!(matches(json!({"flag": true}), Some(r#"{"flag":"true"}"#)));
    assert!(!matches(json!({"flag": true}), Some(r#"{"flag":"True"}"#)));
}

#[test]
fn a_bare_array_is_an_implicit_membership_test() {
    assert_eq!(
        parse_ok(json!({"tag": ["a", "b"]})),
        parse_ok(json!({"tag": {"in": ["a", "b"]}}))
    );
    assert!(matches(json!({"tag": ["a", "b"]}), Some(r#"{"tag":"a"}"#)));
    assert!(!matches(json!({"tag": ["a", "b"]}), Some(r#"{"tag":"c"}"#)));
}

#[test]
fn membership_compares_the_field_text() {
    // A stored number matches the same digits written as a string.
    assert!(matches(
        json!({"tag": {"in": ["1", "2"]}}),
        Some(r#"{"tag":1}"#)
    ));
    assert!(matches(
        json!({"tag": {"nin": ["1"]}}),
        Some(r#"{"tag":2}"#)
    ));
    assert!(!matches(
        json!({"tag": {"nin": ["1"]}}),
        Some(r#"{"tag":1}"#)
    ));
}

#[test]
fn contains_is_case_sensitive_and_icontains_folds_case() {
    assert!(!matches(
        json!({"text": {"contains": "Foo"}}),
        Some(r#"{"text":"a foo b"}"#)
    ));
    assert!(matches(
        json!({"text": {"contains": "Foo"}}),
        Some(r#"{"text":"a Foo b"}"#)
    ));
    assert!(matches(
        json!({"text": {"icontains": "FOO"}}),
        Some(r#"{"text":"a foo b"}"#)
    ));
}

#[test]
fn icontains_folds_non_ascii_case_in_the_oracle() {
    // The oracle defines the contract as Unicode case-insensitive. SQLite cannot express
    // this, so the SQLite dialect refuses a non-ASCII needle instead of matching less.
    assert!(matches(
        json!({"text": {"icontains": "CAFÉ"}}),
        Some(r#"{"text":"le café"}"#)
    ));
}

#[test]
fn ordering_operators_compare_numbers() {
    assert!(matches(
        json!({"score": {"gte": 0.5}}),
        Some(r#"{"score":0.5}"#)
    ));
    assert!(!matches(
        json!({"score": {"gte": 0.5}}),
        Some(r#"{"score":0.4}"#)
    ));
    assert!(matches(json!({"score": {"lt": 2}}), Some(r#"{"score":1}"#)));
    assert!(matches(
        json!({"score": {"lte": 2}}),
        Some(r#"{"score":2}"#)
    ));
    assert!(matches(json!({"score": {"gt": 1}}), Some(r#"{"score":2}"#)));
}

#[test]
fn ordering_does_not_coerce_a_numeric_string() {
    // A documented deviation: the reference lets PostgreSQL cast "9" to numeric. Requiring a
    // JSON number is what keeps PostgreSQL and SQLite in agreement.
    assert!(!matches(
        json!({"score": {"gt": 1}}),
        Some(r#"{"score":"9"}"#)
    ));
}

#[test]
fn ordering_excludes_a_non_numeric_value_instead_of_failing() {
    assert!(!matches(
        json!({"score": {"gt": 1}}),
        Some(r#"{"score":"high"}"#)
    ));
    assert!(!matches(
        json!({"score": {"gt": 1}}),
        Some(r#"{"score":true}"#)
    ));
}

#[test]
fn the_wildcard_form_tests_presence() {
    assert!(matches(
        json!({"owner": WILDCARD}),
        Some(r#"{"owner":"alice"}"#)
    ));
    assert!(!matches(json!({"owner": WILDCARD}), Some("{}")));
}

#[test]
fn presence_holds_even_when_the_value_is_json_null() {
    // `metadata ? 'owner'` is true for a present key regardless of its value.
    assert!(matches(
        json!({"owner": WILDCARD}),
        Some(r#"{"owner":null}"#)
    ));
}

#[test]
fn the_operator_form_of_a_star_is_a_literal_comparison() {
    // Only the bare form is a wildcard; this distinction is easy to lose and would turn a
    // literal comparison into an existence test.
    assert!(matches(
        json!({"owner": {"eq": WILDCARD}}),
        Some(r#"{"owner":"*"}"#)
    ));
    assert!(!matches(
        json!({"owner": {"eq": WILDCARD}}),
        Some(r#"{"owner":"alice"}"#)
    ));
}

#[test]
fn and_is_flattened_into_the_surrounding_conjunction() {
    let flattened = parse_ok(json!({"AND": [{"a": "1"}, {"b": "2"}]}));
    assert_eq!(
        flattened,
        MetadataFilterExpression::All(vec![eq_condition("a", "1"), eq_condition("b", "2")])
    );
    assert_eq!(flattened, parse_ok(json!({"a": "1", "b": "2"})));
    assert!(matches(
        json!({"AND": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"1","b":"2"}"#)
    ));
    assert!(!matches(
        json!({"AND": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"1"}"#)
    ));
}

#[test]
fn or_groups_each_entry_as_a_conjunction() {
    let parsed = parse_ok(json!({"OR": [{"a": "1", "b": "2"}, {"c": "3"}]}));
    assert_eq!(
        parsed,
        MetadataFilterExpression::Any(vec![
            MetadataFilterExpression::All(vec![eq_condition("a", "1"), eq_condition("b", "2")]),
            MetadataFilterExpression::All(vec![eq_condition("c", "3")]),
        ])
    );
    assert!(matches(
        json!({"OR": [{"a": "1", "b": "2"}, {"c": "3"}]}),
        Some(r#"{"a":"1","b":"2"}"#)
    ));
    assert!(matches(
        json!({"OR": [{"a": "1", "b": "2"}, {"c": "3"}]}),
        Some(r#"{"c":"3"}"#)
    ));
    assert!(!matches(
        json!({"OR": [{"a": "1", "b": "2"}, {"c": "3"}]}),
        Some(r#"{"a":"1"}"#)
    ));
}

#[test]
fn not_negates_the_disjunction_of_its_entries() {
    // The reference translates `NOT: [A, B]` as `NOT (A OR B)`, not `NOT A AND NOT B`.
    let parsed = parse_ok(json!({"NOT": [{"a": "1"}, {"b": "2"}]}));
    assert_eq!(
        parsed,
        MetadataFilterExpression::Not(Box::new(MetadataFilterExpression::Any(vec![
            MetadataFilterExpression::All(vec![eq_condition("a", "1")]),
            MetadataFilterExpression::All(vec![eq_condition("b", "2")]),
        ])))
    );
    assert!(matches(
        json!({"NOT": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"2","b":"3"}"#)
    ));
    assert!(!matches(
        json!({"NOT": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"1"}"#)
    ));
    assert!(!matches(
        json!({"NOT": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"b":"2"}"#)
    ));
    // These two pin OR-versus-AND inside the negation. `NOT (A OR B)` admits only the record
    // where neither holds; `NOT (A AND B)` would wrongly admit the record where exactly one
    // holds. Without these, the two readings agree on every other case above.
    assert!(matches(
        json!({"NOT": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"2","b":"3"}"#)
    ));
    assert!(!matches(
        json!({"NOT": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"2","b":"2"}"#)
    ));
}

#[test]
fn referenced_fields_are_sorted_and_deduplicated() {
    assert_eq!(
        parse_ok(json!({"b": "1", "a": "1", "OR": [{"c": "2"}, {"a": "3"}]})).referenced_fields(),
        ["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

// ---------------------------------------------------------------------------------------
// Three-valued evaluation
// ---------------------------------------------------------------------------------------

#[test]
fn inequality_does_not_match_an_absent_field() {
    // `NULL <> 'alice'` is unknown, so SQL discards the row. A two-valued evaluator would
    // wrongly return true here and every filter of this shape would over-match.
    assert!(!matches(json!({"owner": {"ne": "alice"}}), Some("{}")));
    assert!(matches(
        json!({"owner": {"ne": "alice"}}),
        Some(r#"{"owner":"bob"}"#)
    ));
}

#[test]
fn negation_over_an_absent_field_stays_unknown() {
    // `NOT NULL` is NULL, so the row is discarded rather than admitted.
    assert!(!matches(json!({"NOT": [{"a": "1"}]}), Some("{}")));
}

#[test]
fn negation_over_a_present_false_is_true() {
    assert!(matches(json!({"NOT": [{"a": "1"}]}), Some(r#"{"a":"2"}"#)));
}

#[test]
fn an_absent_metadata_value_matches_nothing() {
    assert!(!matches(json!({"a": "1"}), None));
    assert!(!matches(json!({"a": WILDCARD}), None));
    // Unknown dominates a disjunction, so even an OR that could otherwise be satisfied
    // cannot admit the row.
    assert!(!matches(json!({"OR": [{"a": "1"}, {"b": "2"}]}), None));
}

#[test]
fn a_non_object_document_has_no_readable_fields() {
    // Extracting a key from a scalar yields NULL and a presence test yields false, which is
    // exactly how both SQL dialects behave.
    assert!(!matches(json!({"a": "1"}), Some("5")));
    assert!(!matches(json!({"a": WILDCARD}), Some("5")));
    assert!(!matches(json!({"a": "1"}), Some("\"text\"")));
}

#[test]
fn a_present_but_null_field_value_is_unknown_for_comparisons() {
    // `->>` returns SQL NULL for a JSON null, so no comparison against it can be true.
    assert!(!matches(json!({"a": "1"}), Some(r#"{"a":null}"#)));
    assert!(!matches(json!({"a": {"ne": "1"}}), Some(r#"{"a":null}"#)));
}

#[test]
fn a_false_branch_still_narrows_a_conjunction() {
    assert!(!matches(json!({"a": "1", "b": "2"}), Some(r#"{"a":"1"}"#)));
}

#[test]
fn a_true_branch_admits_a_disjunction_with_an_unknown_branch() {
    // Kleene: TRUE OR UNKNOWN is TRUE.
    assert!(matches(
        json!({"OR": [{"a": "1"}, {"b": "2"}]}),
        Some(r#"{"a":"1"}"#)
    ));
}

// ---------------------------------------------------------------------------------------
// Rejections: nothing is silently dropped
// ---------------------------------------------------------------------------------------

#[test]
fn a_non_object_root_is_rejected() {
    assert_eq!(
        parse_err(json!("tenant")),
        MetadataFilterError::RootNotObject { found: "string" }
    );
    assert_eq!(
        parse_err(json!([{"tenant": "t1"}])),
        MetadataFilterError::RootNotObject { found: "array" }
    );
    assert_eq!(
        parse_err(Value::Null),
        MetadataFilterError::RootNotObject { found: "null" }
    );
}

#[test]
fn an_unsupported_operator_is_rejected() {
    let error = parse_err(json!({"a": {"matches": "x"}}));
    let MetadataFilterError::UnsupportedOperator {
        operator,
        supported,
    } = error
    else {
        panic!("expected an unsupported-operator rejection, got {error:?}");
    };
    assert_eq!(operator, "matches");
    for expected in INFIX_OPERATORS {
        assert!(
            supported.contains(expected),
            "the rejection must name {expected} as supported"
        );
    }
}

#[test]
fn a_logical_key_requires_a_list() {
    for logical in [LOGICAL_AND, LOGICAL_OR, LOGICAL_NOT] {
        assert_eq!(
            parse_err(json!({logical: "x"})),
            MetadataFilterError::LogicalNotArray { logical }
        );
    }
}

#[test]
fn an_empty_logical_list_is_rejected() {
    // The reference accepts an empty AND list as a no-op, which silently widens the result
    // set; every logical key refuses it here.
    for logical in [LOGICAL_AND, LOGICAL_OR, LOGICAL_NOT] {
        assert_eq!(
            parse_err(json!({logical: []})),
            MetadataFilterError::LogicalEmpty { logical }
        );
    }
}

#[test]
fn a_logical_entry_must_be_a_non_empty_object() {
    assert_eq!(
        parse_err(json!({"OR": ["x"]})),
        MetadataFilterError::LogicalEntryNotObject {
            logical: LOGICAL_OR,
            found: "string",
        }
    );
    assert_eq!(
        parse_err(json!({"AND": [{}]})),
        MetadataFilterError::LogicalEntryNotObject {
            logical: LOGICAL_AND,
            found: "empty object",
        }
    );
}

#[test]
fn a_blank_field_name_is_rejected() {
    assert_eq!(
        parse_err(json!({"  ": "x"})),
        MetadataFilterError::BlankFieldName
    );
}

#[test]
fn an_empty_condition_object_is_rejected() {
    assert_eq!(
        parse_err(json!({"a": {}})),
        MetadataFilterError::EmptyCondition {
            field: "a".to_string(),
        }
    );
}

#[test]
fn several_operators_on_one_field_are_rejected() {
    assert_eq!(
        parse_err(json!({"a": {"eq": "1", "ne": "2"}})),
        MetadataFilterError::MultipleOperators {
            field: "a".to_string(),
            count: 2,
        }
    );
}

#[test]
fn a_null_literal_is_rejected() {
    assert_eq!(
        parse_err(json!({"a": null})),
        MetadataFilterError::NullLiteral {
            field: "a".to_string(),
        }
    );
    assert_eq!(
        parse_err(json!({"a": {"eq": null}})),
        MetadataFilterError::NullLiteral {
            field: "a".to_string(),
        }
    );
}

#[test]
fn a_list_literal_is_rejected_for_scalar_operators() {
    for operator in ["eq", "ne", "contains", "icontains"] {
        let error = parse_err(json!({"a": {operator: ["x"]}}));
        let MetadataFilterError::InvalidValueShape {
            field,
            operator: name,
            ..
        } = error
        else {
            panic!("expected a value-shape rejection for {operator}");
        };
        assert_eq!(field, "a");
        assert_eq!(name, operator);
    }
}

#[test]
fn an_ordering_bound_must_be_a_number() {
    for operator in ["gt", "gte", "lt", "lte"] {
        assert_eq!(
            parse_err(json!({"a": {operator: "x"}})),
            MetadataFilterError::OrderingValueNotNumber {
                operator: operator.to_string(),
            }
        );
    }
}

#[test]
fn a_set_operator_requires_a_non_empty_list_of_scalars() {
    assert!(matches!(
        parse_err(json!({"a": {"in": "x"}})),
        MetadataFilterError::InvalidValueShape { .. }
    ));
    assert!(matches!(
        parse_err(json!({"a": {"in": []}})),
        MetadataFilterError::InvalidValueShape { .. }
    ));
    assert!(matches!(
        parse_err(json!({"a": {"nin": [{"x": 1}]}})),
        MetadataFilterError::InvalidValueShape { .. }
    ));
    assert!(matches!(
        parse_err(json!({"tag": []})),
        MetadataFilterError::InvalidValueShape { .. }
    ));
}

#[test]
fn a_non_finite_numeric_literal_is_rejected() {
    assert!(NumericLiteral::parse("nan").is_err());
    assert!(NumericLiteral::parse("inf").is_err());
    assert!(NumericLiteral::parse("-inf").is_err());
    // Overflowing a float is non-finite and therefore refused.
    assert!(NumericLiteral::parse("1e400").is_err());
    assert!(NumericLiteral::parse("").is_err());
    assert_eq!(
        NumericLiteral::parse("0.50")
            .expect("valid literal")
            .as_str(),
        "0.50"
    );
}

#[test]
fn stored_metadata_must_be_valid_json() {
    let filter = parse_ok(json!({"a": "1"}));
    let error = filter
        .matches(Some("not json"))
        .expect_err("malformed metadata is refused");
    assert!(matches!(error, MetadataFilterError::MetadataNotJson { .. }));
}

// ---------------------------------------------------------------------------------------
// Nesting
// ---------------------------------------------------------------------------------------

#[test]
fn a_logical_key_is_recognized_inside_a_logical_group() {
    // Regression: a nested logical key used to be read as a field name, so this parsed as a
    // comparison against a metadata key literally named "AND" and then failed with a
    // confusing value-shape error. Nesting must be a group at every depth.
    let nested = parse_ok(json!({"OR": [{"AND": [{"a": "1"}, {"b": "2"}]}, {"c": "3"}]}));
    assert_eq!(nested.referenced_fields(), ["a", "b", "c"]);
    assert_eq!(
        nested,
        parse_ok(json!({"OR": [{"a": "1", "b": "2"}, {"c": "3"}]}))
    );
    assert!(matches(
        json!({"OR": [{"AND": [{"a": "1"}, {"b": "2"}]}, {"c": "3"}]}),
        Some(r#"{"a":"1","b":"2"}"#)
    ));
    assert!(matches(
        json!({"OR": [{"AND": [{"a": "1"}, {"b": "2"}]}, {"c": "3"}]}),
        Some(r#"{"c":"3"}"#)
    ));
    assert!(!matches(
        json!({"OR": [{"AND": [{"a": "1"}, {"b": "2"}]}, {"c": "3"}]}),
        Some(r#"{"a":"1"}"#)
    ));
}

#[test]
fn a_nested_group_is_conjoined_with_its_siblings() {
    let filter = json!({"AND": [{"OR": [{"a": "1"}, {"b": "2"}]}, {"c": "3"}]});
    assert!(matches(filter.clone(), Some(r#"{"a":"1","c":"3"}"#)));
    assert!(matches(filter.clone(), Some(r#"{"b":"2","c":"3"}"#)));
    assert!(!matches(filter.clone(), Some(r#"{"c":"3"}"#)));
    assert!(!matches(filter, Some(r#"{"a":"1"}"#)));
}

#[test]
fn a_nested_not_negates_only_its_own_group() {
    // NOT is scoped to the group it appears in, so the sibling condition still has to hold.
    let filter = json!({"AND": [{"c": "3"}, {"NOT": [{"a": "1"}]}]});
    assert!(matches(filter.clone(), Some(r#"{"a":"2","c":"3"}"#)));
    assert!(!matches(filter.clone(), Some(r#"{"a":"1","c":"3"}"#)));
    assert!(!matches(filter, Some(r#"{"a":"2"}"#)));
}

#[test]
fn an_empty_nested_logical_list_is_rejected() {
    assert_eq!(
        parse_err(json!({"OR": [{"AND": []}]})),
        MetadataFilterError::LogicalEmpty {
            logical: LOGICAL_AND
        }
    );
    assert_eq!(
        parse_err(json!({"OR": [{"AND": [{}]}]})),
        MetadataFilterError::LogicalEntryNotObject {
            logical: LOGICAL_AND,
            found: "empty object",
        }
    );
}

// ---------------------------------------------------------------------------------------
// Structural invariants
// ---------------------------------------------------------------------------------------

#[test]
fn every_advertised_operator_parses_into_exactly_one_family() {
    assert_eq!(INFIX_OPERATORS.len(), 10);
    let mut seen = Vec::new();
    for name in INFIX_OPERATORS {
        assert!(!seen.contains(&name), "operator {name} is advertised twice");
        seen.push(name);
        let families = u8::from(ScalarFilterOperator::parse(name).is_some())
            + u8::from(OrderingFilterOperator::parse(name).is_some())
            + u8::from(SetFilterOperator::parse(name).is_some());
        assert_eq!(
            families, 1,
            "operator {name} belongs to {families} families"
        );
    }
}

#[test]
fn operator_names_round_trip() {
    for operator in [
        ScalarFilterOperator::Eq,
        ScalarFilterOperator::Ne,
        ScalarFilterOperator::Contains,
        ScalarFilterOperator::IContains,
    ] {
        assert_eq!(
            ScalarFilterOperator::parse(operator.as_str()),
            Some(operator)
        );
    }
    for operator in [
        OrderingFilterOperator::Gt,
        OrderingFilterOperator::Gte,
        OrderingFilterOperator::Lt,
        OrderingFilterOperator::Lte,
    ] {
        assert_eq!(
            OrderingFilterOperator::parse(operator.as_str()),
            Some(operator)
        );
    }
    for operator in [SetFilterOperator::In, SetFilterOperator::Nin] {
        assert_eq!(SetFilterOperator::parse(operator.as_str()), Some(operator));
    }
}

#[test]
fn the_wildcard_is_not_an_advertised_infix_operator() {
    // Advertising it would make `{"a":{"*":1}}` look supported while the parser rejects it,
    // and would blur the literal-vs-presence distinction the parser depends on.
    assert!(!INFIX_OPERATORS.contains(&WILDCARD));
    assert!(ScalarFilterOperator::parse(WILDCARD).is_none());
    assert!(OrderingFilterOperator::parse(WILDCARD).is_none());
    assert!(SetFilterOperator::parse(WILDCARD).is_none());
}

#[test]
fn every_condition_reports_its_operator_and_field() {
    let scalar = parse_ok(json!({"a": {"icontains": "x"}}));
    let MetadataFilterExpression::Condition(condition) = scalar else {
        panic!("a single field parses into a single condition");
    };
    assert_eq!(condition.operator_name(), "icontains");
    assert_eq!(condition.field(), "a");

    let exists = parse_ok(json!({"b": WILDCARD}));
    let MetadataFilterExpression::Condition(condition) = exists else {
        panic!("a wildcard parses into a single condition");
    };
    assert_eq!(condition.operator_name(), WILDCARD);
    assert_eq!(condition.field(), "b");
}
