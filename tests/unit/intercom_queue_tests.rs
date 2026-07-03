//! Unit tests for the `.intercom` numbered queue repository.

use agent_intercom::persistence::intercom_queue_repo::IntercomQueueRepo;
use tempfile::TempDir;

fn build_repo() -> (TempDir, IntercomQueueRepo) {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let repo = IntercomQueueRepo::new(&temp_dir.path().join(".intercom"));
    (temp_dir, repo)
}

#[test]
fn add_returns_numbered_item() {
    let (_temp_dir, repo) = build_repo();

    let item = repo.add("alpha").expect("add item");

    assert_eq!(item.number, 1);
    assert_eq!(item.text, "alpha");
}

#[test]
fn list_returns_items_in_insertion_order() {
    let (_temp_dir, repo) = build_repo();

    repo.add("alpha").expect("add alpha");
    repo.add("beta").expect("add beta");

    let items = repo.list().expect("list items");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].number, 1);
    assert_eq!(items[0].text, "alpha");
    assert_eq!(items[1].number, 2);
    assert_eq!(items[1].text, "beta");
}

#[test]
fn numbers_are_stable_after_remove() {
    let (_temp_dir, repo) = build_repo();

    repo.add("a").expect("add a");
    repo.add("b").expect("add b");
    repo.add("c").expect("add c");
    repo.remove(2).expect("remove b");
    repo.add("d").expect("add d");

    let items = repo.list().expect("list items");

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].number, 1);
    assert_eq!(items[0].text, "a");
    assert_eq!(items[1].number, 3);
    assert_eq!(items[1].text, "c");
    assert_eq!(items[2].number, 4);
    assert_eq!(items[2].text, "d");
}

#[test]
fn replace_updates_item_text() {
    let (_temp_dir, repo) = build_repo();

    let item = repo.add("alpha").expect("add alpha");
    let updated = repo.replace(item.number, "beta").expect("replace alpha");
    let items = repo.list().expect("list items");

    assert_eq!(updated.number, 1);
    assert_eq!(updated.text, "beta");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "beta");
}

#[test]
fn replace_nonexistent_returns_error() {
    let (_temp_dir, repo) = build_repo();

    let result = repo.replace(99, "missing");

    assert!(result.is_err());
}

#[test]
fn transfer_removes_item_from_queue() {
    let (_temp_dir, repo) = build_repo();

    repo.add("a").expect("add a");
    repo.add("b").expect("add b");
    repo.add("c").expect("add c");
    let removed = repo.remove(2).expect("remove b");
    let items = repo.list().expect("list items");

    assert_eq!(removed.number, 2);
    assert_eq!(removed.text, "b");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].number, 1);
    assert_eq!(items[1].number, 3);
}

#[test]
fn remove_nonexistent_returns_error() {
    let (_temp_dir, repo) = build_repo();

    let result = repo.remove(99);

    assert!(result.is_err());
}
