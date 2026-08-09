use std::time::Duration;

use gpui::{AnyElement, ClickEvent, CursorStyle, Window};
use ui::{CommonAnimationExt, Disclosure, Divider, DividerColor, Tooltip, prelude::*};
use util::time::duration_alt_display;

const ELAPSED_DISPLAY_THRESHOLD: Duration = Duration::from_secs(10);

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub struct TerminalSandboxWarning {
    pub title: SharedString,
    pub detail: SharedString,
    pub docs_url: SharedString,
}

#[derive(IntoElement, RegisterComponent)]
pub struct TerminalToolHeader {
    id: SharedString,
    hover_group: SharedString,
    working_dir: SharedString,
    is_expanded: bool,
    elapsed: Option<Duration>,
    running: bool,
    truncated_tooltip: Option<SharedString>,
    failed: bool,
    exit_code: Option<i32>,
    sandbox_warning: Option<TerminalSandboxWarning>,
    on_toggle_expand: Option<ClickHandler>,
    on_stop: Option<ClickHandler>,
    command_slot: Option<AnyElement>,
}

impl TerminalToolHeader {
    pub fn new(
        id: impl Into<SharedString>,
        hover_group: impl Into<SharedString>,
        working_dir: impl Into<SharedString>,
        is_expanded: bool,
    ) -> Self {
        Self {
            id: id.into(),
            hover_group: hover_group.into(),
            working_dir: working_dir.into(),
            is_expanded,
            elapsed: None,
            running: false,
            truncated_tooltip: None,
            failed: false,
            exit_code: None,
            sandbox_warning: None,
            on_toggle_expand: None,
            on_stop: None,
            command_slot: None,
        }
    }

    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    pub fn truncated(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.truncated_tooltip = Some(tooltip.into());
        self
    }

    pub fn failed(mut self, exit_code: Option<i32>) -> Self {
        self.failed = true;
        self.exit_code = exit_code;
        self
    }

    pub fn sandbox_warning(mut self, warning: TerminalSandboxWarning) -> Self {
        self.sandbox_warning = Some(warning);
        self
    }

    pub fn on_toggle_expand(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_expand = Some(Box::new(handler));
        self
    }

    pub fn on_stop(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_stop = Some(Box::new(handler));
        self
    }

    pub fn command_slot(mut self, element: impl IntoElement) -> Self {
        self.command_slot = Some(element.into_any_element());
        self
    }
}

impl RenderOnce for TerminalToolHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let show_elapsed = self
            .elapsed
            .is_some_and(|elapsed| elapsed > ELAPSED_DISPLAY_THRESHOLD);

        let Self {
            id,
            hover_group,
            working_dir,
            is_expanded,
            elapsed,
            running,
            truncated_tooltip,
            failed,
            exit_code,
            sandbox_warning,
            on_toggle_expand,
            on_stop,
            command_slot,
        } = self;

        let child_id = |name: &str| format!("terminal-tool-{name}-{id}");

        let header_bg = cx
            .theme()
            .colors()
            .element_background
            .blend(cx.theme().colors().editor_foreground.opacity(0.025));

        let header_row = h_flex()
            .id(child_id("header"))
            .pt_1()
            .pl_1p5()
            .pr_1()
            .flex_none()
            .gap_1()
            .justify_between()
            .rounded_t_md()
            .child(
                div().w_full().min_w_0().overflow_hidden().child(
                    Label::new(working_dir)
                        .buffer_font(cx)
                        .size(LabelSize::XSmall)
                        .color(Color::Muted)
                        .truncate_start(),
                ),
            )
            .child(
                Disclosure::new(child_id("disclosure"), is_expanded)
                    .opened_icon(IconName::ChevronUp)
                    .closed_icon(IconName::ChevronDown)
                    .visible_on_hover(&hover_group)
                    .when_some(on_toggle_expand, |this, handler| this.on_click(handler)),
            )
            .when(show_elapsed, |header| {
                let elapsed = elapsed.unwrap_or_default();
                header.child(
                    Label::new(format!("({})", duration_alt_display(elapsed)))
                        .buffer_font(cx)
                        .color(Color::Muted)
                        .size(LabelSize::XSmall)
                        .mr_0p5()
                        .when(truncated_tooltip.is_some(), |s| s.mr_0()),
                )
            })
            .when(running, |header| {
                header
                    .gap_1p5()
                    .child(
                        Icon::new(IconName::LoadCircle)
                            .size(IconSize::Small)
                            .color(Color::Muted)
                            .with_rotate_animation(2),
                    )
                    .child(Divider::vertical().color(DividerColor::Border).ml_1())
                    .child(
                        IconButton::new(child_id("stop"), IconName::Stop)
                            .icon_size(IconSize::Small)
                            .icon_color(Color::Error)
                            .tooltip(move |_window, cx| {
                                Tooltip::with_meta(
                                    "停止此命令",
                                    None,
                                    "Also possible by placing your cursor inside the terminal \
                                     and using regular terminal bindings.",
                                    cx,
                                )
                            })
                            .when_some(on_stop, |this, handler| this.on_click(handler)),
                    )
            })
            .when_some(truncated_tooltip, |header, tooltip| {
                header.child(
                    IconButton::new(child_id("truncated"), IconName::Info)
                        .cursor_style(CursorStyle::Arrow)
                        .style(ButtonStyle::Transparent)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Muted)
                        .tooltip(Tooltip::text(tooltip)),
                )
            })
            .when(failed, |header| {
                header.child(
                    IconButton::new(child_id("failed"), IconName::Close)
                        .cursor_style(CursorStyle::Arrow)
                        .style(ButtonStyle::Transparent)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Error)
                        .when_some(exit_code, |this, code| {
                            this.tooltip(Tooltip::text(format!("退出代码 {code}")))
                        }),
                )
            })
            .when_some(sandbox_warning, |header, warning| {
                let TerminalSandboxWarning {
                    title,
                    detail,
                    docs_url,
                } = warning;
                header.child(
                    IconButton::new(child_id("sandbox-not-applied"), IconName::LockOff)
                        .icon_size(IconSize::Small)
                        .tooltip(move |_window, cx| {
                            Tooltip::with_meta(
                                title.clone(),
                                None,
                                format!("{detail} 点击了解更多关于沙箱的信息。"),
                                cx,
                            )
                        })
                        .on_click(move |_, _, cx| cx.open_url(&docs_url)),
                )
            });

        v_flex()
            .group(hover_group)
            .text_xs()
            .bg(header_bg)
            .child(header_row)
            .children(command_slot)
    }
}

impl Component for TerminalToolHeader {
    fn scope() -> ComponentScope {
        ComponentScope::Agent
    }

    fn name() -> &'static str {
        "终端工具头部"
    }

    fn description() -> &'static str {
        "代理面板中终端工具调用卡片的顶部。"
    }

    fn preview(_window: &mut Window, cx: &mut App) -> AnyElement {
        let working_dir = "/Users/you/projects/zed";

        let card = |_id: &'static str, header: TerminalToolHeader| {
            v_flex()
                .w_full()
                .border_1()
                .border_color(cx.theme().colors().border.opacity(0.6))
                .rounded_md()
                .overflow_hidden()
                .child(
                    header.command_slot(
                        div().px_1p5().pb_1().child(
                            Label::new("cargo build --release")
                                .buffer_font(cx)
                                .size(LabelSize::XSmall),
                        ),
                    ),
                )
                .into_any_element()
        };

        let sandbox_warning = || TerminalSandboxWarning {
            title: "无沙箱运行".into(),
            detail: "此对话线程的其余部分允许无沙箱执行。".into(),
            docs_url: "https://zed.dev/docs/ai/sandboxing".into(),
        };

        v_flex()
            .gap_4()
            .child(example_group(vec![
                single_example(
                    "运行中",
                    card(
                        "running",
                        TerminalToolHeader::new(
                            "running",
                            "preview-terminal-header-group-running",
                            working_dir,
                            false,
                        )
                        .running(true),
                    ),
                ),
                single_example(
                    "已完成(长时间运行)",
                    card(
                        "elapsed",
                        TerminalToolHeader::new(
                            "elapsed",
                            "preview-terminal-header-group-elapsed",
                            working_dir,
                            false,
                        )
                        .elapsed(Duration::from_secs(83)),
                    ),
                ),
                single_example(
                    "输出已截断",
                    card(
                        "truncated",
                        TerminalToolHeader::new(
                            "truncated",
                            "preview-terminal-header-group-truncated",
                            working_dir,
                            true,
                        )
                        .truncated(
                            "Output is 2.5 MB long, and to avoid unexpected token \
                                     usage, only 16 KB was sent back to the agent.",
                        ),
                    ),
                ),
                single_example(
                    "失败,退出代码",
                    card(
                        "failed",
                        TerminalToolHeader::new(
                            "failed",
                            "preview-terminal-header-group-failed",
                            working_dir,
                            false,
                        )
                        .failed(Some(101)),
                    ),
                ),
                single_example(
                    "无沙箱运行",
                    card(
                        "sandbox",
                        TerminalToolHeader::new(
                            "sandbox",
                            "preview-terminal-header-group-sandbox",
                            working_dir,
                            false,
                        )
                        .sandbox_warning(sandbox_warning()),
                    ),
                ),
                single_example(
                    "长路径(从开头截断)",
                    div()
                        .w_80()
                        .child(card(
                            "long-path",
                            TerminalToolHeader::new(
                                "long-path",
                                "preview-terminal-header-group-long-path",
                                "/Users/you/Documents/GitHub/worktrees/some-monorepo/working-tree-three/packages/deeply/nested/service/backend/src",
                                false,
                            ),
                        ))
                        .into_any_element(),
                ),
                single_example(
                    "同时显示所有状态",
                    card(
                        "kitchen-sink",
                        TerminalToolHeader::new(
                            "kitchen-sink",
                            "preview-terminal-header-group-kitchen-sink",
                            working_dir,
                            true,
                        )
                        .elapsed(Duration::from_secs(3671))
                        .truncated("输出已被截断")
                        .failed(Some(1))
                        .sandbox_warning(sandbox_warning()),
                    ),
                ),
            ]))
            .into_any_element()
    }
}
