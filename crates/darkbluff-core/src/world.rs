//! 视角（World）枚举：共享叶子类型，`content` 与 `save` 层共用，避免存档层反向依赖内容层。

use serde::{Deserialize, Serialize};

/// 当前视角：表面世界（信息恒为真）或影子世界（信息恒为假）。
///
/// 序列化为小写字符串 `surface` / `shadow`，与数据文件、存档 JSON 一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum World {
    Surface,
    Shadow,
}

impl Default for World {
    /// 缺省为表面世界（新章节默认视角）。
    fn default() -> Self {
        World::Surface
    }
}

impl World {
    /// 切换到另一侧视角（`gaze` 的核心动作）。
    pub fn toggle(self) -> World {
        match self {
            World::Surface => World::Shadow,
            World::Shadow => World::Surface,
        }
    }

    /// 玩家可读的中文标签。
    pub fn label(self) -> &'static str {
        match self {
            World::Surface => "表面",
            World::Shadow => "影子",
        }
    }

    /// 英文标识（用于文件名、日志等），与序列化形式一致。
    pub fn as_str(self) -> &'static str {
        match self {
            World::Surface => "surface",
            World::Shadow => "shadow",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&World::Surface).unwrap(), "\"surface\"");
        assert_eq!(serde_json::to_string(&World::Shadow).unwrap(), "\"shadow\"");
        assert_eq!(
            serde_json::from_str::<World>("\"surface\"").unwrap(),
            World::Surface
        );
        assert_eq!(
            serde_json::from_str::<World>("\"shadow\"").unwrap(),
            World::Shadow
        );
    }

    #[test]
    fn world_toggle_and_label() {
        assert_eq!(World::Surface.toggle(), World::Shadow);
        assert_eq!(World::Shadow.toggle(), World::Surface);
        assert_eq!(World::Surface.label(), "表面");
        assert_eq!(World::Shadow.label(), "影子");
    }
}
