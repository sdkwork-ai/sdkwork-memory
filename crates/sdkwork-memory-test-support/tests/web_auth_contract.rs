use sdkwork_memory_test_support::web_auth::{
    legacy_inline_dual_tokens, memory_access_token, memory_auth_token_bearer, memory_dev_api_key,
};
use sdkwork_web_core::{
    DefaultWebRequestContextResolver, WebFrameworkErrorKind, WebRequestContextResolver,
};

#[tokio::test]
async fn memory_jwt_dual_tokens_resolve_through_default_web_resolver() {
    let resolver = DefaultWebRequestContextResolver::default();
    let principal = resolver
        .resolve_dual_token(
            memory_auth_token_bearer("2001")
                .strip_prefix("Bearer ")
                .unwrap(),
            &memory_access_token("2001"),
        )
        .await
        .expect("jwt dual tokens must resolve");

    assert_eq!("100001", principal.tenant_id());
    assert_eq!("2001", principal.user_id());
}

#[tokio::test]
async fn legacy_inline_dual_tokens_are_rejected() {
    let resolver = DefaultWebRequestContextResolver::default();
    let (auth, access) = legacy_inline_dual_tokens("2001");
    let error = resolver
        .resolve_dual_token(auth.strip_prefix("Bearer ").unwrap(), &access)
        .await
        .expect_err("semicolon claim-string tokens must be rejected");

    assert_eq!(WebFrameworkErrorKind::InvalidCredentials, error.kind);
    assert!(error.message.contains("JWT"));
}

#[tokio::test]
async fn memory_dev_api_key_resolves_through_default_web_resolver() {
    let resolver = DefaultWebRequestContextResolver::default();
    let principal = resolver
        .resolve_api_key(&memory_dev_api_key("2001", "dev-key"))
        .await
        .expect("inline api key claims remain valid for open-api dev tests");

    assert_eq!("100001", principal.tenant_id());
    assert_eq!("2001", principal.user_id());
}
