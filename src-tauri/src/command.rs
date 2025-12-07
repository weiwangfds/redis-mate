//! 命令处理和响应格式模块
//! 
//! 本模块定义了应用程序中命令处理的标准响应格式和错误处理机制。
//! 所有从前端调用的命令都应使用统一的响应格式，确保前端能够一致地处理结果。
//! 
//! # 设计原则
//! 
//! - **统一性**：所有命令使用相同的响应格式
//! - **类型安全**：使用泛型支持不同类型的数据载荷
//! - **错误友好**：清晰的错误码和错误消息
//! - **扩展性**：支持添加额外的元数据字段
//! 
//! # 使用示例
//! 
//! ```rust
//! use crate::command::{CommandResponse, CommandResult};
//! 
//! // 成功响应
//! fn get_data() -> CommandResult<String> {
//!     Ok(CommandResponse::ok("Hello, World!".to_string()))
//! }
//! 
//! // 错误响应
//! fn process_data() -> CommandResult<i32> {
//!     CommandResponse::err("VALIDATION_ERROR", "Invalid input format")
//! }
//! ```

/// 标准命令响应结构
/// 
/// 所有 Tauri 命令处理器的返回值都应使用此结构，
/// 确保前端能够以统一的方式处理响应结果。
/// 
/// # 字段说明
/// 
/// - `success`: 操作是否成功，`true` 表示成功，`false` 表示失败
/// - `code`: 响应代码，成功时通常为 "OK"，失败时为错误代码
/// - `message`: 响应消息，成功时通常为空字符串，失败时为错误描述
/// - `data`: 实际的数据载荷，使用 `Option<T>` 类型，失败时为 `None`
/// 
/// # 泛型参数
/// 
/// - `T`: 数据载荷的类型，支持任意可序列化的类型
/// 
/// # 序列化
/// 
/// 实现了 `Serialize` trait，可以自动序列化为 JSON 格式发送给前端。
/// 
/// # 前端处理建议
/// 
/// ```javascript
/// // TypeScript 示例
/// interface CommandResponse<T> {
///   success: boolean;
///   code: string;
///   message: string;
///   data: T | null;
/// }
/// 
/// function handleResponse<T>(response: CommandResponse<T>) {
///   if (response.success) {
///     console.log('Success:', response.data);
///   } else {
///     console.error('Error:', response.code, response.message);
///   }
/// }
/// ```
#[derive(serde::Serialize)]
pub struct CommandResponse<T> {
    /// 操作成功标志
    /// 
    /// - `true`: 操作成功完成
    /// - `false`: 操作失败，应检查 `code` 和 `message` 字段
    pub success: bool,
    
    /// 响应代码
    /// 
    /// 成功时通常为 "OK"，失败时为具体的错误代码，如：
    /// - "VALIDATION_ERROR": 输入验证失败
    /// - "NOT_FOUND": 资源未找到
    /// - "PERMISSION_DENIED": 权限不足
    /// - "INTERNAL_ERROR": 内部服务器错误
    /// - "NETWORK_ERROR": 网络连接错误
    pub code: String,
    
    /// 响应消息
    /// 
    /// 成功时通常为空字符串，失败时提供详细的错误描述信息。
    /// 消息应该对用户友好，便于理解问题原因。
    pub message: String,
    
    /// 数据载荷
    /// 
    /// 包含操作的实际结果数据。使用 `Option<T>` 类型：
    /// - 成功时：`Some(data)` 包含实际数据
    /// - 失败时：`None` 表示无数据可返回
    pub data: Option<T>,
}

impl<T> CommandResponse<T> {
    /// 创建成功响应
    /// 
    /// 创建一个表示操作成功的响应对象，包含提供的数据载荷。
    /// 
    /// # 参数
    /// 
    /// - `data`: 要返回的数据载荷
    /// 
    /// # 返回值
    /// 
    /// 返回一个 `CommandResponse<T>` 实例，其中：
    /// - `success`: `true`
    /// - `code`: `"OK"`
    /// - `message`: 空字符串
    /// - `data`: `Some(data)`
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use crate::command::CommandResponse;
    /// 
    /// // 返回字符串数据
    /// let response = CommandResponse::ok("Hello, World!".to_string());
    /// 
    /// // 返回复杂数据结构
    /// let user = User { id: 1, name: "Alice" };
    /// let response = CommandResponse::ok(user);
    /// ```
    pub fn ok(data: T) -> Self {
        Self { 
            success: true, 
            code: "OK".into(), 
            message: String::new(), 
            data: Some(data) 
        }
    }

    /// 创建错误响应
    /// 
    /// 创建一个表示操作失败的响应对象，包含错误代码和描述信息。
    /// 
    /// # 参数
    /// 
    /// - `code`: 错误代码，用于程序化处理
    /// - `message`: 错误消息，用于显示给用户
    /// 
    /// # 返回值
    /// 
    /// 返回一个 `CommandResponse<T>` 实例，其中：
    /// - `success`: `false`
    /// - `code`: 提供的错误代码
    /// - `message`: 提供的错误消息
    /// - `data`: `None`
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use crate::command::CommandResponse;
    /// 
    /// // 简单错误
    /// let response: CommandResponse<String> = 
    ///     CommandResponse::err("NOT_FOUND", "User not found");
    /// 
    /// // 带验证错误的响应
    /// let response: CommandResponse<User> = 
    ///     CommandResponse::err("VALIDATION_ERROR", "Invalid email format");
    /// ```
    pub fn err(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { 
            success: false, 
            code: code.into(), 
            message: message.into(), 
            data: None 
        }
    }
}


pub type CommandResult<T> = anyhow::Result<CommandResponse<T>>;