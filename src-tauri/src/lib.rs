//! Tauri Redis 客户端应用程序库模块
//! 
//! 本模块包含了应用程序的核心业务逻辑，包括：
//! - 命令处理：处理来自前端界面的调用请求
//! - 日志记录：统一的日志记录功能
//! - Redis 服务：Redis 连接和操作封装
//! - 数据库管理：SQLite 数据库操作
//! - 应用状态：全局状态管理

// 模块声明
pub mod command;      // 命令处理和响应格式
pub mod logging;      // 日志记录插件和工具
pub mod redis_service; // Redis 服务封装
pub mod db;          // 数据库管理
pub mod app_state;   // 应用程序状态管理

// 导入必要的类型和函数
use command::{CommandResponse, CommandResult};
use app_state::AppState;
use tauri::Manager;
use tauri::Emitter;
use crate::redis_service::{RedisConfig, ClusterNodeInfo};
use tauri::ipc::InvokeError;
use serde::Serialize;

/// 健康检查命令处理器
/// 
/// 提供简单的应用程序健康状态检查功能，用于验证后端服务是否正常运行。
/// 
/// # 返回值
/// 
/// 返回包含 "ok" 状态的 `CommandResponse<String>`，表示应用程序运行正常。
/// 
/// # 错误处理
/// 
/// 将任何内部错误转换为 Tauri 的 `InvokeError` 类型，以便前端能够正确处理。
#[tauri::command]
fn health_check() -> Result<CommandResponse<String>, tauri::ipc::InvokeError> {
    // 内部健康检查逻辑
    fn inner() -> CommandResult<String> {
        // 记录健康检查日志
        logging::info("HEALTH", "ok");
        // 返回成功响应
        Ok(CommandResponse::ok("ok".to_string()))
    }
    
    // 将 anyhow 错误转换为 Tauri 的 InvokeError
    inner().map_err(tauri::ipc::InvokeError::from_anyhow)
}

#[derive(Serialize)]
struct ConfigItem {
    name: String,
    config: RedisConfig,
}

/// 列出所有已保存的 Redis 配置（来自数据库）
/// 
/// 该命令从 SQLite 数据库中查询所有已保存的配置项。
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<Vec<ConfigItem>>`，其中 `ConfigItem` 包含：
/// - `name`: 配置的唯一名称
/// - `config`: `RedisConfig` 对象，包含详细连接参数
/// 
/// # 前端示例
/// 
/// ```ts
/// const configs = await listConfigs();
/// console.log('Saved configs:', configs);
/// ```
#[tauri::command]
async fn list_configs(state: tauri::State<'_, AppState>) -> Result<CommandResponse<Vec<ConfigItem>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>) -> CommandResult<Vec<ConfigItem>> {
        let rows = state.db.list_configs().await?;
        let items = rows.into_iter().map(|(name, config)| ConfigItem { name, config }).collect();
        Ok(CommandResponse::ok(items))
    }
    inner(state).await.map_err(InvokeError::from_anyhow)
}

/// 获取指定名称的 Redis 配置
/// 
/// # 参数
/// 
/// - `name`: 配置名称（唯一标识符）
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<Option<RedisConfig>>`：
/// - 如果找到配置，`data` 字段为 `RedisConfig` 对象
/// - 如果未找到，`data` 字段为 `null` (None)
/// 
/// # 前端示例
/// 
/// ```ts
/// const config = await getConfig('production-db');
/// if (config) {
///   // 使用配置...
/// }
/// ```
#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>, name: String) -> Result<CommandResponse<Option<RedisConfig>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String) -> CommandResult<Option<RedisConfig>> {
        let cfg = state.db.get_config(&name).await?;
        Ok(CommandResponse::ok(cfg))
    }
    inner(state, name).await.map_err(InvokeError::from_anyhow)
}

/// 保存（新增或更新）Redis 配置到数据库
/// 
/// 如果指定名称的配置已存在，则更新；否则创建新配置。
/// 
/// # 参数
/// 
/// - `name`: 配置名称（唯一标识符）
/// - `config`: `RedisConfig` 对象
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`，成功时 `data` 为 `true`。
/// 
/// # 前端示例
/// 
/// ```ts
/// await saveConfig('local', { 
///   urls: ['redis://127.0.0.1:6379'],
///   pool_size: 20 
/// });
/// ```
#[tauri::command]
async fn save_config(state: tauri::State<'_, AppState>, name: String, config: RedisConfig) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, config: RedisConfig) -> CommandResult<bool> {
        state.db.save_config(&name, &config).await?;
        Ok(CommandResponse::ok(true))
    }
    inner(state, name, config).await.map_err(InvokeError::from_anyhow)
}

/// 删除指定名称的 Redis 配置
/// 
/// 仅从数据库中删除配置记录，**不会**影响当前内存中已运行的服务实例。
/// 如需停止服务，请使用 `remove_connection`。
/// 
/// # 参数
/// 
/// - `name`: 要删除的配置名称
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`，删除成功时 `data` 为 `true`。
/// 
/// # 前端示例
/// 
/// ```ts
/// await deleteConfig('old-config');
/// ```
#[tauri::command]
async fn delete_config(state: tauri::State<'_, AppState>, name: String) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String) -> CommandResult<bool> {
        let ok = state.db.delete_config(&name).await?;
        Ok(CommandResponse::ok(ok))
    }
    inner(state, name).await.map_err(InvokeError::from_anyhow)
}

/// 列出当前内存中的所有服务连接名称
/// 
/// 返回当前 `AppState` 中已初始化并运行的 Redis 服务实例名称列表。
/// 这些服务可以直接用于执行 Redis 命令。
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<Vec<String>>`，包含所有活跃服务的名称。
/// 
/// # 前端示例
/// 
/// ```ts
/// const activeServices = await listServices();
/// console.log('Active connections:', activeServices);
/// ```
#[tauri::command]
async fn list_services(state: tauri::State<'_, AppState>) -> Result<CommandResponse<Vec<String>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>) -> CommandResult<Vec<String>> {
        let map = state.services.read().await;
        let names = map.keys().cloned().collect::<Vec<_>>();
        Ok(CommandResponse::ok(names))
    }
    inner(state).await.map_err(InvokeError::from_anyhow)
}

/// 从数据库重载所有连接到内存
/// 
/// 执行全量重载操作：
/// 1. 清空当前内存中的所有服务实例（断开现有连接）
/// 2. 从数据库读取所有配置
/// 3. 重新建立连接并初始化服务
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<String>`，成功时返回 "ok"。
/// 
/// # 前端示例
/// 
/// ```ts
/// await reloadServices();
/// ```
#[tauri::command]
async fn reload_services(state: tauri::State<'_, AppState>) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>) -> CommandResult<String> {
        state.reload_from_db().await?;
        Ok(CommandResponse::ok("ok".to_string()))
    }
    inner(state).await.map_err(InvokeError::from_anyhow)
}

/// 检查指定服务是否存在于内存映射
/// 
/// 快速检查某个连接是否已建立并可用。
/// 
/// # 参数
/// 
/// - `name`: 服务名称
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`，存在时为 `true`。
#[tauri::command]
async fn service_exists(state: tauri::State<'_, AppState>, name: String) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String) -> CommandResult<bool> {
        let map = state.services.read().await;
        Ok(CommandResponse::ok(map.contains_key(&name)))
    }
    inner(state, name).await.map_err(InvokeError::from_anyhow)
}

/// 添加新的 Redis 连接配置并建立服务实例
/// 
/// 支持以下模式：
/// - **单机**: 仅提供 `urls`（单个）
/// - **集群**: 设置 `cluster: true` 并提供种子节点 `urls`
/// - **哨兵**: 设置 `sentinel: true`，提供 `sentinel_master_name` 和 `sentinel_urls`
/// 
/// 参数：
/// - `name`: 连接名称（唯一标识）
/// - `config`: 后端 `RedisConfig`，包含地址、模式、重试参数等
/// 
/// 返回：`CommandResponse<String>`，成功返回 `"added"`
/// 
/// 前端示例：
/// ```ts
/// await addConnection('local', { urls: ['redis://127.0.0.1:6379'] })
/// ```
#[tauri::command]
async fn add_connection(state: tauri::State<'_, AppState>, name: String, config: RedisConfig) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, config: RedisConfig) -> CommandResult<String> {
        state.add_connection(&name, config).await?;
        Ok(CommandResponse::ok("added".to_string()))
    }
    inner(state, name, config).await.map_err(InvokeError::from_anyhow)
}

/// 删除已保存的 Redis 连接配置并移除服务实例
/// 
/// 参数：
/// - `name`: 连接名称
/// 
/// 返回：`CommandResponse<String>`，成功返回 `"removed"`
#[tauri::command]
async fn remove_connection(state: tauri::State<'_, AppState>, name: String) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String) -> CommandResult<String> {
        state.remove_connection(&name).await?;
        Ok(CommandResponse::ok("removed".to_string()))
    }
    inner(state, name).await.map_err(InvokeError::from_anyhow)
}

/// 对指定连接执行健康检查（`PING`）
/// 
/// 参数：
/// - `name`: 连接名称
/// 
/// 返回：`CommandResponse<String>`，成功返回 `"ok"`
#[tauri::command]
async fn check_connection(state: tauri::State<'_, AppState>, name: String) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String) -> CommandResult<String> {
        if let Some(svc) = state.get_service(&name).await {
            svc.check_health().await?;
            Ok(CommandResponse::ok("ok".to_string()))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name).await.map_err(InvokeError::from_anyhow)
}

/// 读取键值（`GET`），返回 `Option<String>`
/// 
/// 参数：
/// - `name`: 连接名称
/// - `key`: 键名
/// 
/// 返回：`CommandResponse<Option<String>>`
#[tauri::command]
async fn get_value(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<Option<String>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<Option<String>> {
        if let Some(svc) = state.get_service(&name).await {
            let v: Option<String> = svc.get(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(v))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

/// 设置键值（`SET`），可选过期时间（秒）
/// 
/// 参数：
/// - `name`: 连接名称
/// - `key`: 键名
/// - `value`: 字符串值
/// - `expire_seconds`: 过期时间（秒，可选）
/// 
/// 返回：`CommandResponse<bool>`，成功 `true`
#[tauri::command]
async fn set_value(state: tauri::State<'_, AppState>, name: String, key: String, value: String, expire_seconds: Option<u64>, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, value: String, expire_seconds: Option<u64>, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            svc.set(db.unwrap_or(0), &key, value, expire_seconds).await?;
            Ok(CommandResponse::ok(true))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, value, expire_seconds, db).await.map_err(InvokeError::from_anyhow)
}

/// 删除键（`DEL`）
/// 
/// 参数：
/// - `name`: 连接名称
/// - `key`: 键名
/// 
/// 返回：`CommandResponse<bool>`，存在且删除成功为 `true`
#[tauri::command]
async fn del_key(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.del(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

/// 批量读取（`MGET`），返回 `Vec<Option<String>>`
/// 
/// 参数：
/// - `name`: 连接名称
/// - `keys`: 键名数组
/// 
/// 返回：`CommandResponse<Vec<Option<String>>>`
#[tauri::command]
async fn mget_values(state: tauri::State<'_, AppState>, name: String, keys: Vec<String>) -> Result<CommandResponse<Vec<Option<String>>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, keys: Vec<String>) -> CommandResult<Vec<Option<String>>> {
        if let Some(svc) = state.get_service(&name).await {
            let v: Vec<Option<String>> = svc.mget(&keys).await?;
            Ok(CommandResponse::ok(v))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, keys).await.map_err(InvokeError::from_anyhow)
}

/// 批量写入（`MSET`）
/// 
/// 参数：
/// - `name`: 连接名称
/// - `items`: 二维数组 `[key, value]`
/// 
/// 返回：`CommandResponse<bool>`，成功 `true`
#[tauri::command]
async fn mset_values(state: tauri::State<'_, AppState>, name: String, items: Vec<(String, String)>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, items: Vec<(String, String)>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            svc.mset(&items).await?;
            Ok(CommandResponse::ok(true))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, items).await.map_err(InvokeError::from_anyhow)
}

/// 发布消息（`PUBLISH`）到频道
/// 
/// 参数：
/// - `name`: 连接名称
/// - `channel`: 频道名
/// - `message`: 消息内容
/// 
/// 返回：`CommandResponse<i64>`，订阅者接收数量
#[tauri::command]
async fn publish_message(state: tauri::State<'_, AppState>, name: String, channel: String, message: String) -> Result<CommandResponse<i64>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, channel: String, message: String) -> CommandResult<i64> {
        if let Some(svc) = state.get_service(&name).await {
            let n = svc.publish(&channel, &message).await?;
            Ok(CommandResponse::ok(n))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, channel, message).await.map_err(InvokeError::from_anyhow)
}

/// 订阅频道（`SUBSCRIBE`），并通过事件桥接到前端
/// 
/// 建立一个持续的 Redis 订阅连接。当收到消息时，后端会通过 Tauri 的事件系统
/// 将消息转发给前端。
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `channel`: 频道名
/// - `event`: 前端事件名，后端将通过 `emit(event, payload)` 推送消息
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<String>`，成功订阅返回 `"subscribed"`。
/// 
/// # 前端示例
/// 
/// ```ts
/// const unlisten = await subscribeChannel('local', 'news', 'redis:news', (msg) => {
///   console.log('Received:', msg);
/// });
/// // 页面卸载时调用
/// unlisten();
/// ```
#[tauri::command]
async fn subscribe_channel(app: tauri::AppHandle, state: tauri::State<'_, AppState>, name: String, channel: String, event: String) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(app: tauri::AppHandle, state: tauri::State<'_, AppState>, name: String, channel: String, event: String) -> CommandResult<String> {
        if let Some(svc) = state.get_service(&name).await {
            let ev = event.clone();
            svc.subscribe(channel, move |payload| {
                let _ = app.emit(&ev, payload);
                true
            }).await?;
            Ok(CommandResponse::ok("subscribed".to_string()))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(app, state, name, channel, event).await.map_err(InvokeError::from_anyhow)
}

/// 分布式锁：尝试加锁
/// 
/// 使用 Redis 的 `SET key value NX PX ttl` 命令实现原子加锁。
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `resource`: 资源名（即 Redis 键名）
/// - `token`: 锁标识（客户端随机生成，用于解锁校验）
/// - `ttl_ms`: 锁的自动过期时间（毫秒）
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`：
/// - `true`: 加锁成功
/// - `false`: 锁已被占用
/// 
/// # 前端示例
/// 
/// ```ts
/// const locked = await tryLock('local', 'lock:1', 'uuid', 5000);
/// ```
#[tauri::command]
async fn try_lock(state: tauri::State<'_, AppState>, name: String, resource: String, token: String, ttl_ms: u64) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, resource: String, token: String, ttl_ms: u64) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.try_lock(&resource, &token, ttl_ms).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, resource, token, ttl_ms).await.map_err(InvokeError::from_anyhow)
}

/// 分布式锁：原子解锁
/// 
/// 使用 Lua 脚本保证解锁操作的原子性：仅当键存在且值等于 `token` 时才删除键。
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `resource`: 资源名（键）
/// - `token`: 锁标识（需与加锁时一致）
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`：
/// - `true`: 解锁成功
/// - `false`: 锁不存在或 token 不匹配
/// 
/// # 前端示例
/// 
/// ```ts
/// await unlock('local', 'lock:1', 'uuid');
/// ```
#[tauri::command]
async fn unlock(state: tauri::State<'_, AppState>, name: String, resource: String, token: String) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, resource: String, token: String) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.unlock(&resource, &token).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, resource, token).await.map_err(InvokeError::from_anyhow)
}

/// 移除键的过期时间（PERSIST）
/// 
/// 使键变为永久有效。
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `key`: 键名
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`，成功移除过期时间返回 `true`。
/// 
/// # 前端示例
/// 
/// ```ts
/// await persistKey('local', 'mykey');
/// ```
#[tauri::command]
async fn persist_key(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.persist(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

/// 设置键过期时间（EXPIRE）
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `key`: 键名
/// - `seconds`: 过期时间（秒）
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<bool>`，设置成功返回 `true`。
/// 
/// # 前端示例
/// 
/// ```ts
/// await expireKey('local', 'mykey', 60);
/// ```
#[tauri::command]
async fn expire_key(state: tauri::State<'_, AppState>, name: String, key: String, seconds: u64, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, seconds: u64, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.expire(db.unwrap_or(0), &key, seconds).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, seconds, db).await.map_err(InvokeError::from_anyhow)
}

/// 扫描键（SCAN）
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `cursor`: 游标
/// - `pattern`: 匹配模式（可选）
/// - `count`: 数量（可选）
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<(u64, Vec<String>)>`
#[tauri::command]
async fn scan_keys(state: tauri::State<'_, AppState>, name: String, db: u32, cursor: u64, pattern: Option<String>, count: Option<usize>) -> Result<CommandResponse<(u64, Vec<String>)>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, db: u32, cursor: u64, pattern: Option<String>, count: Option<usize>) -> CommandResult<(u64, Vec<String>)> {
        if let Some(svc) = state.get_service(&name).await {
            let res = svc.scan(db, cursor, pattern, count).await?;
            Ok(CommandResponse::ok(res))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, db, cursor, pattern, count).await.map_err(InvokeError::from_anyhow)
}

/// 获取数据库键数量（DBSIZE）
#[tauri::command]
async fn get_db_size(state: tauri::State<'_, AppState>, name: String, db: u32) -> Result<CommandResponse<u64>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, db: u32) -> CommandResult<u64> {
        if let Some(svc) = state.get_service(&name).await {
            let size = svc.dbsize(db).await?;
            Ok(CommandResponse::ok(size))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, db).await.map_err(InvokeError::from_anyhow)
}


/// 查询键剩余过期时间（TTL）
/// 
/// # 参数
/// 
/// - `name`: 连接名称
/// - `key`: 键名
/// 
/// # 返回值
/// 
/// 返回 `CommandResponse<i64>`，遵循 Redis TTL 语义：
/// - `> 0`: 剩余秒数
/// - `-1`: 键存在但无过期时间（永久）
/// - `-2`: 键不存在
/// 
/// # 前端示例
/// 
/// ```ts
/// const ttl = await ttlKey('local', 'mykey');
/// ```
#[tauri::command]
async fn ttl_key(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<i64>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<i64> {
        if let Some(svc) = state.get_service(&name).await {
            let v = svc.ttl(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(v))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

/// 获取集群信息（仅集群模式有效）
/// 
/// 返回 `CommandResponse<Vec<ClusterNodeInfo>>`
#[tauri::command]
async fn get_cluster_info(state: tauri::State<'_, AppState>, name: String) -> Result<CommandResponse<Vec<ClusterNodeInfo>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String) -> CommandResult<Vec<ClusterNodeInfo>> {
        if let Some(svc) = state.get_service(&name).await {
            let info = svc.get_cluster_nodes().await?;
            Ok(CommandResponse::ok(info))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name).await.map_err(InvokeError::from_anyhow)
}

/// 获取键类型 (TYPE)
#[tauri::command]
async fn get_type(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<String> {
        if let Some(svc) = state.get_service(&name).await {
            let t = svc.get_type(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(t))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

/// 获取哈希表所有字段 (HGETALL)
#[tauri::command]
async fn hgetall_hash(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<std::collections::HashMap<String, String>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<std::collections::HashMap<String, String>> {
        if let Some(svc) = state.get_service(&name).await {
            let res: std::collections::HashMap<String, String> = svc.hgetall(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(res))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn hset_field(state: tauri::State<'_, AppState>, name: String, key: String, field: String, value: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, field: String, value: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.hset(db.unwrap_or(0), &key, &field, value).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, field, value, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn hdel_field(state: tauri::State<'_, AppState>, name: String, key: String, field: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, field: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.hdel(db.unwrap_or(0), &key, &field).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, field, db).await.map_err(InvokeError::from_anyhow)
}

/// 列表左侧推入 (LPUSH)
#[tauri::command]
async fn lpush_list(state: tauri::State<'_, AppState>, name: String, key: String, value: String, db: Option<u32>) -> Result<CommandResponse<i64>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, value: String, db: Option<u32>) -> CommandResult<i64> {
        if let Some(svc) = state.get_service(&name).await {
            let len = svc.lpush(db.unwrap_or(0), &key, value).await?;
            Ok(CommandResponse::ok(len))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, value, db).await.map_err(InvokeError::from_anyhow)
}

/// 列表右侧弹出 (RPOP)
#[tauri::command]
async fn rpop_list(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<Option<String>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<Option<String>> {
        if let Some(svc) = state.get_service(&name).await {
            let val: Option<String> = svc.rpop(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(val))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn lrange_list(state: tauri::State<'_, AppState>, name: String, key: String, start: isize, stop: isize, db: Option<u32>) -> Result<CommandResponse<Vec<String>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, start: isize, stop: isize, db: Option<u32>) -> CommandResult<Vec<String>> {
        if let Some(svc) = state.get_service(&name).await {
            let v: Vec<String> = svc.lrange(db.unwrap_or(0), &key, start, stop).await?;
            Ok(CommandResponse::ok(v))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, start, stop, db).await.map_err(InvokeError::from_anyhow)
}

/// 集合添加元素 (SADD)
#[tauri::command]
async fn sadd_set(state: tauri::State<'_, AppState>, name: String, key: String, value: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, value: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let added = svc.sadd(db.unwrap_or(0), &key, value).await?;
            Ok(CommandResponse::ok(added))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, value, db).await.map_err(InvokeError::from_anyhow)
}

/// 获取集合所有成员 (SMEMBERS)
#[tauri::command]
async fn smembers_set(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> Result<CommandResponse<Vec<String>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, db: Option<u32>) -> CommandResult<Vec<String>> {
        if let Some(svc) = state.get_service(&name).await {
            let members: Vec<String> = svc.smembers(db.unwrap_or(0), &key).await?;
            Ok(CommandResponse::ok(members))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn srem_set(state: tauri::State<'_, AppState>, name: String, key: String, member: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, member: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.srem(db.unwrap_or(0), &key, member).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, member, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn zadd_zset(state: tauri::State<'_, AppState>, name: String, key: String, member: String, score: f64, db: Option<u32>) -> Result<CommandResponse<i64>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, member: String, score: f64, db: Option<u32>) -> CommandResult<i64> {
        if let Some(svc) = state.get_service(&name).await {
            let n = svc.zadd(db.unwrap_or(0), &key, member, score).await?;
            Ok(CommandResponse::ok(n))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, member, score, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn zrem_zset(state: tauri::State<'_, AppState>, name: String, key: String, member: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, member: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let ok = svc.zrem(db.unwrap_or(0), &key, member).await?;
            Ok(CommandResponse::ok(ok))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, member, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn zrange_zset(state: tauri::State<'_, AppState>, name: String, key: String, start: isize, stop: isize, db: Option<u32>) -> Result<CommandResponse<Vec<(String, f64)>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, start: isize, stop: isize, db: Option<u32>) -> CommandResult<Vec<(String, f64)>> {
        if let Some(svc) = state.get_service(&name).await {
            let v = svc.zrange_withscores(db.unwrap_or(0), &key, start, stop).await?;
            Ok(CommandResponse::ok(v))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, start, stop, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn json_get_value(state: tauri::State<'_, AppState>, name: String, key: String, path: Option<String>, db: Option<u32>) -> Result<CommandResponse<Option<serde_json::Value>>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, path: Option<String>, db: Option<u32>) -> CommandResult<Option<serde_json::Value>> {
        if let Some(svc) = state.get_service(&name).await {
            let p = path.unwrap_or("$".to_string());
            let v = svc.json_get(db.unwrap_or(0), &key, &p).await?;
            Ok(CommandResponse::ok(v))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, path, db).await.map_err(InvokeError::from_anyhow)
}

#[tauri::command]
async fn json_set_value(state: tauri::State<'_, AppState>, name: String, key: String, path: Option<String>, value_json: String, db: Option<u32>) -> Result<CommandResponse<bool>, InvokeError> {
    async fn inner(state: tauri::State<'_, AppState>, name: String, key: String, path: Option<String>, value_json: String, db: Option<u32>) -> CommandResult<bool> {
        if let Some(svc) = state.get_service(&name).await {
            let p = path.unwrap_or("$".to_string());
            let v: serde_json::Value = serde_json::from_str(&value_json)?;
            svc.json_set(db.unwrap_or(0), &key, &p, &v).await?;
            Ok(CommandResponse::ok(true))
        } else {
            Ok(CommandResponse::err("NOT_FOUND", "service not found"))
        }
    }
    inner(state, name, key, path, value_json, db).await.map_err(InvokeError::from_anyhow)
}

/// 测试 Redis 连接配置（不保存）
///
/// 用于在添加/编辑连接时测试配置是否有效。
///
/// 参数：
/// - `config`: RedisConfig 对象
///
/// 返回：`CommandResponse<String>`，成功返回 "ok"
#[tauri::command]
async fn test_connection_config(config: RedisConfig) -> Result<CommandResponse<String>, InvokeError> {
    async fn inner(config: RedisConfig) -> CommandResult<String> {
        // 尝试建立连接
        let svc = crate::redis_service::RedisService::new(config).await?;
        // 执行健康检查
        svc.check_health().await?;
        // 断开连接（虽然 Drop 会自动处理，但显式调用更清晰）
        svc.disconnect().await;
        Ok(CommandResponse::ok("ok".to_string()))
    }
    inner(config).await.map_err(InvokeError::from_anyhow)
}

/// 应用程序主运行函数
/// 
/// 初始化并启动 Tauri 应用程序，配置所有必要的插件和处理器。
/// 
/// # 初始化流程
/// 
/// 1. **插件配置**：
///    - 日志插件：用于统一的日志记录
///    - 文件打开插件：用于处理文件系统操作
/// 
/// 2. **应用状态初始化**：
///    - 创建数据库目录
///    - 初始化 `AppState` 实例
///    - 加载已保存的 Redis 连接配置
/// 
/// 3. **命令处理器注册**：
///    - 注册健康检查命令
///    - 其他命令处理器可以在此添加
/// 
/// # 异步初始化
/// 
/// `AppState` 的初始化是异步的，因为它需要：
/// - 创建数据库连接
/// - 加载已保存的配置
/// - 建立 Redis 连接池
/// 
/// # 错误处理
/// 
/// 如果应用程序运行失败，会 panic 并显示错误信息。这通常发生在：
/// - Tauri 上下文创建失败
/// - 插件初始化失败
/// - 应用状态初始化失败
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 创建 Tauri 应用程序构建器
    tauri::Builder::default()
        // 注册日志插件，用于统一日志记录
        .plugin(logging::plugin())
        // 注册文件打开插件，用于处理文件相关操作
        .plugin(tauri_plugin_opener::init())
        // 应用程序设置和初始化
        .setup(|app| {
            // 获取应用程序句柄的克隆，用于异步任务
            let handle = app.handle().clone();
            
            // 在异步运行时中初始化应用状态
            tauri::async_runtime::spawn(async move {
                // 构建数据库文件路径
                let db_path = handle.path().app_data_dir().unwrap().join("app.db");
                
                // 确保数据库目录存在
                if let Some(parent) = db_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                
                // 初始化应用状态
                match AppState::new(db_path.to_str().unwrap()).await {
                    Ok(state) => {
                        // 将应用状态管理器注册到 Tauri 应用程序
                        handle.manage(state);
                        logging::info("INIT", "AppState initialized");
                    }
                    Err(e) => {
                        // 如果初始化失败，记录错误日志
                        logging::error("INIT", &format!("Failed to init AppState: {}", e));
                    }
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            add_connection,
            remove_connection,
            check_connection,
            get_value,
            set_value,
            del_key,
            mget_values,
            mset_values,
            publish_message,
            subscribe_channel,
            try_lock,
            unlock,
            persist_key,
            expire_key,
            ttl_key,
            get_cluster_info,
            scan_keys,
            get_db_size,
            list_configs,
            get_config,
            save_config,
            delete_config,
            list_services,
            reload_services,
            service_exists,
            get_type,
            hgetall_hash,
            lpush_list,
            rpop_list,
            sadd_set,
            smembers_set,
            hset_field,
            hdel_field,
            srem_set,
            lrange_list,
            zadd_zset,
            zrem_zset,
            zrange_zset,
            json_get_value,
            json_set_value,
            test_connection_config
        ])
        // 运行应用程序
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
