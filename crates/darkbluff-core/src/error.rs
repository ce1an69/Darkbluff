//! 跨层错误类型。
//!
//! 设计见 docs/architecture.md「错误处理与日志」：用 `thiserror` 定义领域错误枚举，
//! 跨层以 `Result<T, AppError>` 传播并保留错误类型。外部输入（玩家指令、存档文件、
//! 内容数据）的错误必须以 `Result` 返回并转为 UI 提示，**绝不 panic**。

use thiserror::Error;

/// 全局结果别名。
pub type Result<T> = std::result::Result<T, AppError>;

/// 领域错误枚举。
#[derive(Debug, Error)]
pub enum AppError {
    /// 内容层错误：加载失败、引用断裂、启动校验失败等。
    #[error("内容错误: {0}")]
    Content(String),

    /// 存档层错误：读写失败、结构损坏无法恢复等。
    #[error("存档错误: {0}")]
    Save(String),

    /// 指令层错误：指令无法执行（非可读提示类的内部错误）。
    #[error("指令错误: {0}")]
    Command(String),

    /// IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误。
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML 序列化/反序列化错误。
    #[error("YAML 错误: {0}")]
    Yaml(#[from] serde_yml::Error),

    /// 其他未分类错误。
    #[error("{0}")]
    Other(String),
}

impl AppError {
    /// 以字符串构造任意变体的便捷方法（用于在具体模块中携带上下文）。
    pub fn other(msg: impl Into<String>) -> Self {
        AppError::Other(msg.into())
    }
}
