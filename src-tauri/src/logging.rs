//! 日志记录模块
//! 
//! 本模块提供统一的日志记录功能，基于 Tauri 的日志插件和标准的 Rust 日志生态。
//! 支持结构化日志记录，可以将日志输出到文件、控制台和其他目标。
//! 
//! # 功能特性
//! 
//! - **统一接口**：提供简单易用的日志记录函数
//! - **结构化日志**：使用代码标识符进行日志分类
//! - **多级别支持**：支持 Info、Warn、Error 等不同级别的日志
//! - **Tauri 集成**：与 Tauri 应用程序框架无缝集成
//! - **性能优化**：异步日志记录，不阻塞主线程
//! 
//! # 使用示例
//! 
//! ```rust
//! use crate::logging;
//! 
//! // 记录信息级别日志
//! logging::info("REDIS_CONNECT", "Connected to Redis server");
//! 
//! // 记录警告级别日志
//! logging::warn("REDIS_RETRY", "Connection failed, retrying...");
//! 
//! // 记录错误级别日志
//! logging::error("DB_ERROR", "Failed to save configuration");
//! ```
//! 
//! # 日志级别说明
//! 
//! - **Info**: 一般信息，记录正常操作流程
//! - **Warn**: 警告信息，表示可能出现问题但不影响主要功能
//! - **Error**: 错误信息，表示操作失败或异常情况
//! 
//! # 日志标识符
//! 
//! 建议使用有意义的代码标识符，如：
//! - `REDIS_INIT`: Redis 初始化相关
//! - `REDIS_CONNECT`: Redis 连接相关
//! - `DB_QUERY`: 数据库查询相关
//! - `APP_START`: 应用程序启动相关
//! - `COMMAND_EXEC`: 命令执行相关

use log::LevelFilter;

/// 创建并配置 Tauri 日志插件
/// 
/// 返回一个配置好的 Tauri 日志插件实例，用于在 Tauri 应用程序中启用日志功能。
/// 
/// # 插件配置
/// 
/// - **日志级别**: Info 级别，记录 Info 及以上级别的日志
/// - **输出目标**: 默认输出到控制台和文件（Tauri 自动处理）
/// - **格式化**: 使用 Tauri 日志插件的默认格式
/// 
/// # 使用方法
/// 
/// 在 Tauri 应用程序的构建过程中注册插件：
/// 
/// ```rust
/// tauri::Builder::default()
///     .plugin(logging::plugin())
///     // ... 其他配置
///     .run(tauri::generate_context!())
///     .expect("error while running tauri application");
/// ```
/// 
/// # 自定义配置
/// 
/// 如果需要自定义日志级别或格式，可以修改此函数：
/// 
/// ```rust
/// pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
///     tauri_plugin_log::Builder::new()
///         .level(LevelFilter::Debug)  // 更详细的日志级别
///         .build()
/// }
/// ```
/// 
/// # 返回值
/// 
/// 返回配置好的 Tauri 插件实例。
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .level(LevelFilter::Info)
        .build()
}

/// 记录信息级别日志
/// 
/// 用于记录一般性的信息，如操作成功、状态变更、重要事件等。
/// 这是应用程序中最常用的日志级别。
/// 
/// # 参数
/// 
/// - `code`: 日志标识符，用于分类和过滤日志
/// - `message`: 日志消息内容
/// 
/// # 使用场景
/// 
/// - 应用程序启动和关闭
/// - 连接成功建立
/// - 配置加载完成
/// - 用户操作记录
/// - 重要状态变更
/// 
/// # 示例
/// 
/// ```rust
/// // 应用程序启动
/// logging::info("APP_START", "Tauri Redis Client started");
/// 
/// // Redis 连接成功
/// logging::info("REDIS_CONNECT", &format!("Connected to {}", url));
/// 
/// // 配置加载
/// logging::info("CONFIG_LOAD", "Loaded 3 Redis configurations");
/// ```
/// 
/// # 性能说明
/// 
/// 日志记录是异步的，不会阻塞主线程的执行。
/// 但是仍然建议避免在高频操作中记录过多的日志。
pub fn info(code: &str, message: &str) {
    log::info!(target: code, "{}", message);
}

/// 记录警告级别日志
/// 
/// 用于记录可能的问题或不寻常的情况，这些问题不会导致程序失败，
/// 但可能需要关注或处理。
/// 
/// # 参数
/// 
/// - `code`: 日志标识符，用于分类和过滤日志
/// - `message`: 日志消息内容
/// 
/// # 使用场景
/// 
/// - 连接重试
/// - 配置值异常但可处理
/// - 性能警告
/// - 即将废弃的功能使用
/// - 非关键操作失败
/// 
/// # 示例
/// 
/// ```rust
/// // 连接重试
/// logging::warn("REDIS_RETRY", &format!("Attempt {} failed", attempt));
/// 
/// // 配置警告
/// logging::warn("CONFIG_WARN", "Pool size is unusually large");
/// 
/// // 性能警告
/// logging::warn("PERF_WARN", "Query took longer than expected");
/// ```
pub fn warn(code: &str, message: &str) {
    log::warn!(target: code, "{}", message);
}

/// 记录错误级别日志
/// 
/// 用于记录错误信息和异常情况，这些错误可能会影响应用程序的功能。
/// 通常用于记录操作失败、异常捕获等问题。
/// 
/// # 参数
/// 
/// - `code`: 日志标识符，用于分类和过滤日志
/// - `message`: 日志消息内容
/// 
/// # 使用场景
/// 
/// - 连接失败
/// - 数据库操作错误
/// - 文件读写错误
/// - 网络请求失败
/// - 未预期的异常
/// 
/// # 示例
/// 
/// ```rust
/// // Redis 连接错误
/// logging::error("REDIS_ERROR", &format!("Failed to connect: {}", error));
/// 
/// // 数据库错误
/// logging::error("DB_ERROR", "Failed to save configuration");
/// 
/// // 一般错误
/// logging::error("GENERIC_ERROR", &format!("Operation failed: {}", e));
/// ```
/// 
/// # 错误处理
/// 
/// 记录错误日志不应该影响程序的正常流程。
/// 错误记录后，程序应该继续执行相应的错误处理逻辑。
/// 
/// # 调试建议
/// 
/// 当遇到错误时，应该：
/// 1. 记录详细的错误信息
/// 2. 包含相关的上下文数据
/// 3. 使用有意义的错误代码
/// 4. 考虑错误对用户的影响
pub fn error(code: &str, message: &str) {
    log::error!(target: code, "{}", message);
}

// # 日志最佳实践指南
// 
// ## 1. 日志级别选择
// 
// - **Info**: 记录正常流程中的重要事件
// - **Warn**: 记录可能的问题，但不影响核心功能
// - **Error**: 记录导致功能失败的问题
// 
// ## 2. 日志标识符规范
// 
// 使用统一的大写命名约定，建议的标识符类别：
// 
// - **REDIS_***: Redis 相关操作
//   - `REDIS_INIT`: 初始化
//   - `REDIS_CONNECT`: 连接操作
//   - `REDIS_RETRY`: 重试操作
//   - `REDIS_ERROR`: 错误情况
// 
// - **DB_***: 数据库相关操作
//   - `DB_QUERY`: 查询操作
//   - `DB_SAVE`: 保存操作
//   - `DB_ERROR`: 错误情况
// 
// - **APP_***: 应用程序相关
//   - `APP_START`: 应用启动
//   - `APP_STATE`: 状态变更
//   - `APP_ERROR`: 应用错误
// 
// - **COMMAND_***: 命令相关
//   - `COMMAND_EXEC`: 命令执行
//   - `COMMAND_RESULT`: 命令结果
// 
// ## 3. 消息内容建议
// 
// - **清晰简洁**: 避免冗余信息，突出重点
// - **包含上下文**: 提供足够的上下文信息
// - **避免敏感信息**: 不要记录密码、令牌等敏感数据
// - **使用格式化**: 对于动态数据使用适当的格式化
// 
// ## 4. 性能考虑
// 
// - **避免过度日志**: 不要在高频循环中记录大量日志
// - **异步记录**: 利用异步日志记录减少性能影响
// - **合理过滤**: 在生产环境中可能需要过滤某些日志
// 
// ## 5. 调试技巧
// 
// - **代码分类**: 使用不同的代码标识符分类日志
// - **时序记录**: 在关键操作前后记录日志，便于问题追踪
// - **错误上下文**: 在错误日志中包含足够的上下文信息
// 
// # 示例：完整的日志记录场景
// 
// ```rust
// use crate::logging;
// 
// async fn connect_to_redis(config: &RedisConfig) -> Result<RedisService> {
//     logging::info("REDIS_CONNECT", &format!("Attempting to connect to {:?}", config.urls));
//     
//     match RedisService::new(config.clone()).await {
//         Ok(service) => {
//             logging::info("REDIS_CONNECT", "Successfully connected to Redis");
//             Ok(service)
//         }
//         Err(e) => {
//             logging::error("REDIS_ERROR", &format!("Connection failed: {}", e));
//             
//             // 尝试重试逻辑
//             for attempt in 1..=3 {
//                 logging::warn("REDIS_RETRY", &format!("Retry attempt {}", attempt));
//                 // 重试逻辑...
//             }
//             
//             Err(e)
//         }
//     }
// }
// ```