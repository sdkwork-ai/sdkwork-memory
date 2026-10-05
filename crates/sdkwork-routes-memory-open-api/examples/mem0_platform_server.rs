//! Serves the Memory open-api surface (mem0 platform wire included) on a socket.
//!
//! The point is to be able to point a **real** mem0 client — the official
//! `mem0ai` package, `curl`, a language binding — at the compatibility surface
//! without standing up PostgreSQL or the full runtime assembly. The store is the
//! in-memory fixture, so this is a wire-level harness, not a deployment.
//!
//! ```text
//! cargo run -p sdkwork-routes-memory-open-api --example mem0_platform_server
//! # prints: MEM0_SERVER_LISTENING http://127.0.0.1:<port>
//! ```
//!
//! Environment:
//!
//! * `MEM0_SERVER_ADDR` — the bind address. Defaults to `127.0.0.1:0`, i.e. an
//!   ephemeral port, which is what makes concurrent runs safe. The chosen port
//!   is always printed, because a caller that asked for `:0` has no other way to
//!   learn it.
//! * `MEM0_SERVER_API_KEY` — the API key callers must present as
//!   `Authorization: Token <key>`. Defaults to the dev fixture for user `2001`.
//!
//! The credential is a *claim-form* key: the framework's default API-key lookup
//! reads the caller's tenant/user/app out of the key itself, so it must carry
//! `api_key_id`, `tenant_id`, `user_id`, and `app_id`.

use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::web_auth::memory_dev_api_key;
use sdkwork_routes_memory_open_api::{build_router_with_open_api, wrap_router_with_web_framework};
use sdkwork_web_core::DefaultWebRequestContextResolver;

#[tokio::main]
async fn main() {
    // The web runtime hardening gate refuses to serve an undeclared environment,
    // so this harness declares the honest one: a development fixture.
    std::env::set_var("SDKWORK_ENV", "dev");
    std::env::set_var("SDKWORK_MEMORY_ENVIRONMENT", "development");
    std::env::set_var("SDKWORK_IAM_ALLOW_DEV_AUTH_FALLBACK", "true");

    let addr = std::env::var("MEM0_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:0".to_owned());
    let api_key = std::env::var("MEM0_SERVER_API_KEY")
        .unwrap_or_else(|_| memory_dev_api_key("2001", "mem0-compat-key"));

    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let service = OpenMemoryService::new(store);
    // Print the mem0 compatibility space for the fixture actor, so load
    // drivers and curls can aim space-scoped SDKWork calls at the same space
    // the mem0 wire writes into.
    {
        let context = sdkwork_memory_contract::MemoryOpenApiRequestContext::for_open_surface(
            "mem0-platform-server",
            100_001,
            Some(2001),
        );
        let space_id = service
            .mem0_space_id(&context)
            .await
            .unwrap_or_else(|error| panic!("mem0 space resolution failed: {error:?}"));
        println!("MEM0_COMPAT_SPACE_ID {space_id}");
    }
    let app = wrap_router_with_web_framework(
        DefaultWebRequestContextResolver::default(),
        build_router_with_open_api(service),
    );

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|error| panic!("could not bind `{addr}`: {error}"));
    let bound = listener
        .local_addr()
        .expect("the bound address is known");

    println!("MEM0_SERVER_LISTENING http://{bound}");
    println!("MEM0_SERVER_API_KEY {api_key}");
    println!(
        "try: curl -s -H 'Authorization: Token {api_key}' http://{bound}/v1/ping/"
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|error| panic!("serve failed: {error}"));
}
