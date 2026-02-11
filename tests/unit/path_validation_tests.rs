use std::path::Path;

use monocoque_agent_rem::diff::validate_workspace_path;

#[test]
fn allows_path_inside_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let candidate = Path::new("src/lib.rs");

    let validated = validate_workspace_path(root, candidate).expect("path valid");

    assert!(validated.starts_with(root));
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
