#![allow(unused, dead_code)]
use gpui::{
    AnyElement, App, Entity, EventEmitter, FocusHandle, Focusable, Hsla, Task, actions, hsla,
};
use strum::IntoEnumIterator;
use theme::all_theme_colors;
use ui::{
    AudioStatus, Avatar, AvatarAudioStatusIndicator, AvatarAvailabilityIndicator, ButtonLike,
    Checkbox, CollaboratorAvailability, DecoratedIcon, ElevationIndex, Facepile, IconDecoration,
    Indicator, KeybindingHint, Switch, TintColor, Tooltip, prelude::*,
    utils::calculate_contrast_ratio,
};

use crate::{Item, Workspace};

actions!(
    dev,
    [
        /// Opens the theme preview window.
        OpenThemePreview
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace.register_action(|workspace, _: &OpenThemePreview, window, cx| {
            let theme_preview = cx.new(|cx| ThemePreview::new(window, cx));
            workspace.add_item_to_active_pane(Box::new(theme_preview), None, true, window, cx)
        });
    })
    .detach();
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, strum::EnumIter)]
enum ThemePreviewPage {
    Overview,
    Typography,
}

impl ThemePreviewPage {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Typography => "排版",
        }
    }
}

struct ThemePreview {
    current_page: ThemePreviewPage,
    focus_handle: FocusHandle,
}

impl ThemePreview {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            current_page: ThemePreviewPage::Overview,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn view(
        &self,
        page: ThemePreviewPage,
        window: &mut Window,
        cx: &mut Context<ThemePreview>,
    ) -> impl IntoElement {
        match page {
            ThemePreviewPage::Overview => self.render_overview_page(window, cx).into_any_element(),
            ThemePreviewPage::Typography => {
                self.render_typography_page(window, cx).into_any_element()
            }
        }
    }
}

impl EventEmitter<()> for ThemePreview {}

impl Focusable for ThemePreview {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
impl ThemePreview {}

impl Item for ThemePreview {
    type Event = ();

    fn to_item_events(_: &Self::Event, _: &mut dyn FnMut(crate::item::ItemEvent)) {}

    fn tab_content_text(&self, _detail: usize, cx: &App) -> SharedString {
        let name = cx.theme().name.clone();
        format!("{} 预览", name).into()
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        None
    }

    fn can_split(&self) -> bool {
        true
    }

    fn clone_on_split(
        &self,
        _workspace_id: Option<crate::WorkspaceId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Option<Entity<Self>>>
    where
        Self: Sized,
    {
        Task::ready(Some(cx.new(|cx| Self::new(window, cx))))
    }
}

const AVATAR_URL: &str = "https://avatars.githubusercontent.com/u/1714999?v=4";

impl ThemePreview {
    fn preview_bg(window: &mut Window, cx: &mut App) -> Hsla {
        cx.theme().colors().editor_background
    }

    fn render_text(
        &self,
        layer: ElevationIndex,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg = layer.bg(cx);

        let label_with_contrast = |label: &str, fg: Hsla| {
            let contrast = calculate_contrast_ratio(fg, bg);
            format!("{} ({:.2})", label, contrast)
        };

        v_flex()
            .gap_1()
            .child(Headline::new("文本").size(HeadlineSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .items_start()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Headline::new("标题字号").size(HeadlineSize::Small).color(Color::Muted))
                            .child(Headline::new("特大标题").size(HeadlineSize::XLarge))
                            .child(Headline::new("大标题").size(HeadlineSize::Large))
                            .child(Headline::new("中标题").size(HeadlineSize::Medium))
                            .child(Headline::new("小标题").size(HeadlineSize::Small))
                            .child(Headline::new("特小标题").size(HeadlineSize::XSmall)),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Headline::new("文本颜色").size(HeadlineSize::Small).color(Color::Muted))
                            .child(
                                Label::new(label_with_contrast(
                                    "默认文本",
                                    Color::Default.color(cx),
                                ))
                                .color(Color::Default),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "强调文本",
                                    Color::Accent.color(cx),
                                ))
                                .color(Color::Accent),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "冲突文本",
                                    Color::Conflict.color(cx),
                                ))
                                .color(Color::Conflict),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "已创建文本",
                                    Color::Created.color(cx),
                                ))
                                .color(Color::Created),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "已删除文本",
                                    Color::Deleted.color(cx),
                                ))
                                .color(Color::Deleted),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "禁用文本",
                                    Color::Disabled.color(cx),
                                ))
                                .color(Color::Disabled),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "错误文本",
                                    Color::Error.color(cx),
                                ))
                                .color(Color::Error),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "隐藏文本",
                                    Color::Hidden.color(cx),
                                ))
                                .color(Color::Hidden),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "提示文本",
                                    Color::Hint.color(cx),
                                ))
                                .color(Color::Hint),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "已忽略文本",
                                    Color::Ignored.color(cx),
                                ))
                                .color(Color::Ignored),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "信息文本",
                                    Color::Info.color(cx),
                                ))
                                .color(Color::Info),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "已修改文本",
                                    Color::Modified.color(cx),
                                ))
                                .color(Color::Modified),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "弱化文本",
                                    Color::Muted.color(cx),
                                ))
                                .color(Color::Muted),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "占位文本",
                                    Color::Placeholder.color(cx),
                                ))
                                .color(Color::Placeholder),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "选中文本",
                                    Color::Selected.color(cx),
                                ))
                                .color(Color::Selected),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "成功文本",
                                    Color::Success.color(cx),
                                ))
                                .color(Color::Success),
                            )
                            .child(
                                Label::new(label_with_contrast(
                                    "警告文本",
                                    Color::Warning.color(cx),
                                ))
                                .color(Color::Warning),
                            )
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Headline::new("换行文本").size(HeadlineSize::Small).color(Color::Muted))
                            .child(
                                div().max_w(px(200.)).child(
                                "这是一段较长的文本,应当换行显示为多行。它演示了当文本超出容器宽度时的行为。"
                            ))
                    )
            )
    }

    fn render_colors(
        &self,
        layer: ElevationIndex,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg = layer.bg(cx);
        let all_colors = all_theme_colors(cx);

        v_flex()
            .gap_1()
            .child(
                Headline::new("颜色")
                    .size(HeadlineSize::Small)
                    .color(Color::Muted),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .children(all_colors.into_iter().map(|(color, name)| {
                        let id = ElementId::Name(format!("{:?}-preview", color).into());
                        div().size_8().flex_none().child(
                            ButtonLike::new(id)
                                .child(
                                    div()
                                        .size_8()
                                        .bg(color)
                                        .border_1()
                                        .border_color(cx.theme().colors().border)
                                        .overflow_hidden(),
                                )
                                .size(ButtonSize::None)
                                .style(ButtonStyle::Transparent)
                                .tooltip(move |window, cx| {
                                    let name = name.clone();
                                    Tooltip::with_meta(name, None, format!("{:?}", color), cx)
                                }),
                        )
                    })),
            )
    }

    fn render_theme_layer(
        &self,
        layer: ElevationIndex,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .p_4()
            .bg(layer.bg(cx))
            .text_color(cx.theme().colors().text)
            .gap_2()
            .child(Headline::new(layer.clone().to_string()).size(HeadlineSize::Medium))
            .child(self.render_text(layer, window, cx))
            .child(self.render_colors(layer, window, cx))
    }

    fn render_overview_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("theme-preview-overview")
            .overflow_scroll()
            .size_full()
            .child(
                v_flex()
                    .child(Headline::new("主题预览").size(HeadlineSize::Large))
                    .child(div().w_full().text_color(cx.theme().colors().text_muted).child("此视图允许您预览主题中的各种 UI 元素。可用于测试对主题所做的更改。"))
                    )
            .child(self.render_theme_layer(ElevationIndex::Background, window, cx))
            .child(self.render_theme_layer(ElevationIndex::Surface, window, cx))
            .child(self.render_theme_layer(ElevationIndex::EditorSurface, window, cx))
            .child(self.render_theme_layer(ElevationIndex::ElevatedSurface, window, cx))
    }

    fn render_typography_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("theme-preview-typography")
            .overflow_scroll()
            .size_full()
            .child(v_flex()
                .gap_4()
                .child(Headline::new("标题 1").size(HeadlineSize::XLarge))
                .child(Label::new("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua."))
                .child(Headline::new("标题 2").size(HeadlineSize::Large))
                .child(Label::new("Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."))
                .child(Headline::new("标题 3").size(HeadlineSize::Medium))
                .child(Label::new("Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur."))
                .child(Headline::new("标题 4").size(HeadlineSize::Small))
                .child(Label::new("Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."))
                .child(Headline::new("标题 5").size(HeadlineSize::XSmall))
                .child(Label::new("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."))
                .child(Headline::new("正文").size(HeadlineSize::Small))
                .child(Label::new("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."))
            )
    }

    fn render_page_nav(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("theme-preview-nav")
            .items_center()
            .gap_4()
            .py_2()
            .bg(Self::preview_bg(window, cx))
            .children(ThemePreviewPage::iter().map(|p| {
                Button::new(ElementId::Name(p.name().into()), p.name())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.current_page = p;
                        cx.notify();
                    }))
                    .toggle_state(p == self.current_page)
                    .selected_style(ButtonStyle::Tinted(TintColor::Accent))
            }))
    }
}

impl Render for ThemePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl ui::IntoElement {
        v_flex()
            .id("theme-preview")
            .key_context("ThemePreview")
            .items_start()
            .overflow_hidden()
            .size_full()
            .max_h_full()
            .track_focus(&self.focus_handle)
            .px_2()
            .bg(Self::preview_bg(window, cx))
            .child(self.render_page_nav(window, cx))
            .child(self.view(self.current_page, window, cx))
    }
}
