//! 统一错误码系统
//!
//! 提供跨平台的错误码定义，用于 Rust 和 JavaScript 之间的错误通信。
//!
//! # 错误码范围
//!
//! - **1000-1999**: 通用错误
//! - **2000-2999**: 插件错误
//! - **3000-3999**: 配置错误
//! - **4000-4999**: 网络错误
//! - **5000-5999**: 文件系统错误
//!
//! # 示例
//!
//! ```rust
//! use i::error::{ErrorCode, AppError};
//!
//! let error = AppError::new(ErrorCode::PluginNotFound, "Plugin 'logger' not found");
//! println!("Error {}: {}", error.code(), error.message());
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// 错误码枚举
///
/// 所有错误码都映射到一个唯一的整数值，便于跨平台传输。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum ErrorCode {
    // === 通用错误 (1000-1999) ===
    /// 未知错误
    Unknown = 1000,
    /// 无效参数
    InvalidArgument = 1001,
    /// 资源未找到
    NotFound = 1002,
    /// 资源已存在
    AlreadyExists = 1003,
    /// 权限不足
    PermissionDenied = 1004,
    /// 操作超时
    Timeout = 1005,
    /// 已取消
    Cancelled = 1006,
    /// 未实现
    NotImplemented = 1007,
    /// 内部错误
    Internal = 1008,

    // === 插件错误 (2000-2999) ===
    /// 插件未找到
    PluginNotFound = 2000,
    /// 插件加载失败
    PluginLoadFailed = 2001,
    /// 插件依赖缺失
    PluginDependencyMissing = 2002,
    /// 插件版本不兼容
    PluginVersionIncompatible = 2003,
    /// 插件初始化失败
    PluginInitFailed = 2004,
    /// 插件卸载失败
    PluginUnloadFailed = 2005,
    /// 插件已经加载
    PluginAlreadyLoaded = 2006,
    /// 插件热重载失败
    PluginReloadFailed = 2007,

    // === 配置错误 (3000-3999) ===
    /// 配置无效
    ConfigInvalid = 3000,
    /// 配置缺失
    ConfigMissing = 3001,
    /// 配置解析失败
    ConfigParseFailed = 3002,
    /// 配置键未找到
    ConfigKeyNotFound = 3003,
    /// 配置类型错误
    ConfigTypeMismatch = 3004,

    // === 网络错误 (4000-4999) ===
    /// 连接失败
    ConnectionFailed = 4000,
    /// 连接超时
    ConnectionTimeout = 4001,
    /// 请求失败
    RequestFailed = 4002,
    /// 响应无效
    InvalidResponse = 4003,
    /// DNS 解析失败
    DnsResolutionFailed = 4004,
    /// SSL/TLS 错误
    TlsError = 4005,

    // === 文件系统错误 (5000-5999) ===
    /// 文件未找到
    FileNotFound = 5000,
    /// 文件已存在
    FileAlreadyExists = 5001,
    /// 目录未找到
    DirectoryNotFound = 5002,
    /// 路径无效
    InvalidPath = 5003,
    /// 读取失败
    ReadFailed = 5004,
    /// 写入失败
    WriteFailed = 5005,
    /// 删除失败
    DeleteFailed = 5006,
    /// 权限不足
    FilePermissionDenied = 5007,
}

impl ErrorCode {
    /// 从整数值创建错误码
    pub fn from_u32(value: u32) -> Self {
        match value {
            1000 => ErrorCode::Unknown,
            1001 => ErrorCode::InvalidArgument,
            1002 => ErrorCode::NotFound,
            1003 => ErrorCode::AlreadyExists,
            1004 => ErrorCode::PermissionDenied,
            1005 => ErrorCode::Timeout,
            1006 => ErrorCode::Cancelled,
            1007 => ErrorCode::NotImplemented,
            1008 => ErrorCode::Internal,

            2000 => ErrorCode::PluginNotFound,
            2001 => ErrorCode::PluginLoadFailed,
            2002 => ErrorCode::PluginDependencyMissing,
            2003 => ErrorCode::PluginVersionIncompatible,
            2004 => ErrorCode::PluginInitFailed,
            2005 => ErrorCode::PluginUnloadFailed,
            2006 => ErrorCode::PluginAlreadyLoaded,
            2007 => ErrorCode::PluginReloadFailed,

            3000 => ErrorCode::ConfigInvalid,
            3001 => ErrorCode::ConfigMissing,
            3002 => ErrorCode::ConfigParseFailed,
            3003 => ErrorCode::ConfigKeyNotFound,
            3004 => ErrorCode::ConfigTypeMismatch,

            4000 => ErrorCode::ConnectionFailed,
            4001 => ErrorCode::ConnectionTimeout,
            4002 => ErrorCode::RequestFailed,
            4003 => ErrorCode::InvalidResponse,
            4004 => ErrorCode::DnsResolutionFailed,
            4005 => ErrorCode::TlsError,

            5000 => ErrorCode::FileNotFound,
            5001 => ErrorCode::FileAlreadyExists,
            5002 => ErrorCode::DirectoryNotFound,
            5003 => ErrorCode::InvalidPath,
            5004 => ErrorCode::ReadFailed,
            5005 => ErrorCode::WriteFailed,
            5006 => ErrorCode::DeleteFailed,
            5007 => ErrorCode::FilePermissionDenied,

            _ => ErrorCode::Unknown,
        }
    }

    /// 转换为整数值
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    /// 获取错误码的描述
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCode::Unknown => "未知错误",
            ErrorCode::InvalidArgument => "无效参数",
            ErrorCode::NotFound => "资源未找到",
            ErrorCode::AlreadyExists => "资源已存在",
            ErrorCode::PermissionDenied => "权限不足",
            ErrorCode::Timeout => "操作超时",
            ErrorCode::Cancelled => "已取消",
            ErrorCode::NotImplemented => "未实现",
            ErrorCode::Internal => "内部错误",

            ErrorCode::PluginNotFound => "插件未找到",
            ErrorCode::PluginLoadFailed => "插件加载失败",
            ErrorCode::PluginDependencyMissing => "插件依赖缺失",
            ErrorCode::PluginVersionIncompatible => "插件版本不兼容",
            ErrorCode::PluginInitFailed => "插件初始化失败",
            ErrorCode::PluginUnloadFailed => "插件卸载失败",
            ErrorCode::PluginAlreadyLoaded => "插件已加载",
            ErrorCode::PluginReloadFailed => "插件热重载失败",

            ErrorCode::ConfigInvalid => "配置无效",
            ErrorCode::ConfigMissing => "配置缺失",
            ErrorCode::ConfigParseFailed => "配置解析失败",
            ErrorCode::ConfigKeyNotFound => "配置键未找到",
            ErrorCode::ConfigTypeMismatch => "配置类型错误",

            ErrorCode::ConnectionFailed => "连接失败",
            ErrorCode::ConnectionTimeout => "连接超时",
            ErrorCode::RequestFailed => "请求失败",
            ErrorCode::InvalidResponse => "响应无效",
            ErrorCode::DnsResolutionFailed => "DNS 解析失败",
            ErrorCode::TlsError => "SSL/TLS 错误",

            ErrorCode::FileNotFound => "文件未找到",
            ErrorCode::FileAlreadyExists => "文件已存在",
            ErrorCode::DirectoryNotFound => "目录未找到",
            ErrorCode::InvalidPath => "路径无效",
            ErrorCode::ReadFailed => "读取失败",
            ErrorCode::WriteFailed => "写入失败",
            ErrorCode::DeleteFailed => "删除失败",
            ErrorCode::FilePermissionDenied => "文件权限不足",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.description(), self.as_u32())
    }
}

/// 应用程序错误
///
/// 统一的错误类型，包含错误码和详细信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    /// 错误码
    #[serde(with = "error_code_serde")]
    code: ErrorCode,
    /// 错误消息
    message: String,
    /// 可选的上下文信息
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
}

/// 自定义序列化模块，确保 ErrorCode 序列化为整数
mod error_code_serde {
    use super::ErrorCode;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(code: &ErrorCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(code.as_u32())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ErrorCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Ok(ErrorCode::from_u32(value))
    }
}

impl AppError {
    /// 创建新的错误
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), context: None }
    }

    /// 创建带上下文的错误
    pub fn with_context(code: ErrorCode, message: impl Into<String>, context: impl Into<String>) -> Self {
        Self { code, message: message.into(), context: Some(context.into()) }
    }

    /// 获取错误码
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// 获取错误消息
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 获取上下文信息
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// 转换为 anyhow::Error
    pub fn into_anyhow(self) -> anyhow::Error {
        anyhow::anyhow!("{}", self)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(ctx) = &self.context {
            write!(f, " ({})", ctx)?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_conversion() {
        let code = ErrorCode::PluginNotFound;
        assert_eq!(code.as_u32(), 2000);
        assert_eq!(ErrorCode::from_u32(2000), code);
    }

    #[test]
    fn test_error_code_display() {
        let code = ErrorCode::NotFound;
        assert_eq!(format!("{}", code), "资源未找到 (1002)");
    }

    #[test]
    fn test_app_error_creation() {
        let err = AppError::new(ErrorCode::NotFound, "File 'test.txt' not found");
        assert_eq!(err.code(), ErrorCode::NotFound);
        assert_eq!(err.message(), "File 'test.txt' not found");
        assert!(err.context().is_none());
    }

    #[test]
    fn test_app_error_with_context() {
        let err = AppError::with_context(ErrorCode::PluginLoadFailed, "Failed to load plugin", "plugin=my_plugin");
        assert_eq!(err.code(), ErrorCode::PluginLoadFailed);
        assert_eq!(err.context(), Some("plugin=my_plugin"));
    }

    #[test]
    fn test_error_serialization() {
        let err = AppError::new(ErrorCode::NotFound, "Not found");
        let json = serde_json::to_string(&err).unwrap();
        // ErrorCode 现在序列化为整数
        assert!(json.contains(r#""code":1002"#));
        assert!(json.contains(r#""message":"Not found""#));
    }
}
