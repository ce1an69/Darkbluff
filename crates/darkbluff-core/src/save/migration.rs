//! 存档版本迁移。
//!
//! 设计见 docs/save-system.md「兼容性策略 → 存档结构升级」。当前为 v1，无迁移逻辑；
//! 后续结构升级时在此按 version 递增执行迁移函数，并提升 [`CURRENT_VERSION`]。

use crate::error::Result;
use crate::save::schema::{CURRENT_VERSION, Save};

/// 对加载后的存档执行版本迁移，最终对齐到 [`CURRENT_VERSION`]。
///
/// v1：仅校验/补齐版本号，不做结构变换。
pub fn migrate(save: &mut Save) -> Result<()> {
    // 预留：match save.version { 0 => { ...; save.version = 1; } _ => {} }
    if save.version > CURRENT_VERSION {
        // 来自更高版本，向前兼容降级：保留数据但标记为当前版本（未知字段在反序列化时已忽略）
        save.version = CURRENT_VERSION;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_v1_noop() {
        let mut save = Save::default();
        let version = save.version;
        migrate(&mut save).unwrap();
        assert_eq!(save.version, version);
        assert_eq!(save.version, CURRENT_VERSION);
    }
}
