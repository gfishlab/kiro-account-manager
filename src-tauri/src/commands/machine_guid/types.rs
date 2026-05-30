// 机器码相关数据类型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMachineInfo {
    pub machine_guid: Option<String>,
    pub os_type: String,
    pub can_modify: bool,
    pub requires_admin: bool,
}

