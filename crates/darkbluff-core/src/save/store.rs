//! 存档读写编排：路径解析、加载（含损坏恢复）、原子保存、新游戏初始化、设置持久化。
//!
//! 设计见 docs/save-system.md「存档健壮性」「新游戏初始化」「设置文件」。本模块只负责
//! 文件 IO 层面的健壮性；存档引用内容的失效（线索/章节被删）由引擎层在建立会话时检测。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::save::atomic::{backup_then_atomic_write, bak_path, corrupt_path};
use crate::save::clock::Clock;
use crate::save::migration::migrate;
use crate::save::schema::{CURRENT_VERSION, Save, Settings};
use crate::save::snapshot::SnapshotStore;

/// 加载结果。
#[derive(Debug)]
pub enum LoadResult {
    /// 没有任何存档（save.json 与 .bak 均不存在）。
    None,
    /// 成功加载（可能含损坏恢复报告）。
    Save(Save, LoadReport),
    /// save.json 与 .bak 均损坏；已将损坏的 save.json 另存为 `.corrupt-<stamp>`。
    /// 上层应据此新建游戏（需要内容引擎提供首章）。
    BothCorrupt,
}

/// 加载过程中的文件级报告（供 UI 显示通知条）。
#[derive(Debug, Default, Clone)]
pub struct LoadReport {
    /// 从 `.bak` 恢复。
    pub recovered_from_bak: bool,
}

impl LoadReport {
    pub fn warning_messages(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.recovered_from_bak {
            out.push("存档文件损坏，已从备份恢复".into());
        }
        out
    }
}

/// 单存档存储：管理 `save.json`、`save.json.bak`、`settings.json` 与 `snapshots/`。
pub struct SaveStore {
    save_dir: PathBuf,
    save_path: PathBuf,
    settings_path: PathBuf,
    snapshots: SnapshotStore,
    clock: Box<dyn Clock>,
}

impl SaveStore {
    /// 打开（必要时创建）存档目录。
    pub fn open(save_dir: PathBuf, clock: Box<dyn Clock>) -> Result<Self> {
        fs::create_dir_all(&save_dir)?;
        let snapshots = SnapshotStore::new(&save_dir);
        Ok(Self {
            save_path: save_dir.join("save.json"),
            settings_path: save_dir.join("settings.json"),
            save_dir,
            snapshots,
            clock,
        })
    }

    /// 跨平台默认存档目录（`{data_dir}/darkbluff/save`）。
    pub fn default_dir() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("darkbluff").join("save"))
    }

    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }
    pub fn snapshots(&self) -> &SnapshotStore {
        &self.snapshots
    }
    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    /// 存档文件（或其 .bak）是否存在。
    pub fn has_save(&self) -> bool {
        self.save_path.exists() || bak_path(&self.save_path).exists()
    }

    fn read_save_file(&self, path: &Path) -> Result<Save> {
        let text = fs::read_to_string(path)?;
        let save: Save = serde_json::from_str(&text)?;
        Ok(save)
    }

    /// 加载存档（含损坏恢复）。返回 [`LoadResult`]。
    pub fn load(&self) -> Result<LoadResult> {
        let main = &self.save_path;
        let bak = bak_path(main);

        if main.exists() {
            match self.read_save_file(main) {
                Ok(mut save) => {
                    migrate(&mut save)?;
                    return Ok(LoadResult::Save(save, LoadReport::default()));
                }
                Err(_) => {
                    if bak.exists() {
                        if let Ok(mut save) = self.read_save_file(&bak) {
                            migrate(&mut save)?;
                            tracing::warn!("save.json 损坏，已从 .bak 恢复");
                            return Ok(LoadResult::Save(
                                save,
                                LoadReport {
                                    recovered_from_bak: true,
                                },
                            ));
                        }
                    }
                    // 均损坏 → 另存损坏文件，返回 BothCorrupt
                    let stamp = self.clock.now_stamp();
                    let _ = fs::rename(main, corrupt_path(main, &stamp));
                    tracing::warn!("save.json 与 .bak 均损坏，已将损坏文件另存并以新存档启动");
                    return Ok(LoadResult::BothCorrupt);
                }
            }
        } else if bak.exists() {
            if let Ok(mut save) = self.read_save_file(&bak) {
                migrate(&mut save)?;
                tracing::warn!("save.json 缺失，已从 .bak 恢复");
                return Ok(LoadResult::Save(
                    save,
                    LoadReport {
                        recovered_from_bak: true,
                    },
                ));
            }
        }
        Ok(LoadResult::None)
    }

    /// 原子保存（先备份 `.bak`，再写 `.tmp` → rename）。自动刷新时间戳与版本号。
    pub fn save(&self, save: &Save) -> Result<()> {
        let mut stamped = save.clone();
        stamped.version = CURRENT_VERSION;
        stamped.timestamp = self.clock.now_iso();
        let bytes = serde_json::to_vec_pretty(&stamped)?;
        backup_then_atomic_write(&self.save_path, &bytes)
    }

    /// 新游戏初始化：清空 `snapshots/`、删除 `.bak`、生成空存档并落盘。
    /// 首章的 `chapter_start` 检查点与 intro 展示由引擎层按时机创建。
    pub fn new_game(&self, first_chapter: &str, starting_scene: &str) -> Result<Save> {
        // 清空 snapshots
        let snap_dir = self.save_dir.join("snapshots");
        if snap_dir.exists() {
            let _ = fs::remove_dir_all(&snap_dir);
        }
        fs::create_dir_all(&snap_dir)?;
        // 删除旧备份
        let _ = fs::remove_file(bak_path(&self.save_path));

        let save = Save::new_game(first_chapter, starting_scene, self.clock.now_iso());
        self.save(&save)?;
        Ok(save)
    }

    /// 加载设置（损坏回退 `.bak`，均失败用默认值）。
    pub fn load_settings(&self) -> Result<Settings> {
        if let Ok(text) = fs::read_to_string(&self.settings_path) {
            if let Ok(s) = serde_json::from_str::<Settings>(&text) {
                return Ok(s);
            }
            let bak = bak_path(&self.settings_path);
            if bak.exists() {
                if let Ok(t) = fs::read_to_string(&bak) {
                    if let Ok(s) = serde_json::from_str(&t) {
                        return Ok(s);
                    }
                }
            }
        }
        Ok(Settings::default())
    }

    /// 原子保存设置。
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(settings)?;
        backup_then_atomic_write(&self.settings_path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::clock::FakeClock;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "darkbluff-store-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn store(tag: &str) -> SaveStore {
        SaveStore::open(temp_dir(tag), Box::new(FakeClock::new())).unwrap()
    }

    #[test]
    fn load_none_when_empty() {
        let s = store("none");
        assert!(matches!(s.load().unwrap(), LoadResult::None));
    }

    #[test]
    fn save_then_load_roundtrip() {
        let s = store("roundtrip");
        let mut save = Save::new_game("c1", "tavern", "t".into());
        save.clues_mut("c1").push("x".into());
        s.save(&save).unwrap();
        match s.load().unwrap() {
            LoadResult::Save(loaded, _) => {
                assert_eq!(loaded.current_chapter, "c1");
                assert!(loaded.has_clue("c1", "x"));
            }
            _ => panic!("expected save"),
        }
    }

    #[test]
    fn corrupt_main_recovers_from_bak() {
        let s = store("bak");
        let save = Save::new_game("c1", "tavern", "t".into());
        s.save(&save).unwrap();
        // 再保存一次以生成 .bak（内容 c1），然后写损坏 save.json
        let save2 = Save::new_game("c1", "market", "t2".into());
        s.save(&save2).unwrap(); // .bak 现在是 c1/tavern，save.json 是 c1/market
        std::fs::write(&s.save_path, "{ not json").unwrap();
        match s.load().unwrap() {
            LoadResult::Save(loaded, report) => {
                assert!(report.recovered_from_bak);
                // 从 .bak 恢复 → tavern
                assert_eq!(loaded.current_scene, "tavern");
            }
            _ => panic!("expected recovery from bak"),
        }
    }

    #[test]
    fn both_corrupt_returns_signal_and_side_lines_file() {
        let s = store("both");
        std::fs::write(&s.save_path, "{ broken").unwrap();
        std::fs::write(bak_path(&s.save_path), "{ also broken").unwrap();
        match s.load().unwrap() {
            LoadResult::BothCorrupt => {}
            _ => panic!("expected BothCorrupt"),
        }
        // save.json 已被改名为 .corrupt-*
        assert!(!s.save_path.exists());
        let mut found_corrupt = false;
        for e in std::fs::read_dir(s.save_dir()).unwrap() {
            let n = e.unwrap().file_name().to_string_lossy().into_owned();
            if n.starts_with("save.json.corrupt-") {
                found_corrupt = true;
            }
        }
        assert!(found_corrupt);
    }

    #[test]
    fn new_game_clears_snapshots_and_bak() {
        let s = store("newgame");
        // 预置一个 snapshot 与 bak
        std::fs::create_dir_all(s.save_dir.join("snapshots")).unwrap();
        std::fs::write(s.save_dir.join("snapshots/old.md"), "x").unwrap();
        std::fs::write(bak_path(&s.save_path), "oldbak").unwrap();

        let save = s.new_game("c1", "tavern").unwrap();
        assert_eq!(save.current_chapter, "c1");
        assert!(s.snapshots().list_all().unwrap().is_empty());
        assert!(!bak_path(&s.save_path).exists());
        assert!(s.save_path.exists());
    }

    #[test]
    fn settings_roundtrip_and_default() {
        let s = store("settings");
        assert_eq!(
            s.load_settings().unwrap().motion,
            crate::save::schema::Motion::Full
        );
        let settings = Settings {
            motion: crate::save::schema::Motion::Off,
            ..Settings::default()
        };
        s.save_settings(&settings).unwrap();
        assert_eq!(
            s.load_settings().unwrap().motion,
            crate::save::schema::Motion::Off
        );
    }
}
