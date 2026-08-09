use std::sync::Arc;

use editor::Editor;
use gpui::{AppContext as _, AsyncWindowContext, WeakEntity, WindowBounds, WindowOptions};
use language::Buffer;
use multi_buffer::Anchor;
use project::search::SearchQuery;
use workspace::searchable::SearchableItem;

#[derive(Debug)]
struct Args {
    file: String,
    query: String,
    replace: Option<String>,
    regex: bool,
    whole_word: bool,
    case_sensitive: bool,
    single: bool,
}

fn parse_args() -> Args {
    let mut args_iter = std::env::args().skip(1);
    let mut parsed = Args {
        file: String::new(),
        query: String::new(),
        replace: None,
        regex: false,
        whole_word: false,
        case_sensitive: false,
        single: false,
    };

    let mut positional = Vec::new();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--regex" => parsed.regex = true,
            "--whole-word" => parsed.whole_word = true,
            "--case-sensitive" => parsed.case_sensitive = true,
            "--single" => parsed.single = true,
            "-r" | "--replace" => {
                parsed.replace = args_iter.next();
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: editor_benchmarks [OPTIONS] <FILE> <QUERY>\n\n\
                     Arguments:\n  \
                       <FILE>   Path to the file to search in\n  \
                       <QUERY>  The search query string\n\n\
                     Options:\n  \
                       -r, --replace <TEXT>  Replacement text (runs replace_all)\n      \
                       --regex              Treat query as regex\n      \
                       --whole-word         Match whole words only\n      \
                       --case-sensitive     Case-sensitive matching\n  \
                       -h, --help           Print help"
                );
                std::process::exit(0);
            }
            other => positional.push(other.to_string()),
        }
    }

    if positional.len() < 2 {
        eprintln!("Usage: editor_benchmarks [OPTIONS] <FILE> <QUERY>");
        std::process::exit(1);
    }
    parsed.file = positional.remove(0);
    parsed.query = positional.remove(0);
    parsed
}

fn main() {
    let args = parse_args();

    let file_contents = std::fs::read_to_string(&args.file).expect("无法读取输入文件");
    let file_len = file_contents.len();
    println!("已读取 {}({file_len} 字节)", args.file);

    let mut query = if args.regex {
        SearchQuery::regex(
            &args.query,
            args.whole_word,
            args.case_sensitive,
            false,
            false,
            Default::default(),
            Default::default(),
            false,
            None,
        )
        .expect("无效的正则表达式查询")
    } else {
        SearchQuery::text(
            &args.query,
            args.whole_word,
            args.case_sensitive,
            false,
            Default::default(),
            Default::default(),
            false,
            None,
        )
        .expect("无效的文本查询")
    };

    if let Some(replacement) = args.replace.as_deref() {
        query = query.with_replacement(replacement.to_string());
    }

    let query = Arc::new(query);
    let has_replacement = args.replace.is_some();
    let single = args.single;

    gpui_platform::headless().run(move |cx| {
        release_channel::init_test(
            semver::Version::new(0, 0, 0),
            release_channel::ReleaseChannel::Dev,
            cx,
        );
        settings::init(cx);
        theme::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);

        let buffer = cx.new(|cx| Buffer::local(file_contents, cx));

        let window_handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(gpui::Bounds {
                        origin: Default::default(),
                        size: gpui::size(gpui::px(800.0), gpui::px(600.0)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Editor::for_buffer(buffer, None, window, cx)),
            )
            .expect("无法打开窗口");

        window_handle
            .update(cx, move |_, window, cx| {
                cx.spawn_in(
                    window,
                    async move |weak: WeakEntity<Editor>,
                                cx: &mut AsyncWindowContext|
                                -> anyhow::Result<()> {
                        let find_task = weak.update_in(cx, |editor, window, cx| {
                            editor.find_matches(query.clone(), window, cx)
                        })?;

                        println!("正在查找匹配项...");
                        let timer = std::time::Instant::now();
                        let matches: Vec<std::ops::Range<Anchor>> = find_task.await;
                        let find_elapsed = timer.elapsed();
                        println!("在 {find_elapsed:?} 内找到 {} 个匹配项", matches.len());

                        if has_replacement && !matches.is_empty() {
                            window_handle.update(cx, |editor: &mut Editor, window, cx| {
                                let to_replace = if single { 1 } else { matches.len() };
                                let mut match_iter = matches.iter().take(to_replace);
                                println!("正在替换 {to_replace} 个匹配项...");
                                let timer = std::time::Instant::now();
                                editor.replace_all(
                                    &mut match_iter,
                                    &query,
                                    Default::default(),
                                    window,
                                    cx,
                                );
                                let replace_elapsed = timer.elapsed();
                                println!("在 {replace_elapsed:?} 内替换了 {to_replace} 个匹配项");
                            })?;
                        }

                        std::process::exit(0);
                    },
                )
                .detach();
            })
            .unwrap();
    });
}
