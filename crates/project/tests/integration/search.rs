use std::io::BufReader;

use language::Buffer;
use project::search::SearchQuery;
use text::Rope;
use util::{
    paths::{PathMatcher, PathStyle},
    rel_path::RelPath,
};

#[test]
fn path_matcher_creation_for_valid_paths() {
    for valid_path in [
        "file",
        "Cargo.toml",
        ".DS_Store",
        "~/dir/another_dir/",
        "./dir/file",
        "dir/[a-z].txt",
    ] {
        let path_matcher = PathMatcher::new(&[valid_path.to_owned()], PathStyle::local())
            .unwrap_or_else(|e| panic!("有效路径 {valid_path} 应被接受,但出现错误:{e}"));
        assert!(
            path_matcher.is_match(&RelPath::new(valid_path.as_ref(), PathStyle::local()).unwrap()),
            "有效路径 {valid_path} 的路径匹配器应能匹配自身"
        )
    }
}

#[test]
fn path_matcher_creation_for_globs() {
    for invalid_glob in ["dir/[].txt", "dir/[a-z.txt", "dir/{file"] {
        match PathMatcher::new(&[invalid_glob.to_owned()], PathStyle::local()) {
            Ok(_) => panic!("无效的通配符 {invalid_glob} 不应被接受"),
            Err(_expected) => {}
        }
    }

    for valid_glob in [
        "dir/?ile",
        "dir/*.txt",
        "dir/**/file",
        "dir/[a-z].txt",
        "{dir,file}",
    ] {
        match PathMatcher::new(&[valid_glob.to_owned()], PathStyle::local()) {
            Ok(_expected) => {}
            Err(e) => panic!("有效的通配符应被接受,但出现错误:{e}"),
        }
    }
}

#[test]
fn test_case_sensitive_pattern_items() {
    let case_sensitive = false;
    let search_query = SearchQuery::regex(
        "test\\C",
        false,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("应能创建正则表达式搜索查询");

    assert_eq!(
        search_query.case_sensitive(),
        true,
        "当查询中包含 \\C 模式项时,应启用区分大小写。"
    );

    let case_sensitive = true;
    let search_query = SearchQuery::regex(
        "test\\c",
        true,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("应能创建正则表达式搜索查询");

    assert_eq!(
        search_query.case_sensitive(),
        false,
        "当存在 \\c 模式项时,即使初始设置为 true,也应禁用区分大小写。"
    );

    let case_sensitive = false;
    let search_query = SearchQuery::regex(
        "test\\c\\C",
        false,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("应能创建正则表达式搜索查询");

    assert_eq!(
        search_query.case_sensitive(),
        true,
        "当 \\C 是最后一个模式项时,即使前面有 \\c,也应启用区分大小写。"
    );

    let case_sensitive = false;
    let search_query = SearchQuery::regex(
        "tests\\\\C",
        false,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("应能创建正则表达式搜索查询");

    assert_eq!(
        search_query.case_sensitive(),
        false,
        "当 \\C 模式项前面有反斜杠时,不应启用区分大小写。"
    );
}

#[gpui::test]
async fn test_multiline_regex_crlf(cx: &mut gpui::TestAppContext) {
    let search_query = SearchQuery::regex(
        "^hello$\r?\n",
        false,
        false,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("应能创建正则表达式搜索查询");

    let text = Rope::from("hello\r\nworld\r\nhello\r\nworld");
    let snapshot = cx
        .update(|app| Buffer::build_snapshot(text, None, None, None, app))
        .await;

    let results = search_query.search(&snapshot, None).await;
    assert_eq!(results, vec![0..7, 14..21]);
}

#[gpui::test]
async fn test_multiline_regex(cx: &mut gpui::TestAppContext) {
    let search_query = SearchQuery::regex(
        "^hello$\n",
        false,
        false,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("应能创建正则表达式搜索查询");

    let text = Rope::from("hello\nworld\nhello\nworld");
    let snapshot = cx
        .update(|app| Buffer::build_snapshot(text, None, None, None, app))
        .await;

    let results = search_query.search(&snapshot, None).await;
    assert_eq!(results, vec![0..6, 12..18]);
}

#[gpui::test]
async fn regex_with_eol_detects_lines() {
    let re = SearchQuery::regex(
        "Bool$",
        false,
        false,
        false,
        false,
        PathMatcher::default(),
        PathMatcher::default(),
        false,
        None,
    )
    .unwrap();
    let input = " Bool\nsomething else";
    let result = re
        .detect(BufReader::new(Box::new(input.as_bytes())))
        .await
        .unwrap();
    assert!(result.is_some());
}

#[gpui::test]
async fn multi_line_regex_detects_matches() {
    let re = SearchQuery::regex(
        "Bool$\nbool",
        false,
        false,
        false,
        false,
        PathMatcher::default(),
        PathMatcher::default(),
        false,
        None,
    )
    .unwrap();
    let input = " Bool\nbool";
    let result = re
        .detect(BufReader::new(Box::new(input.as_bytes())))
        .await
        .unwrap();
    assert!(result.is_some());
}
