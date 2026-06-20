//! 原子写入与备份。
//!
//! 设计见 docs/save-system.md「存档健壮性」。保存时先写 `*.tmp` 再 rename，避免写入
//! 中断损坏；覆盖前将当前文件备份为 `*.bak`；损坏文件另存为 `*.corrupt-<suffix>`。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// 临时文件后缀。
pub const TMP_SUFFIX: &str = ".tmp";
/// 备份文件后缀。
pub const BAK_SUFFIX: &str = ".bak";

/// `.tmp` 临时文件路径。
pub fn tmp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(TMP_SUFFIX);
    PathBuf::from(s)
}

/// `.bak` 备份文件路径。
pub fn bak_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(BAK_SUFFIX);
    PathBuf::from(s)
}

/// 损坏文件另存路径：`{path}.corrupt-{suffix}`。
pub fn corrupt_path(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".corrupt-{suffix}"));
    PathBuf::from(s)
}

/// 原子写入字节：写 `.tmp` → 同步 → rename 覆盖目标（同文件系统下 rename 原子）。
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = tmp_path(path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// 原子写入字节（无 fsync）：写 `.tmp` → rename 覆盖目标。
/// 供持久化要求弱、写入频率高的数据（如设置）使用：省去 `sync_all`，崩溃最坏
/// 丢失最近一次改动，由加载侧 fallback 兜底；rename 仍保证不会写出半截文件。
pub fn atomic_write_bytes_nofsync(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = tmp_path(path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// 若目标文件存在，将其复制为 `.bak`（覆盖旧备份）。不存在则无操作。
pub fn backup_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        let bak = bak_path(path);
        fs::copy(path, &bak)?;
    }
    Ok(())
}

/// 备份后原子写入文本（典型存档/设置保存流程）。
pub fn backup_then_atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    backup_if_exists(path)?;
    atomic_write_bytes(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("darkbluff-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        dir.join(name)
    }

    #[test]
    fn atomic_write_creates_file() {
        let p = temp("atomic_create.json");
        let _ = std::fs::remove_file(&p);
        atomic_write_bytes(&p, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "hello");
        // tmp 已被 rename 消费
        assert!(!tmp_path(&p).exists());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn backup_then_atomic_write_keeps_bak() {
        let p = temp("atomic_backup.json");
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(bak_path(&p));
        atomic_write_bytes(&p, b"v1").unwrap();
        backup_then_atomic_write(&p, b"v2").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v2");
        assert_eq!(std::fs::read_to_string(bak_path(&p)).unwrap(), "v1");
        std::fs::remove_file(&p).ok();
        std::fs::remove_file(bak_path(&p)).ok();
    }

    #[test]
    fn corrupt_path_format() {
        let p = Path::new("save/save.json");
        assert_eq!(
            corrupt_path(p, "20260616T120000Z"),
            Path::new("save/save.json.corrupt-20260616T120000Z")
        );
    }
}
