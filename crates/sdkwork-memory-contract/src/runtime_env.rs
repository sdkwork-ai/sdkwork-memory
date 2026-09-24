//! Memory runtime environment helpers shared by routers and the API server.

use sdkwork_utils_rust::parse_bool;

static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Serializes env-mutating tests across crates that share process environment state.
#[doc(hidden)]
pub fn env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Panic-safe process-environment override for tests.
///
/// A hand-written save/restore pair only restores the environment when the test body returns
/// normally. When an assertion fails, the unwind skips the restore and the leaked `SDKWORK_*`
/// values stay visible to every other test running on another thread of the same test binary —
/// which turns one genuine failure into a cascade of unrelated-looking failures elsewhere.
/// `MemoryEnvScope` restores on drop, so a failure stays local to the test that caused it.
///
/// Pair it with [`env_test_lock`]: the lock serializes access, the scope guarantees release.
#[doc(hidden)]
#[must_use = "the previous values are restored only while the scope is alive"]
pub struct MemoryEnvScope {
    previous: Vec<(String, Option<std::ffi::OsString>)>,
}

impl MemoryEnvScope {
    /// Apply every `(key, value)` override, remembering the previous value of each key.
    ///
    /// `None` removes the variable for the duration of the scope.
    pub fn new(vars: &[(&str, Option<&str>)]) -> Self {
        let previous = vars
            .iter()
            .map(|(key, _)| ((*key).to_string(), std::env::var_os(key)))
            .collect();
        for (key, value) in vars {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        Self { previous }
    }
}

impl Drop for MemoryEnvScope {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

/// Canonical `SDKWORK_MEMORY_ENVIRONMENT` / `SDKWORK_MEMORY_CONFIG_PROFILE` value.
pub fn memory_environment_name() -> String {
    std::env::var("SDKWORK_MEMORY_ENVIRONMENT")
        .or_else(|_| std::env::var("SDKWORK_MEMORY_CONFIG_PROFILE"))
        .unwrap_or_else(|_| "development".to_string())
        .to_ascii_lowercase()
}

/// Environment names accepted by [`require_explicit_memory_environment`].
const RECOGNIZED_ENVIRONMENTS: &[&str] = &[
    "development", "dev", "production", "prod", "staging", "stage", "test",
];

/// Refuses to run a server process with an unset or unrecognized
/// `SDKWORK_MEMORY_ENVIRONMENT`.
///
/// The security posture (authorization policy, tenant isolation, Redis-backed
/// rate limiting, request deadlines, CORS hardening) switches on the resolved
/// environment, and the resolver used to silently default an unset variable to
/// `development` — so one missed env var turned an intended production replica
/// into an unhardened dev server with no warning. Server hosts must declare the
/// environment explicitly; the `test-runner` escape hatch matches the
/// server-role engine admission rule in the repository bootstrap.
pub fn require_explicit_memory_environment() -> Result<(), String> {
    if std::env::var("SDKWORK_MEMORY_RUNTIME_TARGET").as_deref() == Ok("test-runner") {
        return Ok(());
    }
    let raw = std::env::var("SDKWORK_MEMORY_ENVIRONMENT")
        .or_else(|_| std::env::var("SDKWORK_MEMORY_CONFIG_PROFILE"))
        .map_err(|_| {
            "SDKWORK_MEMORY_ENVIRONMENT must be set explicitly for a Memory server process \
             (development|staging|production); refusing to start with the historical implicit \
             'development' default because it disables authorization, tenant isolation, \
             Redis-backed rate limiting, and request deadlines"
                .to_string()
        })?;
    let normalized = raw.trim().to_ascii_lowercase();
    if RECOGNIZED_ENVIRONMENTS.contains(&normalized.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "SDKWORK_MEMORY_ENVIRONMENT value '{raw}' is not recognized \
             (expected one of: development, staging, production)"
        ))
    }
}

/// Returns true for production-like lifecycle profiles that must never use dev inline auth.
pub fn memory_is_production_like_environment() -> bool {
    matches!(
        memory_environment_name().as_str(),
        "production" | "prod" | "staging" | "stage" | "test"
    )
}

fn env_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|value| parse_bool(&value))
        .unwrap_or(false)
}

/// `SDKWORK_MEMORY_DEV_AUTH_BYPASS` enables inline dev credentials only in non-production profiles.
pub fn memory_dev_auth_bypass_enabled() -> bool {
    env_truthy("SDKWORK_MEMORY_DEV_AUTH_BYPASS")
}

/// Whether HTTP surfaces may use `DefaultWebRequestContextResolver` with inline dev credentials.
pub fn memory_use_dev_inline_auth_resolver() -> bool {
    !memory_is_production_like_environment() && memory_dev_auth_bypass_enabled()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_env(key: &str, value: Option<&str>, test: impl FnOnce()) {
        let _scope = MemoryEnvScope::new(&[(key, value)]);
        test();
    }

    #[test]
    fn env_scope_restores_the_previous_value_on_unwind() {
        let _guard = env_test_lock();
        const KEY: &str = "SDKWORK_MEMORY_CONTRACT_ENV_SCOPE_PROBE";
        std::env::remove_var(KEY);

        let panicked = std::panic::catch_unwind(|| {
            let _scope = MemoryEnvScope::new(&[(KEY, Some("production"))]);
            assert_eq!(std::env::var(KEY).as_deref(), Ok("production"));
            panic!("simulated assertion failure inside the scope");
        });

        assert!(panicked.is_err(), "the probe closure must have panicked");
        assert_eq!(
            std::env::var_os(KEY),
            None,
            "the scope must restore the environment while unwinding, or a single failing \
             assertion leaks SDKWORK_* values into every sibling test in this binary"
        );
    }

    #[test]
    fn explicit_environment_gate_accepts_recognized_names() {
        let _guard = env_test_lock();
        for value in ["development", "dev", "staging", "stage", "production", "prod", "test"] {
            with_env("SDKWORK_MEMORY_RUNTIME_TARGET", None, || {
                with_env("SDKWORK_MEMORY_ENVIRONMENT", Some(value), || {
                    assert!(
                        require_explicit_memory_environment().is_ok(),
                        "{value} must be accepted"
                    );
                });
            });
        }
    }

    #[test]
    fn explicit_environment_gate_rejects_unset_and_unknown_values() {
        let _guard = env_test_lock();
        with_env("SDKWORK_MEMORY_RUNTIME_TARGET", None, || {
            with_env("SDKWORK_MEMORY_ENVIRONMENT", None, || {
                with_env("SDKWORK_MEMORY_CONFIG_PROFILE", None, || {
                    assert!(require_explicit_memory_environment().is_err());
                });
            });
            with_env("SDKWORK_MEMORY_ENVIRONMENT", Some("prodction"), || {
                assert!(require_explicit_memory_environment().is_err());
            });
        });
    }

    #[test]
    fn test_runner_escape_hatch_allows_implicit_environment() {
        let _guard = env_test_lock();
        with_env("SDKWORK_MEMORY_RUNTIME_TARGET", Some("test-runner"), || {
            with_env("SDKWORK_MEMORY_ENVIRONMENT", None, || {
                assert!(require_explicit_memory_environment().is_ok());
            });
        });
    }

    #[test]
    fn production_never_uses_dev_inline_auth_even_when_bypass_flag_is_set() {
        let _guard = env_test_lock();
        with_env("SDKWORK_MEMORY_ENVIRONMENT", Some("production"), || {
            with_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", Some("true"), || {
                assert!(!memory_use_dev_inline_auth_resolver());
            });
        });
    }

    #[test]
    fn development_requires_explicit_dev_auth_bypass_flag() {
        let _guard = env_test_lock();
        with_env("SDKWORK_MEMORY_ENVIRONMENT", Some("development"), || {
            with_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", None, || {
                assert!(!memory_use_dev_inline_auth_resolver());
            });
            with_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", Some("true"), || {
                assert!(memory_use_dev_inline_auth_resolver());
            });
        });
    }

    #[test]
    fn dev_auth_bypass_rejects_non_boolean_env_values() {
        let _guard = env_test_lock();
        with_env("SDKWORK_MEMORY_ENVIRONMENT", Some("development"), || {
            with_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", Some("maybe"), || {
                assert!(!memory_dev_auth_bypass_enabled());
            });
            with_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", Some("false"), || {
                assert!(!memory_dev_auth_bypass_enabled());
            });
        });
    }
}
