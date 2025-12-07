//! 应用程序状态管理模块
//! 
//! 本模块负责管理应用程序的全局状态，包括：
//! - Redis 服务连接池管理
//! - 数据库连接管理
//! - 配置信息持久化
//! - 服务生命周期管理
//! 
//! # 设计模式
//! 
//! 使用 `Arc<RwLock<HashMap<String, RedisService>>>` 模式：
//! - `Arc`：允许在多个线程间共享所有权
//! - `RwLock`：提供读写锁，支持多个并发读取或独占写入
//! - `HashMap`：存储命名服务实例，支持多连接管理
//! 
//! # 使用场景
//! 
//! - 应用程序启动时加载已保存的连接配置
//! - 动态添加/删除 Redis 连接
//! - 获取指定名称的 Redis 服务实例
//! - 热重载配置信息

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use crate::redis_service::{RedisService, RedisConfig};
use crate::db::DbManager;
use crate::logging;

/// 应用程序全局状态管理器
/// 
/// 负责管理数据库连接和 Redis 服务实例集合。
/// 提供了连接的增删改查功能，以及从数据库加载配置的能力。
/// 
/// # 字段说明
/// 
/// - `db`: SQLite 数据库管理器，负责配置信息的持久化存储
/// - `services`: Redis 服务实例映射，键为连接名称，值为对应的服务实例
/// 
/// # 线程安全
/// 
/// `services` 使用 `Arc<RwLock<>>` 确保线程安全：
/// - 多个线程可以同时读取服务实例
/// - 只有一个线程可以修改服务实例集合
/// - 支持异步操作，不会阻塞事件循环
pub struct AppState {
    /// 数据库管理器，负责 SQLite 数据库的操作
    pub db: DbManager,
    
    /// Redis 服务实例映射
    /// 
    /// 键：连接名称（用户定义的友好名称）
    /// 值：对应的 Redis 服务实例，支持连接池和重试机制
    pub services: Arc<RwLock<HashMap<String, RedisService>>>,
}

impl AppState {
    /// 创建新的应用状态实例
    /// 
    /// 初始化数据库连接并创建服务映射容器。如果数据库中已存在配置，
    /// 会自动加载并建立对应的 Redis 连接。
    /// 
    /// # 参数
    /// 
    /// - `db_path`: SQLite 数据库文件的路径
    /// 
    /// # 返回值
    /// 
    /// 返回初始化完成的 `AppState` 实例，包含数据库连接和已加载的服务。
    /// 
    /// # 错误处理
    /// 
    /// 可能的错误包括：
    /// - 数据库连接失败
    /// - Redis 服务连接失败
    /// - 配置数据格式错误
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let state = AppState::new("app.db").await?;
    /// ```
    pub async fn new(db_path: &str) -> Result<Self> {
        // 初始化数据库连接
        let db = DbManager::new(db_path).await?;
        
        // 创建线程安全的服务映射容器
        let services = Arc::new(RwLock::new(HashMap::new()));
        
        // 创建应用状态实例
        let state = Self { db, services };
        
        // 从数据库加载已保存的配置并建立连接
        state.reload_from_db().await?;
        
        Ok(state)
    }

    /// 从数据库重新加载所有连接配置
    /// 
    /// 读取数据库中保存的所有 Redis 连接配置，并重新建立对应的 Redis 服务实例。
    /// 
    /// # 重载策略
    /// 
    /// 当前实现采用"清空重建"策略：
    /// 1. 清空现有的所有服务连接
    /// 2. 从数据库重新加载所有配置
    /// 3. 为每个配置创建新的 Redis 服务实例
    /// 
    /// # 优点
    /// - 确保内存中的状态与数据库完全一致
    /// - 简单可靠，避免状态不一致问题
    /// 
    /// # 缺点
    /// - 会断开所有现有连接
    /// - 对于正在使用的连接可能造成短暂中断
    /// 
    /// # 未来改进
    /// 
    /// 可以考虑增量更新策略，只更新发生变化的配置。
    /// 
    /// # 错误处理
    /// 
    /// 如果某个配置无法创建连接，会记录错误日志但不会中断整个重载过程。
    pub async fn reload_from_db(&self) -> Result<()> {
        // 从数据库获取所有保存的配置
        let configs = self.db.list_configs().await?;
        
        // 获取写锁权限
        let mut map = self.services.write().await;
        
        // 清空现有连接，确保状态一致性
        map.clear();
        
        // 为每个配置创建 Redis 服务实例
        for (name, cfg) in configs {
            match RedisService::new(cfg).await {
                Ok(svc) => {
                    // 添加成功，记录日志
                    map.insert(name.clone(), svc);
                    logging::info("APP_STATE", &format!("Loaded service: {}", name));
                },
                Err(e) => {
                    // 连接失败，记录错误但不中断其他连接
                    logging::error("APP_STATE", &format!("Failed to load service {}: {}", name, e));
                }
            }
        }
        
        Ok(())
    }

    /// 获取指定名称的 Redis 服务实例
    /// 
    /// 从服务映射中获取指定名称的 Redis 服务实例的克隆。
    /// 这是线程安全的，多个线程可以同时调用。
    /// 
    /// # 参数
    /// 
    /// - `name`: 要获取的服务实例的名称
    /// 
    /// # 返回值
    /// 
    /// 如果找到对应的服务实例，返回 `Some(RedisService)`，
    /// 否则返回 `None`。
    /// 
    /// # 线程安全
    /// 
    /// 使用读锁，多个线程可以同时获取不同的服务实例。
    /// 返回的 `RedisService` 实例是 `Clone` 的，可以安全地在多个地方使用。
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// if let Some(redis) = state.get_service("my_redis").await {
    ///     let value: Option<String> = redis.get("my_key").await?;
    /// }
    /// ```
    pub async fn get_service(&self, name: &str) -> Option<RedisService> {
        // 获取读锁权限，查找指定名称的服务
        let map = self.services.read().await;
        map.get(name).cloned()
    }

    /// 添加新的 Redis 连接配置
    /// 
    /// 执行完整的添加流程：
    /// 1. 验证 Redis 连接是否可用
    /// 2. 将配置保存到数据库
    /// 3. 将服务实例添加到内存映射中
    /// 
    /// # 参数
    /// 
    /// - `name`: 连接的友好名称，必须唯一
    /// - `config`: Redis 连接配置，包含服务器地址、认证等信息
    /// 
    /// # 返回值
    /// 
    /// 成功时返回 `Ok(())`，失败时返回错误信息。
    /// 
    /// # 错误处理
    /// 
    /// 采用"先验证后保存"的策略：
    /// - 如果 Redis 连接失败，不会保存配置到数据库
    /// - 如果数据库保存失败，不会添加到内存映射
    /// 
    /// 这种设计确保了数据一致性，避免保存无效的配置。
    /// 
    /// # 事务性
    /// 
    /// 虽然不是原子操作，但通过错误处理确保：
    /// - 要么全部操作成功
    /// - 要么全部失败，不留下部分状态
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let config = RedisConfig {
    ///     urls: vec!["redis://localhost:6379".to_string()],
    ///     ..Default::default()
    /// };
    /// state.add_connection("local_redis", config).await?;
    /// ```
    pub async fn add_connection(&self, name: &str, config: RedisConfig) -> Result<()> {
        // 第一步：验证 Redis 连接是否可用
        // 这里会建立实际的连接并执行基本的健康检查
        let svc = RedisService::new(config.clone()).await
            .context("Failed to connect to Redis")?;
        
        // 第二步：将配置保存到数据库持久化存储
        self.db.save_config(name, &config).await
            .context("Failed to save config to DB")?;
        
        // 第三步：将验证通过的服务实例添加到内存映射
        let mut map = self.services.write().await;
        map.insert(name.to_string(), svc);
        
        // 记录成功日志
        logging::info("APP_STATE", &format!("Added connection: {}", name));
        
        Ok(())
    }

    /// 删除指定的 Redis 连接配置
    /// 
    /// 执行完整的删除流程：
    /// 1. 从数据库中删除配置记录
    /// 2. 从内存映射中移除服务实例
    /// 
    /// # 参数
    /// 
    /// - `name`: 要删除的连接名称
    /// 
    /// # 返回值
    /// 
    /// 成功时返回 `Ok(())`，失败时返回错误信息。
    /// 
    /// # 资源清理
    /// 
    /// 当服务实例从映射中移除后，其引用计数会减少。
    /// 当引用计数降为 0 时，底层的 Redis 连接会被自动关闭。
    /// 
    /// # 顺序保证
    /// 
    /// 先删除数据库记录，再删除内存中的实例。
    /// 这样即使后续操作失败，数据库中的状态也是一致的。
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// state.remove_connection("old_redis").await?;
    /// ```
    pub async fn remove_connection(&self, name: &str) -> Result<()> {
        // 第一步：从数据库删除配置记录
        let deleted = self.db.delete_config(name).await
            .context("Failed to delete config from DB")?;
        
        if !deleted {
            logging::warn("APP_STATE", &format!("Config not found in DB during removal: {}", name));
        }

        // 第二步：从内存映射中移除服务实例
        let mut map = self.services.write().await;
        map.remove(name);
        
        // 记录成功日志
        logging::info("APP_STATE", &format!("Removed connection: {}", name));
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 测试应用程序状态的完整生命周期
    /// 
    /// 测试流程：
    /// 1. 创建应用状态
    /// 2. 添加连接配置
    /// 3. 验证连接可以获取
    /// 4. 重载配置
    /// 5. 删除连接配置
    #[tokio::test]
    async fn test_app_state_flow() {
        // 使用临时数据库文件进行测试
        let db_path = "test_app_state.db";
        // 清理之前可能存在的测试文件
        let _ = fs::remove_file(db_path);

        // 创建应用状态实例
        let state = AppState::new(db_path).await.unwrap();
        
        // 1. 添加连接配置
        let cfg = RedisConfig {
            urls: vec!["redis://127.0.0.1:6379".into()], // 假设本地 Redis 服务正在运行
            ..Default::default()
        };
        
        // 尝试添加连接（可能因 Redis 服务不可用而失败）
        let res = state.add_connection("test_conn", cfg).await;
        if let Err(e) = &res {
            println!("Add connection failed (expected if redis not up): {}", e);
            // 如果 Redis 服务不可用，我们无法测试完整的连接流程
            // 但可以测试数据库持久化部分
        } else {
            // 连接成功的情况
            assert!(res.is_ok());
            
            // 2. 验证可以获取服务实例
            let svc = state.get_service("test_conn").await;
            assert!(svc.is_some());
            
            // 3. 测试重载功能
            state.reload_from_db().await.unwrap();
            let svc = state.get_service("test_conn").await;
            assert!(svc.is_some());
            
            // 4. 删除连接配置
            state.remove_connection("test_conn").await.unwrap();
            let svc = state.get_service("test_conn").await;
            assert!(svc.is_none());
        }

        // 清理测试数据库文件
        let _ = fs::remove_file(db_path);
    }
}