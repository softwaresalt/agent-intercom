use std::path::Path;

use monocoque_agent_rem::diff::{path_safety, validate_workspace_path};

#[test]
fn allows_path_inside_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let candidate = Path::new("src/lib.rs");

    let validated = validate_workspace_path(root, candidate).expect("path valid");

    let canonical_root = root.canonicalize().expect("canonicalize root");
    assert!(validated.starts_with(&canonical_root));
    assert!(validated.ends_with(Path::new("src/lib.rs")));
}

#[test]
fn rejects_traversal() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let candidate = Path::new("../secret.txt");

    let result = validate_workspace_path(root, candidate);

    assert!(result.is_err());
}

#[test]
fn rejects_deep_traversal() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let candidate = Path::new("src/../../secret.txt");

    let result = validate_workspace_path(root, candidate);

    assert!(result.is_err());
}

#[test]
fn allows_relative_subdirectory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let candidate = Path::new("src/utils/helpers.rs");

    let validated = validate_workspace_path(root, candidate).expect("path valid");

    let canonical_root = root.canonicalize().expect("canonicalize root");
    assert!(validated.starts_with(&canonical_root));
    assert!(validated.ends_with("src/utils/helpers.rs"));
}

#[test]
fn allows_dot_segment() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let candidate = Path::new("./src/main.rs");

    let validated = validate_workspace_path(root, candidate).expect("path valid");

    let canonical_root = root.canonicalize().expect("canonicalize root");
    assert!(validated.starts_with(&canonical_root));
}

#[test]
fn rejects_workspace_root_boundary() {
    // A path that enters a subdirectory then traverses past the workspace root.
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let result = validate_workspace_path(root, "subdir/../../escape.txt");

    assert!(result.is_err());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");

    // Create a file outside the workspace.
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "top secret").expect("write secret");

    // Create a symlink inside the workspace pointing outside.
    let link = workspace.path().join("sneaky_link");
    symlink(&secret, &link).expect("symlink");

    let result = path_safety::validate_path(workspace.path(), Path::new("sneaky_link"));

    assert!(result.is_err(), "symlink escape should be rejected");
}

#[test]
fn path_safety_allows_non_existent_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    // File doesn't exist but path is valid.
    let result = path_safety::validate_path(root, "new_dir/new_file.rs");

    assert!(result.is_ok());
}

#[test]
fn path_safety_rejects_invalid_workspace() {
    let result = path_safety::validate_path(Path::new("/nonexistent/workspace"), "file.rs");

    assert!(result.is_err());
}
