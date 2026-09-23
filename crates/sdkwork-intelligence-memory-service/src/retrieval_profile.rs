use serde_json::Value;

use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceResult};
use sdkwork_memory_retrieval::MemoryRetrievalStrategy;
use sdkwork_memory_retrieval::RetrievalFusionPolicy;

// `entity` ranks memories whose provenance edges tie them to entities that
// match the query; it runs on in-repo graph data, so unlike `vector` it is
// provider-free. `vector` stays out of the vocabulary until an embedding
// provider can actually supply scores (parity matrix §20.4).
const SUPPORTED_RETRIEVERS: &[&str] = &["keyword", "dictionary", "sql", "time", "event", "entity"];

pub fn validate_retrieval_limits(
    top_k: i32,
    context_budget_tokens: i32,
) -> MemoryServiceResult<()> {
    if !(1..=crate::platform::MAX_RETRIEVAL_TOP_K).contains(&top_k) {
        return Err(MemoryServiceError::validation(format!(
            "topK must be between 1 and {}",
            crate::platform::MAX_RETRIEVAL_TOP_K
        )));
    }
    if context_budget_tokens < 1 {
        return Err(MemoryServiceError::validation(
            "contextBudgetTokens must be at least 1",
        ));
    }
    Ok(())
}

pub fn validate_retrieval_retrievers(retrievers: &Value) -> MemoryServiceResult<()> {
    let Some(object) = retrievers.as_object() else {
        return Err(MemoryServiceError::validation(
            "retrievers must be a JSON object mapping retriever names to weight config",
        ));
    };
    if object.is_empty() {
        return Err(MemoryServiceError::validation(
            "retrievers must not be empty",
        ));
    }
    for (key, config) in object {
        if !SUPPORTED_RETRIEVERS.contains(&key.as_str()) {
            return Err(MemoryServiceError::validation(format!(
                "unsupported retriever '{key}': supported values are {}",
                SUPPORTED_RETRIEVERS.join(", ")
            )));
        }
        config
            .as_object()
            .and_then(|config| config.get("weight"))
            .and_then(Value::as_f64)
            .filter(|weight| weight.is_finite() && *weight > 0.0 && *weight <= 10.0)
            .ok_or_else(|| {
                MemoryServiceError::validation(format!(
                    "retriever '{key}' weight must be a finite number greater than 0 and at most 10"
                ))
            })?;
    }
    Ok(())
}

pub fn validate_retrieval_strategy(strategy: &str, retrievers: &Value) -> MemoryServiceResult<()> {
    match strategy.trim().to_ascii_lowercase().as_str() {
        "deterministic" | "custom_weighted_rrf" => Ok(()),
        "hybrid" => {
            if retrievers.as_object().map_or(0, serde_json::Map::len) < 2 {
                return Err(MemoryServiceError::validation(
                    "hybrid retrieval strategy must enable at least two retrievers",
                ));
            }
            Ok(())
        }
        builtin
        @ ("balanced" | "search_first" | "search-first" | "event_aware" | "event-aware") => {
            let expected = MemoryRetrievalStrategy::parse(builtin)
                .map_err(MemoryServiceError::validation)?
                .retriever_profile();
            if retrievers != &expected {
                return Err(MemoryServiceError::validation(format!(
                    "built-in retrieval strategy {builtin} must use its canonical retriever weights"
                )));
            }
            Ok(())
        }
        other => Err(MemoryServiceError::validation(format!(
            "unsupported retrieval strategy '{other}'"
        ))),
    }
}

pub fn resolve_retrieval_fusion_policy(
    fusion_policy: Option<&Value>,
) -> MemoryServiceResult<RetrievalFusionPolicy> {
    let Some(fusion_policy) = fusion_policy else {
        return Ok(RetrievalFusionPolicy::default());
    };
    let Some(object) = fusion_policy.as_object() else {
        return Err(MemoryServiceError::validation(
            "fusionPolicy must be a JSON object",
        ));
    };
    for key in object.keys() {
        if !matches!(key.as_str(), "algorithm" | "mode" | "rankConstant") {
            return Err(MemoryServiceError::validation(format!(
                "unsupported fusionPolicy field '{key}'"
            )));
        }
    }
    let algorithm = object
        .get("algorithm")
        .and_then(Value::as_str)
        .or_else(|| object.get("mode").and_then(Value::as_str))
        .unwrap_or("weighted_rrf");
    let algorithm = if algorithm == "rrf" {
        "weighted_rrf"
    } else {
        algorithm
    };
    if algorithm != "weighted_rrf" {
        return Err(MemoryServiceError::validation(format!(
            "unsupported fusionPolicy algorithm '{algorithm}': supported value is weighted_rrf"
        )));
    }
    let rank_constant = object
        .get("rankConstant")
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && (1.0..=1000.0).contains(value))
                .ok_or_else(|| {
                    MemoryServiceError::validation(
                        "fusionPolicy.rankConstant must be between 1 and 1000",
                    )
                })
        })
        .transpose()?
        .unwrap_or_else(|| RetrievalFusionPolicy::default().rank_constant);
    Ok(RetrievalFusionPolicy { rank_constant })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unsupported_vector_retriever() {
        let err = validate_retrieval_retrievers(&json!({ "vector": { "weight": 1.0 } }))
            .expect_err("vector retriever must be rejected");
        assert!(format!("{err:?}").contains("unsupported retriever 'vector'"));
    }

    #[test]
    fn accepts_supported_retriever_mix() {
        validate_retrieval_retrievers(&json!({
            "keyword": { "weight": 0.6 },
            "dictionary": { "weight": 0.4 }
        }))
        .expect("supported retrievers must pass validation");
    }

    #[test]
    fn validates_named_strategy_semantics_instead_of_ignoring_the_label() {
        let balanced = MemoryRetrievalStrategy::Balanced.retriever_profile();
        validate_retrieval_strategy("balanced", &balanced).unwrap();
        validate_retrieval_strategy(
            "hybrid",
            &json!({
                "keyword": { "weight": 0.6 },
                "dictionary": { "weight": 0.4 }
            }),
        )
        .unwrap();
        assert!(
            validate_retrieval_strategy("hybrid", &json!({ "keyword": { "weight": 1.0 } }))
                .is_err()
        );
        assert!(validate_retrieval_strategy(
            "search_first",
            &MemoryRetrievalStrategy::Balanced.retriever_profile()
        )
        .is_err());
        assert!(validate_retrieval_strategy("magic", &balanced).is_err());
    }

    #[test]
    fn rejects_missing_zero_or_excessive_weights() {
        for retrievers in [
            json!({ "keyword": {} }),
            json!({ "keyword": { "weight": 0.0 } }),
            json!({ "keyword": { "weight": 10.1 } }),
        ] {
            assert!(validate_retrieval_retrievers(&retrievers).is_err());
        }
    }

    #[test]
    fn resolves_weighted_rrf_policy_and_rejects_silent_misconfiguration() {
        let policy = resolve_retrieval_fusion_policy(Some(&json!({
            "algorithm": "weighted_rrf",
            "rankConstant": 20
        })))
        .expect("weighted RRF must be supported");
        assert_eq!(policy.rank_constant, 20.0);
        assert_eq!(
            resolve_retrieval_fusion_policy(Some(&json!({ "mode": "rrf" })))
                .unwrap()
                .rank_constant,
            RetrievalFusionPolicy::default().rank_constant
        );

        assert!(resolve_retrieval_fusion_policy(Some(&json!({
            "algorithm": "score_sum"
        })))
        .is_err());
        assert!(resolve_retrieval_fusion_policy(Some(&json!({
            "algorithm": "weighted_rrf",
            "unknown": true
        })))
        .is_err());
    }

    #[test]
    fn rejects_invalid_retrieval_limits() {
        assert!(validate_retrieval_limits(1, 1).is_ok());
        assert!(validate_retrieval_limits(0, 1).is_err());
        assert!(validate_retrieval_limits(101, 1).is_err());
        assert!(validate_retrieval_limits(1, 0).is_err());
    }
}
