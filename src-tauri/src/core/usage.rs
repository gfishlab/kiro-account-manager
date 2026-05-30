//! Kiro 账号配额/超额相关的统一工具
//!
//! 之前这些判断散在 3 个文件里（auto_switch、account、proxy），
//! 字段大小写、字段优先级、is_capped 逻辑都不一致，
//! 这里抽一份统一实现。
//!
//! 字段对照（IDE 源码）：
//!   subscriptionInfo.overageCapability    "OVERAGE_CAPABLE" / "OVERAGE_INCAPABLE"
//!   overageConfiguration.overageStatus    "ENABLED"         / "DISABLED"
//!   usageBreakdownList[0].currentUsage / currentUsageWithPrecision
//!   usageBreakdownList[0].usageLimit / usageLimitWithPrecision
//!   usageBreakdownList[0].overageCap / overageCapWithPrecision

use serde_json::Value;

/// 资格枚举（账号有没有资格开超额）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverageCapability {
    /// Pro / Pro+ 账号有资格开超额
    Capable,
    /// Free / Power 账号没资格
    Incapable,
    /// API 没返回这个字段（账号未抓 usage 数据等）
    Unknown,
}

impl OverageCapability {
    pub fn from_usage_data(usage_data: Option<&Value>) -> Self {
        match usage_data
            .and_then(|d| d.get("subscriptionInfo"))
            .and_then(|s| s.get("overageCapability"))
            .and_then(Value::as_str)
        {
            Some("OVERAGE_CAPABLE") => Self::Capable,
            Some("OVERAGE_INCAPABLE") => Self::Incapable,
            _ => Self::Unknown,
        }
    }

    #[allow(dead_code)]
    pub fn is_capable(self) -> bool {
        self == Self::Capable
    }
}

/// 状态枚举（账号实际有没有开超额）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverageStatus {
    Enabled,
    Disabled,
    /// API 没返回 overageConfiguration 字段
    Unknown,
}

impl OverageStatus {
    pub fn from_usage_data(usage_data: Option<&Value>) -> Self {
        match usage_data
            .and_then(|d| d.get("overageConfiguration"))
            .and_then(|c| c.get("overageStatus"))
            .and_then(Value::as_str)
        {
            Some("ENABLED") => Self::Enabled,
            Some("DISABLED") => Self::Disabled,
            _ => Self::Unknown,
        }
    }

    pub fn is_enabled(self) -> bool {
        self == Self::Enabled
    }
}

/// 配额数字（精度优先版本，回退到整数版本）
fn read_amount(item: &Value, integer_key: &str, precision_key: &str) -> Option<f64> {
    item.get(precision_key)
        .and_then(Value::as_f64)
        .or_else(|| item.get(integer_key).and_then(Value::as_f64))
        .or_else(|| item.get(integer_key).and_then(Value::as_i64).map(|n| n as f64))
}

/// usageBreakdownList[0] 三个核心字段：current / limit / overage_cap
#[derive(Debug, Clone, Copy)]
pub struct UsageBreakdown {
    pub current: f64,
    pub limit: f64,
    pub overage_cap: f64,
}

impl UsageBreakdown {
    pub fn from_usage_data(usage_data: Option<&Value>) -> Option<Self> {
        let item = usage_data?
            .get("usageBreakdownList")?
            .as_array()?
            .first()?;

        let current = read_amount(item, "currentUsage", "currentUsageWithPrecision")?;
        let limit = read_amount(item, "usageLimit", "usageLimitWithPrecision")?;
        let overage_cap = read_amount(item, "overageCap", "overageCapWithPrecision").unwrap_or(0.0);

        Some(Self { current, limit, overage_cap })
    }

    /// 总可用额度（含超额）
    pub fn effective_limit(&self, status: OverageStatus) -> f64 {
        if status.is_enabled() {
            self.limit + self.overage_cap
        } else {
            self.limit
        }
    }

    /// 用量百分比（0.0 ~ 100.0+），分母用有效额度
    pub fn usage_percentage(&self, status: OverageStatus) -> f64 {
        let denom = self.effective_limit(status);
        if denom <= 0.0 {
            return 0.0;
        }
        (self.current / denom) * 100.0
    }
}

/// 完整的配额视图：主配额 + 免费试用 + 奖励 + 超额
///
/// 用于自动切号判断"剩余可用配额"等场景。
#[derive(Debug, Clone, Copy)]
pub struct UsageDetails {
    pub main_limit: f64,
    pub main_usage: f64,
    pub trial_limit: f64,
    pub trial_usage: f64,
    pub bonus_limit: f64,
    pub bonus_usage: f64,
    pub overage_cap: f64,
}

impl UsageDetails {
    pub fn from_usage_data(usage_data: Option<&Value>) -> Option<Self> {
        let item = usage_data?
            .get("usageBreakdownList")?
            .as_array()?
            .first()?;

        let main_limit = item.get("usageLimit").and_then(Value::as_f64).unwrap_or(0.0);
        let main_usage = item
            .get("currentUsage")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);

        // 免费试用（仅 ACTIVE 时计入）
        let trial_info = item.get("freeTrialInfo");
        let trial_active = trial_info
            .and_then(|t| t.get("freeTrialStatus"))
            .and_then(Value::as_str)
            == Some("ACTIVE");
        let (trial_limit, trial_usage) = if trial_active {
            let l = trial_info
                .and_then(|t| t.get("usageLimit"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let u = trial_info
                .and_then(|t| t.get("currentUsage"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            (l, u)
        } else {
            (0.0, 0.0)
        };

        // 奖励（仅未过期 + ACTIVE 计入）
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let (bonus_limit, bonus_usage) = item
            .get("bonuses")
            .and_then(Value::as_array)
            .map(|bonuses| {
                bonuses.iter().fold((0.0, 0.0), |(l, u), b| {
                    let expiry_ms = b
                        .get("expiresAt")
                        .and_then(Value::as_i64)
                        .map(|t| t * 1000)
                        .unwrap_or(i64::MAX);
                    let active = b.get("status").and_then(Value::as_str) == Some("ACTIVE");
                    if expiry_ms > now_ms && active {
                        let bl = b.get("usageLimit").and_then(Value::as_f64).unwrap_or(0.0);
                        let bu = b.get("currentUsage").and_then(Value::as_f64).unwrap_or(0.0);
                        (l + bl, u + bu)
                    } else {
                        (l, u)
                    }
                })
            })
            .unwrap_or((0.0, 0.0));

        let overage_cap = if OverageStatus::from_usage_data(usage_data).is_enabled() {
            item.get("overageCap").and_then(Value::as_f64).unwrap_or(0.0)
        } else {
            0.0
        };

        Some(Self {
            main_limit,
            main_usage,
            trial_limit,
            trial_usage,
            bonus_limit,
            bonus_usage,
            overage_cap,
        })
    }

    /// 剩余可用配额（含主配额 + 试用 + 奖励 + 已开启的超额，减去全部已用）
    pub fn remaining(&self) -> f64 {
        let total_limit = self.main_limit + self.trial_limit + self.bonus_limit + self.overage_cap;
        let total_usage = self.main_usage + self.trial_usage + self.bonus_usage;
        total_limit - total_usage
    }
}

/// 账号是否处于超额状态
///
/// 超额状态：开启了超额功能，且当前用量已超过基础配额但未达到封顶
/// 条件：overageStatus=ENABLED && current > limit && current < (limit + overageCap)
pub fn is_in_overage(usage_data: Option<&Value>) -> bool {
    let Some(breakdown) = UsageBreakdown::from_usage_data(usage_data) else {
        return false;
    };
    if breakdown.limit <= 0.0 {
        return false;
    }
    let status = OverageStatus::from_usage_data(usage_data);
    if !status.is_enabled() {
        return false;
    }
    // 超过基础配额但未封顶
    breakdown.current > breakdown.limit && !is_usage_capped(usage_data)
}

/// 账号是否已封顶不可用
///
/// 封顶判断：所有可用配额（基础+试用+奖励+超额）都用完了
/// 即 remaining <= 0，其中 remaining = (main + trial + bonus + overage) - (main_usage + trial_usage + bonus_usage)
pub fn is_usage_capped(usage_data: Option<&Value>) -> bool {
    let Some(details) = UsageDetails::from_usage_data(usage_data) else {
        return false;
    };
    // 总配额 <= 0 说明账号没有任何配额，不算封顶
    let total_limit = details.main_limit + details.trial_limit + details.bonus_limit + details.overage_cap;
    if total_limit <= 0.0 {
        return false;
    }
    // 剩余配额 <= 0 才算真正封顶
    details.remaining() <= 0.0
}

/// 账号配额是否超过给定阈值（百分比，0-100）
pub fn usage_exceeds_threshold(usage_data: Option<&Value>, threshold_pct: f64) -> bool {
    let Some(breakdown) = UsageBreakdown::from_usage_data(usage_data) else {
        return false;
    };
    if breakdown.limit <= 0.0 {
        return false;
    }
    let status = OverageStatus::from_usage_data(usage_data);
    breakdown.usage_percentage(status) >= threshold_pct
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data(status: &str, current: f64, limit: f64, cap: f64, capability: &str) -> Value {
        json!({
            "subscriptionInfo": { "overageCapability": capability },
            "overageConfiguration": { "overageStatus": status },
            "usageBreakdownList": [{
                "currentUsageWithPrecision": current,
                "usageLimitWithPrecision": limit,
                "overageCapWithPrecision": cap
            }]
        })
    }

    #[test]
    fn capped_when_disabled_over_limit() {
        let d = data("DISABLED", 100.0, 100.0, 0.0, "OVERAGE_INCAPABLE");
        assert!(is_usage_capped(Some(&d)));
    }

    #[test]
    fn not_capped_when_disabled_under_limit() {
        let d = data("DISABLED", 50.0, 100.0, 0.0, "OVERAGE_INCAPABLE");
        assert!(!is_usage_capped(Some(&d)));
    }

    #[test]
    fn not_capped_when_enabled_within_overage() {
        // 已用 150，base 100，overage 100 → 总 200，未封顶
        let d = data("ENABLED", 150.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!(!is_usage_capped(Some(&d)));
    }

    #[test]
    fn capped_when_enabled_over_overage() {
        // 已用 250，base 100，overage 100 → 总 200，已封顶
        let d = data("ENABLED", 250.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!(is_usage_capped(Some(&d)));
    }

    #[test]
    fn capability_parsing() {
        let d = data("DISABLED", 0.0, 100.0, 0.0, "OVERAGE_CAPABLE");
        assert!(OverageCapability::from_usage_data(Some(&d)).is_capable());
        let d2 = data("DISABLED", 0.0, 100.0, 0.0, "OVERAGE_INCAPABLE");
        assert_eq!(OverageCapability::from_usage_data(Some(&d2)), OverageCapability::Incapable);
        assert_eq!(OverageCapability::from_usage_data(None), OverageCapability::Unknown);
    }

    #[test]
    fn threshold_uses_effective_limit() {
        // base 100，overage 100，已用 150 → enabled 时 75%，disabled 时算 150% 但 disabled 已经 capped
        let enabled = data("ENABLED", 150.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!((UsageBreakdown::from_usage_data(Some(&enabled)).unwrap()
            .usage_percentage(OverageStatus::Enabled) - 75.0).abs() < 0.01);
        assert!(!usage_exceeds_threshold(Some(&enabled), 80.0));
        assert!(usage_exceeds_threshold(Some(&enabled), 70.0));
    }

    #[test]
    fn in_overage_when_between_limit_and_cap() {
        // 已用 150，base 100，overage 100 → 在超额区间
        let d = data("ENABLED", 150.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!(is_in_overage(Some(&d)));
    }

    #[test]
    fn not_in_overage_when_disabled() {
        // 超额未开启，即使超过 limit 也不算超额状态
        let d = data("DISABLED", 150.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!(!is_in_overage(Some(&d)));
    }

    #[test]
    fn not_in_overage_when_under_limit() {
        // 还没超过基础配额
        let d = data("ENABLED", 50.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!(!is_in_overage(Some(&d)));
    }

    #[test]
    fn not_in_overage_when_capped() {
        // 已经封顶了
        let d = data("ENABLED", 250.0, 100.0, 100.0, "OVERAGE_CAPABLE");
        assert!(!is_in_overage(Some(&d)));
    }

    #[test]
    fn details_remaining_includes_trial_and_bonus() {
        let d = json!({
            "overageConfiguration": { "overageStatus": "ENABLED" },
            "usageBreakdownList": [{
                "usageLimit": 100.0,
                "currentUsage": 30.0,
                "overageCap": 50.0,
                "freeTrialInfo": {
                    "freeTrialStatus": "ACTIVE",
                    "usageLimit": 20.0,
                    "currentUsage": 5.0
                },
                "bonuses": [
                    // 已过期的 bonus 应被忽略
                    { "status": "ACTIVE", "expiresAt": 0, "usageLimit": 50.0, "currentUsage": 0.0 },
                    // 状态非 ACTIVE 也应忽略
                    { "status": "EXPIRED", "expiresAt": i64::MAX / 1000, "usageLimit": 50.0, "currentUsage": 0.0 },
                    // 有效 bonus
                    { "status": "ACTIVE", "expiresAt": i64::MAX / 1000, "usageLimit": 30.0, "currentUsage": 10.0 }
                ]
            }]
        });
        let details = UsageDetails::from_usage_data(Some(&d)).unwrap();
        // total_limit = 100 (main) + 20 (trial) + 30 (bonus) + 50 (overage) = 200
        // total_usage = 30 + 5 + 10 = 45
        // remaining = 155
        assert!((details.remaining() - 155.0).abs() < 0.01);
    }

    #[test]
    fn details_ignores_inactive_trial() {
        let d = json!({
            "overageConfiguration": { "overageStatus": "DISABLED" },
            "usageBreakdownList": [{
                "usageLimit": 100.0,
                "currentUsage": 50.0,
                "freeTrialInfo": {
                    "freeTrialStatus": "EXPIRED",
                    "usageLimit": 100.0,
                    "currentUsage": 0.0
                }
            }]
        });
        let details = UsageDetails::from_usage_data(Some(&d)).unwrap();
        assert!((details.remaining() - 50.0).abs() < 0.01);
    }
}
