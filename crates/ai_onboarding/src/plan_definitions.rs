use gpui::{IntoElement, ParentElement};
use ui::{List, ListBulletItem, prelude::*};

/// Centralized definitions for Zed AI plans
pub struct PlanDefinitions;

impl PlanDefinitions {
    pub fn free_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("2,000 条已接受的编辑预测"))
            .child(ListBulletItem::new(
                "使用您的 AI API 密钥无限次提示",
            ))
            .child(ListBulletItem::new("无限使用外部代理"))
    }

    pub fn sign_in_upsell(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("无限编辑预测"))
            .child(ListBulletItem::new("Zed 代理中包含 $20 的令牌"))
            .child(ListBulletItem::new("无需信用卡"))
    }

    pub fn pro_trial(&self, period: bool) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("Zed 代理中包含 $20 的令牌"))
            .child(ListBulletItem::new("无限编辑预测"))
            .when(period, |this| {
                this.child(ListBulletItem::new(
                    "免费试用 14 天,无需信用卡",
                ))
            })
    }

    pub fn pro_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("Zed 代理中包含 $5 的令牌"))
            .child(ListBulletItem::new("超出 $5 后按用量计费"))
            .child(ListBulletItem::new("无限编辑预测"))
    }

    pub fn business_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("无限编辑预测"))
            .child(ListBulletItem::new("按用量计费"))
    }

    pub fn vip_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("无限编辑预测"))
            .child(ListBulletItem::new("Zed 代理中的令牌"))
    }

    pub fn student_plan(&self) -> impl IntoElement {
        List::new()
            .child(ListBulletItem::new("无限编辑预测"))
            .child(ListBulletItem::new("Zed 代理中包含 $10 的令牌"))
            .child(ListBulletItem::new(
                "可选的信用包,用于额外用量",
            ))
    }
}
