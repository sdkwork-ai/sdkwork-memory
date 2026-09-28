//! Standard guard for the app-api-only embedding factory.
//!
//! The gate lives in `tests/` on purpose: it reads `../src/bootstrap.rs` as
//! text, so putting the needles in the guarded file itself would let every
//! positive assertion satisfy itself (and every negative one trip on its own
//! literal). See the workspace convention for `include_str!` source gates.

/// An embedding host (the Cloud Router unified runtime) owns the process and
/// already serves the open-api and backend-api Memory mounts. The app-api-only
/// factory must therefore publish exactly one surface: composing the whole
/// module here would re-publish those routes and collide with the host's own
/// capability mounts.
#[test]
fn app_api_contribution_factory_publishes_only_the_app_surface() {
    let source = include_str!("../src/bootstrap.rs");

    let start = source
        .find("pub async fn assemble_app_api_contribution_from_env()")
        .expect("app-api contribution factory must exist");
    let body = &source[start..];

    assert!(
        body.contains("build_app_router_with_product("),
        "the app-api factory must mount the app surface"
    );
    assert!(
        body.contains("wrap_app_router("),
        "the app-api factory must keep the app surface's own Web Framework layer"
    );
    assert!(
        body.contains("sdkwork_routes_memory_app_api::app_route_manifest()"),
        "the app-api factory must publish the app surface's own route manifest"
    );

    // Needles are assembled at compile time so the contiguous identifier never
    // appears in this file, and each is checked against the slice that starts
    // at the factory, not the whole module.
    let forbidden_whole_module_composition = ["assemble_api_router", "("].concat();
    assert!(
        !body.contains(&forbidden_whole_module_composition),
        "the app-api factory must not compose the whole module (standalone gateway's job)"
    );
    let forbidden_open_surface = ["sdkwork_routes_memory_open_api", "::"].concat();
    assert!(
        !body.contains(&forbidden_open_surface),
        "the app-api factory must not publish the open-api surface"
    );
    let forbidden_backend_surface = ["sdkwork_routes_memory_backend_api", "::"].concat();
    assert!(
        !body.contains(&forbidden_backend_surface),
        "the app-api factory must not publish the backend-api surface"
    );
}
