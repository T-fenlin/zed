#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    App, Bounds, BoxShadow, Context, Div, SharedString, Window, WindowBounds, WindowOptions, div,
    hsla, prelude::*, px, relative, rgb, size,
};
use gpui_platform::application;

struct Shadow {}

impl Shadow {
    fn base() -> Div {
        div()
            .size_16()
            .bg(rgb(0xffffff))
            .rounded_full()
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.0, 0.1))
    }

    fn square() -> Div {
        div()
            .size_16()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.0, 0.1))
    }

    fn rounded_small() -> Div {
        div()
            .size_16()
            .bg(rgb(0xffffff))
            .rounded(px(4.))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.0, 0.1))
    }

    fn rounded_medium() -> Div {
        div()
            .size_16()
            .bg(rgb(0xffffff))
            .rounded(px(8.))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.0, 0.1))
    }

    fn rounded_large() -> Div {
        div()
            .size_16()
            .bg(rgb(0xffffff))
            .rounded(px(12.))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.0, 0.1))
    }
}

fn example(label: impl Into<SharedString>, example: impl IntoElement) -> impl IntoElement {
    let label = label.into();

    div()
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .w(relative(1. / 6.))
        .border_r_1()
        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_1()
                .py_12()
                .child(example),
        )
        .child(
            div()
                .w_full()
                .border_t_1()
                .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                .p_1()
                .flex()
                .items_center()
                .child(label),
        )
}

impl Render for Shadow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("shadow-example")
            .overflow_y_scroll()
            .bg(rgb(0xffffff))
            .size_full()
            .text_xs()
            .child(div().flex().flex_col().w_full().children(vec![
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .flex_row()
                        .children(vec![
                            example(
                                "方形",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "圆角 4",
                                Shadow::rounded_small().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "圆角 8",
                                Shadow::rounded_medium().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "圆角 16",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "圆形",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                        ]),
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .w_full()
                        .children(vec![
                            example("无", Shadow::base()),
                            // 2Xsmall shadow
                            example("2X 小", Shadow::base().shadow_2xs()),
                            // Xsmall shadow
                            example("特小", Shadow::base().shadow_xs()),
                            // Small shadow
                            example("小", Shadow::base().shadow_sm()),
                            // Medium shadow
                            example("中", Shadow::base().shadow_md()),
                            // Large shadow
                            example("大", Shadow::base().shadow_lg()),
                            example("特大", Shadow::base().shadow_xl()),
                            example("2X 大", Shadow::base().shadow_2xl()),
                        ]),
                    // Horizontal list of increasing blur radii
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "模糊 0",
                                Shadow::base().shadow(vec![BoxShadow::new(
                                    px(0.),
                                    px(8.),
                                    hsla(0.0, 0.0, 0.0, 0.3),
                                )]),
                            ),
                            example(
                                "模糊 2",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(2.)),
                                ]),
                            ),
                            example(
                                "模糊 4",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(4.)),
                                ]),
                            ),
                            example(
                                "模糊 8",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "模糊 16",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(16.)),
                                ]),
                            ),
                        ]),
                    // Horizontal list of increasing spread radii
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "扩散 0",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "扩散 2",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(2.)),
                                ]),
                            ),
                            example(
                                "扩散 4",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(4.)),
                                ]),
                            ),
                            example(
                                "扩散 8",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "扩散 16",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(16.)),
                                ]),
                            ),
                        ]),
                    // Square spread examples
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "方形扩散 0",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "方形扩散 8",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "方形扩散 16",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(16.)),
                                ]),
                            ),
                        ]),
                    // Rounded large spread examples
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "大圆角扩散 0",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "大圆角扩散 8",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "大圆角扩散 16",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
                                        .blur_radius(px(8.))
                                        .spread_radius(px(16.)),
                                ]),
                            ),
                        ]),
                    // Directional shadows
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "左侧",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(-8.), px(0.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "右侧",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(8.), px(0.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "顶部",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(-8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "底部",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                        ]),
                    // Square directional shadows
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "方形左",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(-8.), px(0.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "方形右",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(8.), px(0.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "方形上",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(-8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "方形下",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                        ]),
                    // Rounded large directional shadows
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "大圆角左",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(-8.), px(0.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "大圆角右",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(8.), px(0.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "大圆角上",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(-8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                            example(
                                "大圆角下",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.5, 0.5, 0.3))
                                        .blur_radius(px(8.)),
                                ]),
                            ),
                        ]),
                    // Multiple shadows for different shapes
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .children(vec![
                            example(
                                "圆形多重",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(
                                        px(0.),
                                        px(-12.),
                                        hsla(0.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(12.),
                                        px(0.),
                                        hsla(60.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(0.),
                                        px(12.),
                                        hsla(120.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(-12.),
                                        px(0.),
                                        hsla(240.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                ]),
                            ),
                            example(
                                "方形多重",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(
                                        px(0.),
                                        px(-12.),
                                        hsla(0.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(12.),
                                        px(0.),
                                        hsla(60.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(0.),
                                        px(12.),
                                        hsla(120.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(-12.),
                                        px(0.),
                                        hsla(240.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                ]),
                            ),
                            example(
                                "大圆角多重",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(
                                        px(0.),
                                        px(-12.),
                                        hsla(0.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(12.),
                                        px(0.),
                                        hsla(60.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(0.),
                                        px(12.),
                                        hsla(120.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                    BoxShadow::new(
                                        px(-12.),
                                        px(0.),
                                        hsla(240.0 / 360., 1.0, 0.5, 0.3),
                                    )
                                    .blur_radius(px(8.))
                                    .spread_radius(px(2.)),
                                ]),
                            ),
                        ]),
                    // Inset shadows (CSS `box-shadow: inset ...`).
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .w_full()
                        .children(vec![
                            example(
                                "内阴影基础",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(0.), hsla(0.0, 0.0, 0.0, 0.5))
                                        .blur_radius(px(12.))
                                        .inset(),
                                ]),
                            ),
                            example(
                                "内阴影偏移",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(6.), px(6.), hsla(0.0, 0.0, 0.0, 0.5))
                                        .blur_radius(px(8.))
                                        .inset(),
                                ]),
                            ),
                            example(
                                "内阴影扩散",
                                Shadow::base().shadow(vec![
                                    BoxShadow::new(px(0.), px(0.), hsla(0.0, 0.0, 0.0, 0.5))
                                        .blur_radius(px(4.))
                                        .spread_radius(px(8.))
                                        .inset(),
                                ]),
                            ),
                            example(
                                "内阴影圆角",
                                Shadow::rounded_large().shadow(vec![
                                    BoxShadow::new(px(0.), px(4.), hsla(0.0, 0.0, 0.0, 0.5))
                                        .blur_radius(px(10.))
                                        .spread_radius(px(2.))
                                        .inset(),
                                ]),
                            ),
                            example(
                                "内阴影锐利",
                                Shadow::square().shadow(vec![
                                    BoxShadow::new(px(0.), px(0.), hsla(0.0, 0.0, 0.0, 0.6))
                                        .spread_radius(px(6.))
                                        .inset(),
                                ]),
                            ),
                        ]),
                    // Combined: drop + inset shadows on the same element.
                    div()
                        .border_b_1()
                        .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                        .flex()
                        .w_full()
                        .children(vec![example(
                            "外阴影 + 内阴影",
                            Shadow::rounded_medium().shadow(vec![
                                BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.25))
                                    .blur_radius(px(12.)),
                                BoxShadow::new(px(0.), px(2.), hsla(0.0, 0.0, 0.0, 0.4))
                                    .blur_radius(px(4.))
                                    .inset(),
                            ]),
                        )]),
                ]))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1000.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Shadow {}),
        )
        .unwrap();

        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
