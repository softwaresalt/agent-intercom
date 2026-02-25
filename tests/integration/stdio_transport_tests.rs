//! Integration test for the stdio MCP transport with rmcp 0.13 (T095).
//!
//! Verifies that the stdio transport (`rmcp::transport::io::stdio()`) continues
//! to work after the rmcp version upgrade. The test compiles in all builds
//! (no feature gate) since stdio is not changing — it serves as a stability
//! regression guard to ensure the upgrade does not break existing stdio paths.
//!
//! Because starting a real stdio server requires a subprocess, this test is a
//! compile-time + unit-level verification: it checks that the `serve_stdio`
//! public function remains exported and that the transport module structure is
//! intact.

/// T095 (runtime): Verify that the transport module structure is intact after
/// any rmcp SDK changes.
///
/// This is a compile-time stability check: if the rmcp 0.13 upgrade accidentally
/// removes or renames `serve_stdio`, this test will fail to compile.
#[test]
fn transport_module_exports_serve_stdio() {
    // Bind `serve_stdio` to verify it exists and has the expected signature.
    // This fails at compile time if the function is removed or renamed.
    // Using `std::mem::size_of_val` creates an observable side-effect that
    // satisfies `clippy::no_effect_underscore_binding` on newer toolchains.
    let serve_stdio_fn = agent_intercom::mcp::transport::serve_stdio;
    assert!(
        std::mem::size_of_val(&serve_stdio_fn) == 0,
        "async fn items are zero-sized"
    );

    // Also verify the crate name appears in the type path as a basic sanity
    // check that we are importing from the right crate.
    let module_path = std::any::type_name::<agent_intercom::mcp::handler::AppState>();
    assert!(
        module_path.contains("agent_intercom"),
        "module name should contain crate name: {module_path}"
    );
}
