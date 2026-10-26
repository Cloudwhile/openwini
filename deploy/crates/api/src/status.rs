//! 插件状态模型 —— 唯一权威定义。
//!
//! `plugins.status` 共 8 个合法值（数据库 CHECK 约束），按语义分为四组：
//!
//! 1. 审核流转（申请在审核队列中的位置）：
//!    - `draft`      草稿：已创建插件资料，尚未提交审核。
//!    - `pending`    待审核：申请在人工审核队列中。
//!    - `rejected`   未通过：审核驳回；若同时存在 `stop_reason`，表示管理员
//!      对**已发布插件**执行「退回修改」（见 display_status）。
//!    - `cancelled`  已取消：开发者主动撤回申请（仅 draft/pending 可取消）。
//!
//! 2. 发布状态（插件是否对外分发）：
//!    - `published`  已发布：进入公开目录与宿主索引。
//!    - `paused`     开发者暂停：开发者自助暂停分发，可随时自行恢复。
//!    - `stopped`    平台停止：管理员强制停止分发（含封禁级联），开发者需重新提交审核。
//!    - `unlisted`   取消公开展示：开发者主动下架，公开详情不再可见，重新上架需重新提交审核。
//!
//! 3. 公开目录可见性（由 status 推导，不是独立字段）：
//!    - 目录列表 / 宿主索引：仅 `published`。
//!    - 公开详情页：`published`、`stopped`（中性下架说明）、`paused`（暂停说明）；
//!      `unlisted` 及一切未发布状态一律 404。
//!
//! 4. 更新策略：`plugins.updates_halted`（独立布尔字段，仅对 `published` 有意义）。
//!    冻结后：索引保留当前版本，新版本无法提交，直到管理员「恢复更新」。
//!
//! 对外展示状态（display status）是对 API 消费者更友好的推导值：
//! - `published` + `updates_halted` → `updates_halted`（已发布但更新被冻结）
//! - `rejected` + 存在 `stop_reason` → `returned`（管理员退回修改，区别于审核驳回）
/// 展示状态推导。
///
/// `stop_reason_present`：`plugins.stop_reason` 是否非空（审核驳回不写该字段，
/// 管理员「退回修改」会写入）。
pub fn display_status(
    status: &str,
    updates_halted: bool,
    stop_reason_present: bool,
) -> &'static str {
    match status {
        "published" if updates_halted => "updates_halted",
        "rejected" if stop_reason_present => "returned",
        "draft" => "draft",
        "pending" => "pending",
        "published" => "published",
        "rejected" => "rejected",
        "cancelled" => "cancelled",
        "stopped" => "stopped",
        "paused" => "paused",
        "unlisted" => "unlisted",
        _ => "unknown",
    }
}

/// 开发者能否编辑插件资料（仓库/主页/文档链接）。
pub fn can_edit(status: &str) -> bool {
    matches!(status, "draft" | "pending" | "rejected" | "unlisted")
}

/// 开发者能否取消申请。
pub fn can_cancel(status: &str) -> bool {
    matches!(status, "draft" | "pending")
}

/// 开发者能否提交首次审核（含从驳回/取消/下架重新提交）。
pub fn can_submit(status: &str) -> bool {
    matches!(
        status,
        "draft" | "rejected" | "cancelled" | "pending" | "unlisted"
    )
}

/// 开发者能否对已发布插件提交新版本。
pub fn can_submit_new_version(status: &str, updates_halted: bool) -> bool {
    status == "published" && !updates_halted
}

/// 开发者能否自助暂停（published → paused）。
pub fn can_pause(status: &str) -> bool {
    status == "published"
}

/// 开发者能否自助恢复（paused → published）。
pub fn can_resume_self(status: &str) -> bool {
    status == "paused"
}

/// 开发者能否取消公开展示（published/paused → unlisted）。
pub fn can_unlist(status: &str) -> bool {
    matches!(status, "published" | "paused")
}

/// 管理员能否停止分发（published → stopped）。
pub fn admin_can_stop(status: &str) -> bool {
    status == "published"
}

/// 管理员能否退回修改（published → rejected + stop_reason）。
pub fn admin_can_unpublish(status: &str) -> bool {
    status == "published"
}

/// 管理员能否冻结更新（published + updates_halted=false）。
pub fn admin_can_halt(status: &str, updates_halted: bool) -> bool {
    status == "published" && !updates_halted
}

/// 管理员恢复操作：stopped → published，或解除更新冻结。
pub fn admin_can_resume(status: &str, updates_halted: bool) -> bool {
    status == "stopped" || (status == "published" && updates_halted)
}

/// 公开详情页可见性策略。
pub fn publicly_visible_detail(status: &str) -> bool {
    matches!(status, "published" | "stopped" | "paused")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_status_maps_updates_halted() {
        assert_eq!(display_status("published", true, false), "updates_halted");
        assert_eq!(display_status("published", false, false), "published");
    }

    #[test]
    fn display_status_maps_returned_vs_rejected() {
        // 审核驳回：无 stop_reason
        assert_eq!(display_status("rejected", false, false), "rejected");
        // 管理员退回修改：带 stop_reason
        assert_eq!(display_status("rejected", false, true), "returned");
        // 更新冻结与退回不会同时出现（退回会清除 halted），但若出现，冻结优先级更高仅对 published 生效
        assert_eq!(display_status("rejected", true, true), "returned");
    }

    #[test]
    fn display_status_passthrough_others() {
        for s in [
            "draft",
            "pending",
            "cancelled",
            "stopped",
            "paused",
            "unlisted",
        ] {
            assert_eq!(display_status(s, false, false), s);
        }
    }

    #[test]
    fn developer_transition_predicates() {
        assert!(
            can_edit("draft")
                && can_edit("pending")
                && can_edit("rejected")
                && can_edit("unlisted")
        );
        assert!(!can_edit("published") && !can_edit("stopped") && !can_edit("cancelled"));
        assert!(
            can_submit("unlisted")
                && can_submit("rejected")
                && can_submit("draft")
                && can_submit("cancelled")
        );
        assert!(!can_submit("published") && !can_submit("stopped"));
        assert!(can_submit_new_version("published", false));
        assert!(!can_submit_new_version("published", true));
        assert!(!can_submit_new_version("paused", false));
        assert!(can_pause("published") && !can_pause("paused"));
        assert!(can_resume_self("paused") && !can_resume_self("published"));
        assert!(can_unlist("paused") && can_unlist("published") && !can_unlist("draft"));
    }

    #[test]
    fn admin_transition_predicates() {
        assert!(admin_can_stop("published") && !admin_can_stop("stopped"));
        assert!(admin_can_unpublish("published") && !admin_can_unpublish("rejected"));
        assert!(admin_can_halt("published", false) && !admin_can_halt("published", true));
        assert!(admin_can_resume("stopped", false));
        assert!(admin_can_resume("published", true));
        assert!(!admin_can_resume("published", false));
        assert!(!admin_can_resume("paused", false));
    }

    #[test]
    fn public_detail_visibility() {
        assert!(publicly_visible_detail("published"));
        assert!(publicly_visible_detail("stopped"));
        assert!(publicly_visible_detail("paused"));
        // 开发者明确取消公开展示 → 详情不再可见
        assert!(!publicly_visible_detail("unlisted"));
        assert!(!publicly_visible_detail("draft"));
        assert!(!publicly_visible_detail("pending"));
        assert!(!publicly_visible_detail("rejected"));
        assert!(!publicly_visible_detail("cancelled"));
    }
}
