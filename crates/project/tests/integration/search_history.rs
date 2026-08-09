use project::search_history::{QueryInsertionBehavior, SearchHistory, SearchHistoryCursor};

#[test]
fn test_add() {
    const MAX_HISTORY_LEN: usize = 20;
    let mut search_history = SearchHistory::new(
        Some(MAX_HISTORY_LEN),
        QueryInsertionBehavior::ReplacePreviousIfContains,
    );
    let mut cursor = SearchHistoryCursor::default();

    assert_eq!(
        search_history.current(&cursor),
        None,
        "默认搜索历史不应设置当前选中项"
    );

    search_history.add(&mut cursor, "rust".to_string());
    assert_eq!(
        search_history.current(&cursor),
        Some("rust"),
        "新添加的项应被选中"
    );

    // check if duplicates are not added
    search_history.add(&mut cursor, "rust".to_string());
    assert_eq!(search_history.len(), 1, "不应添加重复项");
    assert_eq!(search_history.current(&cursor), Some("rust"));

    // check if new string containing the previous string replaces it
    search_history.add(&mut cursor, "rustlang".to_string());
    assert_eq!(
        search_history.len(),
        1,
        "如果新字符串包含前一项,则应替换前一项"
    );
    assert_eq!(search_history.current(&cursor), Some("rustlang"));

    // add item when it equals to current item if it's not the last one
    search_history.add(&mut cursor, "php".to_string());
    search_history.previous(&mut cursor, "");
    assert_eq!(search_history.current(&cursor), Some("rustlang"));
    search_history.add(&mut cursor, "rustlang".to_string());
    assert_eq!(search_history.len(), 3, "应添加该项");
    assert_eq!(search_history.current(&cursor), Some("rustlang"));

    // push enough items to test SEARCH_HISTORY_LIMIT
    for i in 0..MAX_HISTORY_LEN * 2 {
        search_history.add(&mut cursor, format!("item{i}"));
    }
    assert!(search_history.len() <= MAX_HISTORY_LEN);
}

#[test]
fn test_next_and_previous() {
    let mut search_history = SearchHistory::new(None, QueryInsertionBehavior::AlwaysInsert);
    let mut cursor = SearchHistoryCursor::default();

    assert_eq!(
        search_history.next(&mut cursor),
        None,
        "默认搜索历史不应有下一项"
    );

    search_history.add(&mut cursor, "Rust".to_string());
    assert_eq!(search_history.next(&mut cursor), None);
    search_history.add(&mut cursor, "JavaScript".to_string());
    assert_eq!(search_history.next(&mut cursor), None);
    search_history.add(&mut cursor, "TypeScript".to_string());
    assert_eq!(search_history.next(&mut cursor), None);

    assert_eq!(search_history.current(&cursor), Some("TypeScript"));

    assert_eq!(search_history.previous(&mut cursor, ""), Some("JavaScript"));
    assert_eq!(search_history.current(&cursor), Some("JavaScript"));

    assert_eq!(search_history.previous(&mut cursor, ""), Some("Rust"));
    assert_eq!(search_history.current(&cursor), Some("Rust"));

    assert_eq!(search_history.previous(&mut cursor, ""), None);
    assert_eq!(search_history.current(&cursor), Some("Rust"));

    assert_eq!(search_history.next(&mut cursor), Some("JavaScript"));
    assert_eq!(search_history.current(&cursor), Some("JavaScript"));

    assert_eq!(search_history.next(&mut cursor), Some("TypeScript"));
    assert_eq!(search_history.current(&cursor), Some("TypeScript"));

    assert_eq!(search_history.next(&mut cursor), None);
    assert_eq!(search_history.current(&cursor), Some("TypeScript"));
}

#[test]
fn test_reset_selection() {
    let mut search_history = SearchHistory::new(None, QueryInsertionBehavior::AlwaysInsert);
    let mut cursor = SearchHistoryCursor::default();

    search_history.add(&mut cursor, "Rust".to_string());
    search_history.add(&mut cursor, "JavaScript".to_string());
    search_history.add(&mut cursor, "TypeScript".to_string());

    assert_eq!(search_history.current(&cursor), Some("TypeScript"));
    cursor.reset();
    assert_eq!(search_history.current(&cursor), None);
    assert_eq!(
        search_history.previous(&mut cursor, ""),
        Some("TypeScript"),
        "重置后查询上一项时应从末尾开始"
    );

    search_history.previous(&mut cursor, "");
    assert_eq!(search_history.current(&cursor), Some("JavaScript"));
    search_history.previous(&mut cursor, "");
    assert_eq!(search_history.current(&cursor), Some("Rust"));

    cursor.reset();
    assert_eq!(search_history.current(&cursor), None);
}

#[test]
fn test_multiple_cursors() {
    let mut search_history = SearchHistory::new(None, QueryInsertionBehavior::AlwaysInsert);
    let mut cursor1 = SearchHistoryCursor::default();
    let mut cursor2 = SearchHistoryCursor::default();

    search_history.add(&mut cursor1, "Rust".to_string());
    search_history.add(&mut cursor1, "JavaScript".to_string());
    search_history.add(&mut cursor1, "TypeScript".to_string());

    search_history.add(&mut cursor2, "Python".to_string());
    search_history.add(&mut cursor2, "Java".to_string());
    search_history.add(&mut cursor2, "C++".to_string());

    assert_eq!(search_history.current(&cursor1), Some("TypeScript"));
    assert_eq!(search_history.current(&cursor2), Some("C++"));

    assert_eq!(
        search_history.previous(&mut cursor1, ""),
        Some("JavaScript")
    );
    assert_eq!(search_history.previous(&mut cursor2, ""), Some("Java"));

    assert_eq!(search_history.next(&mut cursor1), Some("TypeScript"));
    assert_eq!(search_history.next(&mut cursor1), Some("Python"));

    cursor1.reset();
    cursor2.reset();

    assert_eq!(search_history.current(&cursor1), None);
    assert_eq!(search_history.current(&cursor2), None);
}
