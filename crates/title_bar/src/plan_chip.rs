use cloud_api_types::Plan;
use i18n::{keys, t};
use ui::{Chip, prelude::*};

/// A [`Chip`] that displays a [`Plan`].
#[derive(IntoElement)]
pub struct PlanChip {
    plan: Plan,
}

impl PlanChip {
    pub fn new(plan: Plan) -> Self {
        Self { plan }
    }
}

impl RenderOnce for PlanChip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let free_chip_bg = cx
            .theme()
            .colors()
            .editor_background
            .opacity(0.5)
            .blend(cx.theme().colors().text_accent.opacity(0.05));

        let pro_chip_bg = cx
            .theme()
            .colors()
            .editor_background
            .opacity(0.5)
            .blend(cx.theme().colors().text_accent.opacity(0.2));

        let (plan_name, label_color, bg_color) = match self.plan {
            Plan::ZedFree => (t(keys::titlebar::TITLEBAR_PLAN_FREE), Color::Default, free_chip_bg),
            Plan::ZedProTrial => (t(keys::titlebar::TITLEBAR_PLAN_PRO_TRIAL), Color::Accent, pro_chip_bg),
            Plan::ZedPro => (t(keys::titlebar::TITLEBAR_PLAN_PRO), Color::Accent, pro_chip_bg),
            Plan::ZedBusiness => (t(keys::titlebar::TITLEBAR_PLAN_BUSINESS), Color::Accent, pro_chip_bg),
            Plan::ZedVip => ("VIP".to_string(), Color::Accent, pro_chip_bg),
            Plan::ZedStudent => (t(keys::titlebar::TITLEBAR_PLAN_STUDENT), Color::Accent, pro_chip_bg),
        };

        Chip::new(plan_name.to_string())
            .bg_color(bg_color)
            .label_color(label_color)
    }
}
