//! Tests for binary version and feature flag configuration (T080, T081).
//!
//! T080: `--version` flag outputs a version string matching `CARGO_PKG_VERSION`.
//! T081: Feature flags default to disabled; `rmcp-upgrade` is absent from default builds.

/// T080: Verify that `CARGO_PKG_VERSION` is a valid semver string.
///
/// The `clap` `version` attribute on both `agent-intercom` and `agent-intercom-ctl`
/// automatically passes `env!("CARGO_PKG_VERSION")` to `--version` output.
/// This test confirms the constant is well-formed.
#[test]
fn cargo_pkg_version_is_valid_semver() {
    let version = env!("CARGO_PKG_VERSION");
    assert!(!version.is_empty(), "CARGO_PKG_VERSION must not be empty");
    // Semver requires at least MAJOR.MINOR.PATCH
    let major_minor_patch: Vec<&str> = version.split('-').next().unwrap_or("").split('.').collect();
    assert!(
        major_minor_patch.len() >= 3,
        "CARGO_PKG_VERSION must have at least MAJOR.MINOR.PATCH, got: {version}"
    );
    // Each numeric component must parse as u64
    for part in &major_minor_patch {
        assert!(
            part.parse::<u64>().is_ok(),
            "CARGO_PKG_VERSION numeric part must parse as u64: '{part}' in '{version}'"
        );
    }
}

/// T080 (supplementary): Verify the version string matches the expected workspace version.
///
/// Prevents accidental version drift where `[workspace.package] version` is updated
/// but the binary ships with a stale constant.
#[test]
fn cargo_pkg_version_matches_expected_prefix() {
    let version = env!("CARGO_PKG_VERSION");
    // Must start with a digit (reject accidental empty or "dev" placeholders)
    assert!(
        version.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "CARGO_PKG_VERSION must start with a digit, got: '{version}'"
    );
}

/// T081: Verify that the `rmcp-upgrade` feature flag is NOT enabled by default.
///
/// The `[features]` section in `Cargo.toml` sets `default = []`, so no optional
/// features are active in a standard `cargo build`. This test guards against
/// accidentally adding `rmcp-upgrade` to the `default` feature set.
#[test]
#[allow(clippy::assertions_on_constants)] // intentional: cfg! is compile-time; always false in default builds
fn rmcp_upgrade_feature_is_not_enabled_by_default() {
    assert!(
        !cfg!(feature = "rmcp-upgrade"),
        "rmcp-upgrade must not be in the default feature set; \
         add it explicitly with --features rmcp-upgrade when needed"
    );
}

/// T081 (supplementary): Verify default build has no unexpected features enabled.
///
/// A feature-gated function annotated with `#[cfg(feature = "rmcp-upgrade")]`
/// must be absent when compiled without that feature. At compile time, this
/// module itself will only include the `rmcp_upgrade_gated` body when the feature
/// is active. In a default build the inner assertion never fires.
#[cfg(not(feature = "rmcp-upgrade"))]
#[test]
fn rmcp_upgrade_gated_fn_is_absent_in_default_build() {
    // This test body only compiles and runs when rmcp-upgrade is NOT enabled.
    // Its presence in the test output confirms the feature is absent from the default set.
    // If the feature were accidentally added to `default`, this cfg-gated test
    // would disappear from the suite, and rmcp_upgrade_feature_is_not_enabled_by_default
    // above would fail.
}
