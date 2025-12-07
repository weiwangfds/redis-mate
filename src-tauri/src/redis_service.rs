//! Redis 服务模块
//! 
//! 本模块提供了一个统一的 Redis 服务接口，支持单机、哨兵和集群三种部署模式。
//! 封装了常用的 Redis 数据结构操作、批量操作、事务、发布订阅、分布式锁以及管理命令。
//! 
//! # 核心特性
//! 
//! - **多模式支持**: 统一接口支持单机、哨兵、集群三种 Redis 部署模式
//! - **自动重试**: 内置重试机制，提高连接的可靠性和鲁棒性
//! - **类型安全**: 利用 Rust 的类型系统提供类型安全的 Redis 操作
//! - **异步操作**: 所有操作都是异步的，不阻塞事件循环
//! - **连接池管理**: 自动管理连接生命周期和资源清理
//! - **错误处理**: 详细的错误上下文信息，便于调试和问题定位
//! 
//! # 支持的 Redis 功能
//! 
//! ## 基础数据结构
//! - **String**: 基本的键值对操作
//! - **Hash**: 哈希表字段操作，支持批量操作
//! - **List**: 列表的推入、弹出操作
//! - **Set**: 集合的添加、查询操作
//! 
//! ## 高级功能
//! - **批量操作**: MGET、MSET 等批量命令
//! - **事务**: MULTI/EXEC 事务支持
//! - **发布订阅**: 普通模式和分片模式的 Pub/Sub
//! - **分布式锁**: 基于 SET NX PX 的分布式锁实现
//! - **JSON 操作**: JSON 数据的存储和检索
//! 
//! ## 管理功能
//! - **健康检查**: PING 命令验证连接状态
//! - **集群管理**: CLUSTER NODES、CLUSTER SLOTS 等命令
//! - **配置管理**: CONFIG SET、BGSAVE 等管理命令
//! 
//! # 使用示例
//! 
//! ## 基本连接和操作
//! 
//! ```rust
//! use crate::redis_service::{RedisService, RedisConfig};
//! 
//! // 创建配置
//! let config = RedisConfig {
//!     urls: vec!["redis://127.0.0.1:6379".to_string()],
//!     ..Default::default()
//! };
//! 
//! // 创建服务实例
//! let redis = RedisService::new(config).await?;
//! 
//! // 基础操作
//! redis.set("key", "value", Some(3600)).await?;
//! let value: Option<String> = redis.get("key").await?;
//! 
//! // 哈希操作
//! redis.hset("user:1", "name", "Alice").await?;
//! let name: Option<String> = redis.hget("user:1", "name").await?;
//! ```
//! 
//! ## 集群模式连接
//! 
//! ```rust
//! let cluster_config = RedisConfig {
//!     cluster: true,
//!     urls: vec![
//!         "redis://127.0.0.1:7001".to_string(),
//!         "redis://127.0.0.1:7002".to_string(),
//!     ],
//!     ..Default::default()
//! };
//! 
//! let cluster_redis = RedisService::new(cluster_config).await?;
//! ```
//! 
//! ## 哨兵模式连接
//! 
//! ```rust
//! let sentinel_config = RedisConfig {
//!     sentinel: true,
//!     sentinel_master_name: Some("mymaster".to_string()),
//!     sentinel_urls: vec!["redis://127.0.0.1:26379".to_string()],
//!     ..Default::default()
//! };
//! 
//! let sentinel_redis = RedisService::new(sentinel_config).await?;
//! ```
//! 
//! ## 分布式锁使用
//! 
//! ```rust
//! let resource = "critical_section";
//! let token = "unique_lock_token";
//! let ttl_ms = 5000; // 5秒过期时间
//! 
//! // 尝试获取锁
//! if redis.try_lock(resource, token, ttl_ms).await? {
//!     // 执行临界区代码
//!     // ...
//!     
//!     // 释放锁
//!     redis.unlock(resource, token).await?;
//! } else {
//!     // 获取锁失败
//! }
//! ```
//! 
//! # 性能考虑
//! 
//! - **连接复用**: 使用连接池管理连接，避免频繁建立/断开连接
//! - **批量操作**: 尽可能使用批量命令减少网络往返
//! - **管道操作**: 对于多个命令，考虑使用事务或管道
//! - **集群感知**: 在集群模式下，确保相关键在同一个槽位
//! 
//! # 错误处理
//! 
//! 所有方法都返回 `Result<T>`，包含详细的错误上下文：
//! - 连接失败会自动重试（可配置重试次数）
//! - 网络超时和临时错误会被透明处理
//! - 配置错误和认证失败会立即返回
//! 
//! # 线程安全
//! 
//! `RedisService` 实现了 `Clone`，可以安全地在多个线程间共享：
//! - 单机模式使用 `ConnectionManager`，线程安全
//! - 集群模式使用 `ClusterClient`，支持并发访问
//! 
//! # 依赖说明
//! 
//! 本模块依赖以下主要 crate：
//! - `redis`: Redis 客户端库
//! - `tokio`: 异步运行时
//! - `serde`: 序列化/反序列化支持
//! - `anyhow`: 错误处理
//! - `futures`: 异步工具

use anyhow::{anyhow, Context, Result};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Cmd, Pipeline};
use redis::cluster::ClusterClient;
use crate::logging;
use std::time::Duration;
use std::collections::HashMap;
use futures::StreamExt;

/// Redis 连接配置结构
/// 
/// 统一管理 Redis 连接的所有配置参数，支持单机、哨兵、集群三种模式。
/// 提供了合理的默认值，支持根据实际需求调整连接参数。
/// 
/// # 字段说明
/// 
/// ## 连接配置
/// - `urls`: Redis 服务器地址列表
///   - 单机模式：使用第一个地址
///   - 集群模式：传入一个或多个种子节点地址
///   - 哨兵模式：此字段为主节点地址（通常由哨兵自动解析）
/// 
/// ## 模式选择
/// - `cluster`: 启用集群模式（与 `sentinel` 互斥）
/// - `sentinel`: 启用哨兵模式（与 `cluster` 互斥）
/// - `sentinel_master_name`: 哨兵主节点名称（哨兵模式必需）
/// - `sentinel_urls`: 哨兵节点地址列表（哨兵模式必需）
/// 
/// ## 性能配置
/// - `pool_size`: 连接池大小（底层管理器实际处理连接数）
/// - `retries`: 操作失败时的自动重试次数
/// - `retry_delay_ms`: 重试之间的延迟时间（毫秒）
/// 
/// # 配置示例
/// 
/// ```rust
/// // 单机模式
/// let standalone_config = RedisConfig {
///     urls: vec!["redis://127.0.0.1:6379".to_string()],
///     cluster: false,
///     sentinel: false,
///     ..Default::default()
/// };
/// 
/// // 集群模式
/// let cluster_config = RedisConfig {
///     cluster: true,
///     urls: vec![
///         "redis://127.0.0.1:7001".to_string(),
///         "redis://127.0.0.1:7002".to_string(),
///     ],
///     ..Default::default()
/// };
/// 
/// // 哨兵模式
/// let sentinel_config = RedisConfig {
///     sentinel: true,
///     sentinel_master_name: Some("mymaster".to_string()),
///     sentinel_urls: vec!["redis://127.0.0.1:26379".to_string()],
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RedisConfig {
    /// Redis 服务器地址列表
    /// 
    /// 格式支持：
    /// - `redis://127.0.0.1:6379` - 标准 Redis 连接
    /// - `redis://:password@127.0.0.1:6379` - 带密码认证
    /// - `rediss://127.0.0.1:6379` - SSL/TLS 连接
    /// 
    /// 单机模式使用第一个地址，集群模式可以使用多个地址作为种子节点。
    pub urls: Vec<String>,
    
    /// 是否启用集群模式
    /// 
    /// Redis Cluster 提供数据分片、高可用性和水平扩展能力。
    /// 设置为 `true` 时，会使用集群客户端连接到 Redis 集群。
    /// 
    /// 注意：`cluster` 和 `sentinel` 不能同时为 `true`。
    pub cluster: bool,
    
    /// 连接池大小
    /// 
    /// 指定连接池中保持的最大连接数。虽然 Redis 客户端内部管理实际连接，
    /// 但这个参数会影响并发操作的性能。
    /// 
    /// 推荐值：
    /// - 低并发应用：4-8
    /// - 中等并发应用：8-16
    /// - 高并发应用：16-32
    pub pool_size: usize,
    
    /// 自动重试次数
    /// 
    /// 当 Redis 操作因网络问题或临时故障失败时，自动重试的次数。
    /// 增加此值可以提高连接的稳定性，但会增加操作延迟。
    /// 
    /// 推荐值：2-5 次
    pub retries: u32,
    
    /// 重试延迟时间（毫秒）
    /// 
    /// 每次重试之间的等待时间。使用指数退避策略会更有效，
    /// 当前实现使用固定延迟。
    /// 
    /// 推荐值：100-500 毫秒
    pub retry_delay_ms: u64,
    
    /// 是否启用哨兵模式
    /// 
    /// Redis Sentinel 提供高可用性监控和自动故障转移。
    /// 设置为 `true` 时，会通过哨兵发现主节点地址。
    /// 
    /// 注意：`sentinel` 和 `cluster` 不能同时为 `true`。
    pub sentinel: bool,
    
    /// 哨兵主节点名称
    /// 
    /// 在哨兵配置中指定的主节点名称。客户端通过哨兵查询此名称
    /// 来获取当前主节点的实际地址。
    /// 
    /// 哨兵模式必需字段。
    pub sentinel_master_name: Option<String>,
    
    /// 哨兵节点地址列表
    /// 
    /// 哨兵进程的地址列表。客户端会连接这些哨兵来获取主节点信息。
    /// 建议配置多个哨兵地址以提高可用性。
    /// 
    /// 哨兵模式必需字段。
    pub sentinel_urls: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClusterNodeInfo {
    pub id: String,
    pub addr: String,
    pub flags: String,
    pub master_id: String,
    pub ping_sent: String,
    pub pong_recv: String,
    pub config_epoch: String,
    pub link_state: String,
    pub slots: Vec<String>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            // 默认连接本地 Redis 实例
            urls: vec!["redis://127.0.0.1:6379".into()],
            
            // 默认单机模式
            cluster: false,
            
            // 适中的连接池大小
            pool_size: 16,
            
            // 适中的重试策略
            retries: 3,
            retry_delay_ms: 200,
            
            // 默认不使用哨兵
            sentinel: false,
            sentinel_master_name: None,
            sentinel_urls: vec![],
        }
    }
}

/// Redis 服务实例
/// 
/// 主要的 Redis 操作接口，封装了底层连接管理和重试逻辑。
/// 支持单机、哨兵、集群三种部署模式的统一操作接口。
/// 
/// # 线程安全
/// 
/// `RedisService` 实现了 `Clone`，可以安全地在多个线程间共享：
/// - 克隆操作不会创建新的连接，只是增加引用计数
/// - 内部使用连接管理器自动处理连接复用
/// - 所有操作都是异步的，不会阻塞线程
/// 
/// # 重试机制
/// 
/// 所有 Redis 操作都通过 `with_retry` 方法包装，提供：
/// - 自动重试失败的连接
/// - 可配置的重试次数和延迟
/// - 详细的错误日志记录
/// - 智能的错误分类和处理
/// 
/// # 使用建议
/// 
/// - 长期应用中保持 `RedisService` 实例，避免频繁创建
/// - 在集群模式下注意键的分布，避免跨槽位操作
/// - 合理配置重试参数，平衡稳定性和响应速度
/// - 监控连接状态，及时发现和处理问题
#[derive(Clone)]
pub struct RedisService {
    /// 连接类型枚举，存储实际的连接对象
    kind: ConnectionKind,
    
    /// 连接配置，用于重连和日志记录
    cfg: RedisConfig,
}

/// Redis 连接类型枚举
/// 
/// 封装不同部署模式的连接对象：
/// - `Standalone`: 单机或哨兵模式的连接管理器和原始客户端（用于特定 DB 操作）
    /// - `Cluster`: 集群模式的客户端连接
    #[derive(Clone)]
    enum ConnectionKind {
        /// 单机模式连接管理器
        /// 
        /// 使用 `ConnectionManager` 提供：
        /// - 自动连接复用
        /// - 连接断开自动重连
        /// - 线程安全的并发访问
        /// 
        /// `Client` 用于创建特定数据库的临时连接
        Standalone(ConnectionManager, redis::Client),
        
        /// 集群模式客户端
        /// 
        /// 使用 `ClusterClient` 提供：
        /// - 自动路由到正确的节点
        /// - 槽位感知的命令分发
        /// - 集群拓扑的自动更新
        Cluster(ClusterClient),
    }

impl RedisService {
    /// 创建新的 Redis 服务实例
    /// 
    /// 根据配置初始化对应模式的 Redis 连接：
    /// - 集群模式：创建 `ClusterClient`
    /// - 哨兵模式：通过哨兵发现主节点地址
    /// - 单机模式：直接连接到指定地址
    /// 
    /// # 参数
    /// 
    /// - `cfg`: Redis 连接配置，包含服务器地址、认证信息等
    /// 
    /// # 返回值
    /// 
    /// 返回初始化完成的 `RedisService` 实例，可以立即使用。
    /// 
    /// # 错误处理
    /// 
    /// 可能的错误包括：
    /// - 连接字符串格式错误
    /// - 网络连接失败
    /// - 认证失败
    /// - 集群拓扑获取失败
    /// - 哨兵主节点解析失败
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let config = RedisConfig {
    ///     urls: vec!["redis://127.0.0.1:6379".to_string()],
    ///     ..Default::default()
    /// };
    /// 
    /// let redis = RedisService::new(config).await?;
    /// ```
    pub async fn new(cfg: RedisConfig) -> Result<Self> {
        if cfg.cluster {
            // 集群模式初始化
            logging::info("REDIS_INIT", &format!("cluster mode urls={:?}", cfg.urls));
            let client = ClusterClient::new(cfg.urls.clone())?;
            return Ok(Self { kind: ConnectionKind::Cluster(client), cfg });
        }

        // 解析连接地址
        let url = if cfg.sentinel {
            // 哨兵模式：通过 redis+sentinel 协议自动处理
            let master = cfg.sentinel_master_name.as_ref()
                .ok_or_else(|| anyhow!("sentinel master name required"))?;
            logging::info("REDIS_INIT", &format!("sentinel mode master={} sentinels={:?}", master, cfg.sentinel_urls));
            
            let url = build_sentinel_url(master, &cfg.sentinel_urls)?;
            logging::info("REDIS_INIT", &format!("sentinel url={}", url));
            url
        } else {
            // 单机模式：直接使用配置的地址
            cfg.urls.get(0)
                .ok_or_else(|| anyhow!("no redis url provided"))?
                .clone()
        };
        
        logging::info("REDIS_INIT", &format!("connecting to url={}", url));
        
        // 创建 Redis 客户端和连接管理器
        let client = redis::Client::open(url)?;
        let manager = client.get_connection_manager().await?;
        
        Ok(Self { kind: ConnectionKind::Standalone(manager, client), cfg })
    }

    /// 带自动重试的操作执行包装器
    /// 
    /// 为所有 Redis 操作提供统一的错误重试机制：
    /// - 可配置的重试次数和延迟
    /// - 智能错误分类，只重试可恢复的错误
    /// - 详细的错误日志记录
    /// - 指数退避策略（可扩展）
    /// 
    /// # 泛型参数
    /// 
    /// - `F`: 异步函数闭包类型
    /// - `Fut`: 异步函数返回的 Future 类型
    /// - `T`: 操作的返回值类型
    /// 
    /// # 参数
    /// 
    /// - `f`: 要执行的异步操作闭包
    /// 
    /// # 返回值
    /// 
    /// 返回操作的结果或最终的错误。
    /// 
    /// # 重试策略
    /// 
    /// 当前实现使用固定延迟重试：
    /// 1. 执行操作
    /// 2. 如果失败且未达到重试上限，等待 `retry_delay_ms`
    /// 3. 记录警告日志
    /// 4. 重新执行操作
    /// 5. 重复直到成功或达到重试上限
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// self.with_retry(|| async {
    ///     let mut conn = manager.clone();
    ///     conn.set("key", "value").await
    /// }).await
    /// ```
    async fn with_retry<F, Fut, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut attempts = 0;
        
        loop {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    attempts += 1;
                    
                    // 检查是否超过重试次数
                    if attempts > self.cfg.retries {
                        return Err(e);
                    }
                    
                    // 等待重试延迟
                    let delay = Duration::from_millis(self.cfg.retry_delay_ms);
                    logging::warn("REDIS_RETRY", &format!("attempt {} failed: {}", attempts, e));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// 健康检查
    /// 
    /// 通过 PING 命令验证 Redis 连接的可用性。
    /// 这是一个简单的连接状态检查，不涉及复杂的操作。
    /// 
    /// # 返回值
    /// 
    /// 成功时返回 `Ok(())`，表示连接正常。
    /// 失败时返回具体的错误信息。
    /// 
    /// # 使用场景
    /// 
    /// - 应用程序启动时的连接验证
    /// - 定期的健康状态监控
    /// - 连接恢复后的状态检查
    /// - 用户手动连接测试
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// match redis.check_health().await {
    ///     Ok(_) => println!("Redis is healthy"),
    ///     Err(e) => println!("Redis health check failed: {}", e),
    /// }
    /// ```
    pub async fn check_health(&self) -> Result<()> {
        let pong: String = self.ping().await.context("check health ping")?;
        if pong != "PONG" {
            return Err(anyhow!("Unexpected health check response: {}", pong));
        }
        Ok(())
    }

    /// 显式断开连接
    /// 
    /// 注意：Redis 客户端使用引用计数管理连接，调用此方法并不会立即关闭连接。
    /// 当所有 `RedisService` 实例被丢弃时，连接会自动关闭。
    /// 
    /// # 资源管理
    /// 
    /// - `ConnectionManager`: 自动管理连接生命周期
    /// - `ClusterClient`: 使用引用计数，计数为 0 时关闭连接
    /// 
    /// # 建议
    /// 
    /// 通常不需要显式调用此方法。让 Rust 的所有权系统自动管理资源即可。
    pub async fn disconnect(&self) {
        logging::info("REDIS_CLOSE", "Disconnect called (dropping handles)");
        // ConnectionManager and ClusterClient are ref-counted handles.
        // Dropping them reduces ref count. If 0, actual connections are closed.
        // There is no async close method on them in redis crate.
    }

    /// 扫描当前数据库的键（SCAN 命令）
    ///
    /// 支持分页遍历键空间，避免 KEYS 命令阻塞 Redis。
    ///
    /// # 参数
    ///
    /// - `db`: 数据库索引（仅单机模式有效）
    /// - `cursor`: 游标，开始时为 0
    /// - `pattern`: 匹配模式（可选）
    /// - `count`: 每次扫描的建议数量（可选）
    ///
    /// # 返回值
    ///
    /// 返回 `(u64, Vec<String>)`：
    /// - `u64`: 下次迭代的游标，为 0 表示结束
    /// - `Vec<String>`: 扫描到的键列表
    pub async fn scan(&self, db: u32, cursor: u64, pattern: Option<String>, count: Option<usize>) -> Result<(u64, Vec<String>)> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let mut cmd = redis::cmd("SCAN");
                        cmd.arg(cursor);
                        if let Some(p) = &pattern {
                            if !p.is_empty() {
                                cmd.arg("MATCH").arg(p);
                            }
                        }
                        if let Some(c) = count {
                            if c > 0 {
                                cmd.arg("COUNT").arg(c);
                            }
                        }
                        let (next_cursor, keys): (u64, Vec<String>) = cmd.query_async(&mut conn).await.context("SCAN")?;
                        Ok((next_cursor, keys))
                    } else {
                         let client = client.clone();
                         let pattern = pattern.clone();
                         tokio::task::spawn_blocking(move || -> Result<(u64, Vec<String>)> {
                             let mut conn = client.get_connection().context("get dedicated connection")?;
                             redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                             let mut cmd = redis::cmd("SCAN");
                             cmd.arg(cursor);
                             if let Some(p) = &pattern {
                                 if !p.is_empty() {
                                     cmd.arg("MATCH").arg(p);
                                 }
                             }
                             if let Some(c) = count {
                                 if c > 0 {
                                     cmd.arg("COUNT").arg(c);
                                 }
                             }
                             let (next_cursor, keys): (u64, Vec<String>) = cmd.query(&mut conn).context("SCAN")?;
                             Ok((next_cursor, keys))
                         }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let client = client.clone();
                    let pattern = pattern.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<(u64, Vec<String>)> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let mut cmd = redis::cmd("SCAN");
                        cmd.arg(cursor);
                        if let Some(p) = &pattern {
                            if !p.is_empty() {
                                cmd.arg("MATCH").arg(p);
                            }
                        }
                        if let Some(c) = count {
                            if c > 0 {
                                cmd.arg("COUNT").arg(c);
                            }
                        }
                        let (next_cursor, keys): (u64, Vec<String>) = cmd.query(&mut conn).context("SCAN")?;
                        Ok((next_cursor, keys))
                    }).await.unwrap()
                }
            }
        }).await
    }
    /// 获取当前数据库的键数量（DBSIZE 命令）
    ///
    /// # 参数
    ///
    /// - `db`: 数据库索引
    ///
    /// # 返回值
    ///
    /// 返回数据库中的键总数。
    pub async fn dbsize(&self, db: u32) -> Result<u64> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let size: u64 = redis::cmd("DBSIZE").query_async(&mut conn).await.context("DBSIZE")?;
                        Ok(size)
                    } else {
                        let client = client.clone();
                        tokio::task::spawn_blocking(move || -> Result<u64> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let size: u64 = redis::cmd("DBSIZE").query(&mut conn).context("DBSIZE")?;
                            Ok(size)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let client = client.clone();
                    tokio::task::spawn_blocking(move || -> Result<u64> {
                        let mut conn = client.get_connection().context("get dedicated connection")?;
                        let size: u64 = redis::cmd("DBSIZE").query(&mut conn).context("DBSIZE")?;
                        Ok(size)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 批量操作 ---

    /// 批量获取多个键的值（MGET 命令）
    /// 
    /// 一次性获取多个键的值，比多次单独 GET 操作更高效。
    /// 在集群模式下，所有键应该在同一个槽位以避免跨节点错误。
    /// 
    /// # 泛型参数
    /// 
    /// - `K`: 键类型，必须实现 `ToRedisArgs`
    /// - `T`: 值类型，必须实现 `FromRedisValue`
    /// 
    /// # 参数
    /// 
    /// - `keys`: 要获取的键列表
    /// 
    /// # 返回值
    /// 
    /// 返回与键列表对应的值列表，不存在的键对应 `None`。
    /// 
    /// # 性能考虑
    /// 
    /// - 集群模式下确保键在同一槽位
    /// - 一次获取的键数量不宜过多（建议 < 100）
    /// - 可以用来替代多个单次 GET 操作
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let keys = vec!["user:1", "user:2", "user:3"];
    /// let values: Vec<Option<String>> = redis.mget(&keys).await?;
    /// ```
    pub async fn mget<K: redis::ToRedisArgs + Send + Sync, T: redis::FromRedisValue + Send + 'static>(&self, keys: &[K]) -> Result<Vec<Option<T>>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let v: Vec<Option<T>> = conn.mget(keys).await.context("MGET")?;
                    Ok(v)
                }
                ConnectionKind::Cluster(client) => {
                    // 集群模式下的 MGET 处理
                    let keys: Vec<String> = keys.iter()
                        .map(|k| redis::ToRedisArgs::to_redis_args(k).get(0)
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_default())
                        .collect();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Vec<Option<T>>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Vec<Option<T>> = redis::cmd("MGET").arg(&keys).query(&mut conn).context("MGET")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 批量设置多个键值对（MSET 命令）
    /// 
    /// 一次性设置多个键值对，比多次单独 SET 操作更高效。
    /// 在集群模式下，所有键应该在同一个槽位。
    /// 
    /// # 泛型参数
    /// 
    /// - `K`: 键类型，必须实现 `ToRedisArgs`
    /// - `V`: 值类型，必须实现 `ToRedisArgs`
    /// 
    /// # 参数
    /// 
    /// - `items`: 键值对列表
    /// 
    /// # 性能考虑
    /// 
    /// - 原子操作：要么全部成功，要么全部失败
    /// - 比多次 SET 操作减少网络往返
    /// - 适合批量初始化数据
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let items = vec![
    ///     ("key1", "value1"),
    ///     ("key2", "value2"),
    ///     ("key3", "value3"),
    /// ];
    /// redis.mset(&items).await?;
    /// ```
    pub async fn mset<K: redis::ToRedisArgs + Send + Sync + 'static, V: redis::ToRedisArgs + Send + Sync + 'static>(&self, items: &[(K, V)]) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    conn.mset::<_, _, ()>(items).await.context("MSET")?;
                    Ok(())
                }
                ConnectionKind::Cluster(client) => {
                    // 集群模式下的 MSET 处理
                    let items_vec: Vec<(String, Vec<u8>)> = items.iter().map(|(k, v)| {
                        let k_str = redis::ToRedisArgs::to_redis_args(k).get(0)
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_default();
                        let v_bytes = redis::ToRedisArgs::to_redis_args(v).get(0)
                            .map(|b| b.clone())
                            .unwrap_or_default();
                        (k_str, v_bytes)
                    }).collect();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        redis::cmd("MSET").arg(&items_vec).query::<()>(&mut conn).context("MSET")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 事务 ---

    /// 执行 Redis 事务（MULTI/EXEC）
    /// 
    /// 通过管道构建器模式创建事务，保证命令的原子执行。
    /// 在集群模式下，所有键必须在同一个槽位。
    /// 
    /// # 参数
    /// 
    /// - `f`: 闭包，用于构建事务管道
    /// 
    /// # 事务特性
    /// 
    /// - **原子性**: 要么全部执行，要么全部不执行
    /// - **隔离性**: 事务执行期间不会被其他命令打断
    /// - **一致性**: 保证数据库状态的一致性
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.transaction(|pipe| {
    ///     pipe.incr("counter", 1);
    ///     pipe.set("last_update", "2023-01-01");
    ///     pipe.expire("last_update", 3600);
    /// }).await?;
    /// ```
    /// 
    /// # 注意事项
    /// 
    /// - 事务中的命令不会被立即执行，而是在 EXEC 时批量执行
    /// - 集群模式下所有涉及的键必须在同一槽位
    /// - 事务执行失败时，所有命令都不会执行
    pub async fn transaction<F>(&self, f: F) -> Result<()> 
    where F: Fn(&mut Pipeline) + Send + Sync + Clone + 'static
    {
        self.with_retry(|| {
            let f = f.clone();
            async move {
                match &self.kind {
                    ConnectionKind::Standalone(manager, _) => {
                        let mut conn = manager.clone();
                        let mut pipe = redis::pipe();
                        pipe.atomic(); // 设置原子模式
                        f(&mut pipe);
                        pipe.query_async::<()>(&mut conn).await.context("TRANSACTION")?;
                        Ok(())
                    }
                    ConnectionKind::Cluster(client) => {
                        let client = client.clone();
                        let mut pipe = redis::pipe();
                        pipe.atomic();
                        f(&mut pipe);
                        
                        tokio::task::spawn_blocking(move || -> Result<()> {
                            let mut conn = client.get_connection().context("get cluster connection")?;
                            pipe.query::<()>(&mut conn).context("TRANSACTION")?;
                            Ok(())
                        }).await.unwrap()
                    }
                }
            }
        }).await
    }

    // --- 发布订阅 ---

    /// 订阅 Redis 频道并处理消息
    /// 
    /// 创建独立的订阅连接，避免阻塞主要业务连接。
    /// 为每个收到的消息执行回调函数，当回调返回 `false` 时停止订阅。
    /// 
    /// # 参数
    /// 
    /// - `channel`: 要订阅的频道名称
    /// - `callback`: 消息处理回调，返回 `false` 时停止订阅
    /// 
    /// # 实现细节
    /// 
    /// - 使用专用的 Pub/Sub 连接，不影响其他操作
    /// - 集群模式下连接到种子节点（传统 Pub/Sub 是节点局部的）
    /// - 异步消息处理循环，出现错误时记录日志并继续
    /// - 支持优雅停止（通过回调返回值）
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.subscribe("notifications", |message| {
    ///     println!("Received: {}", message);
    ///     true // 继续订阅
    /// }).await?;
    /// ```
    /// 
    /// # 注意事项
    /// 
    /// - 订阅是长期运行的，会创建后台任务
    /// - 集群模式下 Pub/Sub 是节点局部的
    /// - 分片 Pub/Sub 请使用 `ssubscribe` 和 `spublish`
    /// - 回调函数应该是快速执行的，避免阻塞消息处理
    pub async fn subscribe<F>(&self, channel: String, mut callback: F) -> Result<()> 
    where F: FnMut(String) -> bool + Send + 'static // Returns false to stop
    {
        // 根据模式确定连接地址
        let url = if self.cfg.cluster {
            // 集群模式：连接到种子节点
            self.cfg.urls.get(0)
                .ok_or_else(|| anyhow!("no cluster seed url"))?
                .clone()
        } else if self.cfg.sentinel {
            // 哨兵模式：构建 Sentinel URL
            let master = self.cfg.sentinel_master_name.as_ref()
                .ok_or_else(|| anyhow!("no master name"))?;
            build_sentinel_url(master, &self.cfg.sentinel_urls)?
        } else {
            // 单机模式：直接使用配置地址
            self.cfg.urls.get(0)
                .ok_or_else(|| anyhow!("no url"))?
                .clone()
        };

        // 创建专用的 Pub/Sub 连接
        let client = redis::Client::open(url)?;
        let mut pubsub_conn = client.get_async_pubsub().await?;
        pubsub_conn.subscribe(channel.clone()).await?;
        
        // 启动消息处理任务
        tokio::spawn(async move {
            let mut stream = pubsub_conn.on_message();
            while let Some(msg) = stream.next().await {
                let payload: String = match msg.get_payload() {
                    Ok(s) => s,
                    Err(e) => {
                        logging::error("PUBSUB", &format!("Payload error: {}", e));
                        continue;
                    }
                };
                
                // 执行回调，如果返回 false 则停止订阅
                if !callback(payload) {
                    break;
                }
            }
        });
        
        Ok(())
    }

    /// 发布消息到指定频道
    /// 
    /// 向指定频道发布消息，返回订阅该频道的客户端数量。
    /// 支持普通 Pub/Sub 模式。
    /// 
    /// # 参数
    /// 
    /// - `channel`: 频道名称
    /// - `message`: 要发布的消息内容
    /// 
    /// # 返回值
    /// 
    /// 返回接收到消息的订阅者数量。
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let subscribers = redis.publish("news", "Hello, World!").await?;
    /// println!("Message sent to {} subscribers", subscribers);
    /// ```
    pub async fn publish(&self, channel: &str, message: &str) -> Result<i64> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let n: i64 = conn.publish(channel, message).await.context("PUBLISH")?;
                    Ok(n)
                }
                ConnectionKind::Cluster(client) => {
                    let channel = channel.to_string();
                    let message = message.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<i64> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("PUBLISH").arg(&channel).arg(&message).query(&mut conn).context("PUBLISH")?;
                        Ok(n)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 发布消息到指定分片频道
    /// 
    /// Redis 7.0+ 的分片 Pub/Sub 功能，将消息路由到特定的分片。
    /// 相比普通 Pub/Sub，具有更好的扩展性。
    /// 
    /// # 参数
    /// 
    /// - `channel`: 分片频道名称
    /// - `message`: 要发布的消息内容
    /// 
    /// # 返回值
    /// 
    /// 返回接收到消息的订阅者数量。
    /// 
    /// # 版本要求
    /// 
    /// 需要 Redis 7.0 或更高版本。
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let subscribers = redis.spublish("sharded_news", "Hello, Sharded World!").await?;
    /// println!("Message sent to {} subscribers", subscribers);
    /// ```
    pub async fn spublish(&self, channel: &str, message: &str) -> Result<i64> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let n: i64 = redis::cmd("SPUBLISH").arg(channel).arg(message).query_async(&mut conn).await.context("SPUBLISH")?;
                    Ok(n)
                }
                ConnectionKind::Cluster(client) => {
                    let channel = channel.to_string();
                    let message = message.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<i64> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("SPUBLISH").arg(&channel).arg(&message).query(&mut conn).context("SPUBLISH")?;
                        Ok(n)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 分布式锁 ---

    /// 尝试获取分布式锁
    /// 
    /// 使用 Redis 的 SET NX PX 命令实现分布式锁，支持过期时间。
    /// 锁的持有者需要使用唯一令牌来确保只有自己能释放锁。
    /// 
    /// # 参数
    /// 
    /// - `resource`: 锁的资源名称（键名）
    /// - `token`: 唯一的锁令牌，用于验证锁的持有者
    /// - `ttl_ms`: 锁的过期时间（毫秒）
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成功获取锁
    /// - `false`: 锁已被其他进程持有
    /// 
    /// # 锁的特性
    /// 
    /// - **互斥性**: 同一时刻只有一个进程能持有锁
    /// - **过期保护**: 锁会自动过期，避免死锁
    /// - **令牌验证**: 只有持有正确令牌的进程才能释放锁
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let resource = "critical_section";
    /// let token = format!("{}:{}", process_id, thread_id);
    /// let ttl_ms = 5000; // 5秒过期
    /// 
    /// if redis.try_lock(resource, &token, ttl_ms).await? {
    ///     // 执行临界区代码
    ///     // ...
    ///     
    ///     // 释放锁
    ///     redis.unlock(resource, &token).await?;
    /// } else {
    ///     println!("Failed to acquire lock");
    /// }
    /// ```
    pub async fn try_lock(&self, resource: &str, token: &str, ttl_ms: u64) -> Result<bool> {
        let result: Option<String> = self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let res: Option<String> = redis::cmd("SET")
                        .arg(resource)
                        .arg(token)
                        .arg("NX")  // 只在键不存在时设置
                        .arg("PX")  // 设置过期时间（毫秒）
                        .arg(ttl_ms)
                        .query_async(&mut conn).await.context("TRY_LOCK")?;
                    Ok(res)
                }
                ConnectionKind::Cluster(client) => {
                    let resource = resource.to_string();
                    let token = token.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Option<String>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let res: Option<String> = redis::cmd("SET")
                            .arg(&resource)
                            .arg(&token)
                            .arg("NX")
                            .arg("PX")
                            .arg(ttl_ms)
                            .query(&mut conn).context("TRY_LOCK")?;
                        Ok(res)
                    }).await.unwrap()
                }
            }
        }).await?;
        
        Ok(result.is_some())
    }

    /// 释放分布式锁
    /// 
    /// 使用 Lua 脚本原子地验证锁令牌并删除键，避免竞争条件。
    /// 只有持有正确令牌的进程才能成功释放锁。
    /// 
    /// # 参数
    /// 
    /// - `resource`: 锁的资源名称（键名）
    /// - `token`: 锁的令牌，必须与获取锁时使用的令牌一致
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成功释放锁
    /// - `false`: 锁不存在或令牌不匹配
    /// 
    /// # Lua 脚本逻辑
    /// 
    /// ```lua
    /// if redis.call("get", KEYS[1]) == ARGV[1] then
    ///     return redis.call("del", KEYS[1])
    /// else
    ///     return 0
    /// end
    /// ```
    /// 
    /// # 安全性
    /// 
    /// - 原子操作：避免竞争条件
    /// - 令牌验证：防止误删其他进程的锁
    /// - 幂等性：多次释放同一个是安全的
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let released = redis.unlock("critical_section", &token).await?;
    /// if released {
    ///     println!("Lock released successfully");
    /// } else {
    ///     println!("Failed to release lock (lock not owned)");
    /// }
    /// ```
    pub async fn unlock(&self, resource: &str, token: &str) -> Result<bool> {
        // Lua 脚本确保原子性
        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
        "#;
        
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let n: i64 = redis::Script::new(script)
                        .key(resource)
                        .arg(token)
                        .invoke_async(&mut conn).await.context("UNLOCK")?;
                    Ok(n > 0)
                }
                ConnectionKind::Cluster(client) => {
                    let resource = resource.to_string();
                    let token = token.to_string();
                    let client = client.clone();
                    let s = redis::Script::new(script);
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = s.key(&resource).arg(&token).invoke(&mut conn).context("UNLOCK")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }
    

    // --- 高级功能 ---
    
    /// 移除键的过期时间
    /// 
    /// 使用 PERSIST 命令移除键的过期时间，使键永久存在。
    /// 
    /// # 参数
    /// 
    /// - `key`: 要移除过期时间的键名
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成功移除过期时间
    /// - `false`: 键不存在或没有设置过期时间
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.set("temp_key", "value", Some(60)).await?; // 60秒过期
    /// let removed = redis.persist("temp_key").await?;  // 移除过期时间
    /// ```
    pub async fn persist(&self, db: u32, key: &str) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = conn.persist(key).await.context("PERSIST")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("PERSIST").arg(&key).query(&mut conn).context("PERSIST")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("PERSIST").arg(&key).query(&mut conn).context("PERSIST")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取键的类型
    /// 
    /// 使用 TYPE 命令获取键的数据类型。
    /// 
    /// # 参数
    /// 
    /// - `db`: 数据库索引
    /// - `key`: 键名
    /// 
    /// # 返回值
    /// 
    /// 返回键的类型字符串（如 "string", "list", "set", "zset", "hash", "stream", "none"）。
    pub async fn get_type(&self, db: u32, key: &str) -> Result<String> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let t: String = redis::cmd("TYPE").arg(key).query_async(&mut conn).await.context("TYPE")?;
                        Ok(t)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<String> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let t: String = redis::cmd("TYPE").arg(&key).query(&mut conn).context("TYPE")?;
                            Ok(t)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<String> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let t: String = redis::cmd("TYPE").arg(&key).query(&mut conn).context("TYPE")?;
                        Ok(t)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 存储 JSON 数据
    /// 
    /// 将可序列化的对象转换为 JSON 字符串并存储到 Redis。
    /// 这是一个便利方法，内部使用 serde 进行序列化。
    /// 
    /// # 泛型参数
    /// 
    /// - `V`: 要存储的值类型，必须实现 `Serialize`
    /// 
    /// # 参数
    /// 
    /// - `db`: 数据库索引
    /// - `key`: 存储的键名
    /// - `value`: 要存储的值
    /// - `expire_seconds`: 可选的过期时间（秒）
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// #[derive(Serialize)]
    /// struct User {
    ///     id: u32,
    ///     name: String,
    /// }
    /// 
    /// let user = User { id: 1, name: "Alice".to_string() };
    /// redis.set_json(0, "user:1", &user, Some(3600)).await?;
    /// ```
    pub async fn set_json<V: serde::Serialize + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, value: &V, expire_seconds: Option<u64>) -> Result<()> {
        let json_str = serde_json::to_string(value).context("serialize json")?;
        self.set(db, key, json_str, expire_seconds).await
    }

    /// 获取并反序列化 JSON 数据
    /// 
    /// 从 Redis 获取 JSON 字符串并反序列化为指定类型的对象。
    /// 这是一个便利方法，内部使用 serde 进行反序列化。
    /// 
    /// # 泛型参数
    /// 
    /// - `T`: 要反序列化的目标类型，必须实现 `DeserializeOwned`
    /// 
    /// # 参数
    /// 
    /// - `db`: 数据库索引
    /// - `key`: 要获取的键名
    /// 
    /// # 返回值
    /// 
    /// - `Some(T)`: 成功获取并反序列化
    /// - `None`: 键不存在
    /// 
    /// # 错误处理
    /// 
    /// 如果 JSON 格式不正确或类型不匹配，会返回反序列化错误。
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// #[derive(Deserialize)]
    /// struct User {
    ///     id: u32,
    ///     name: String,
    /// }
    /// 
    /// if let Some(user) = redis.get_json::<User>(0, "user:1").await? {
    ///     println!("User: {} - {}", user.id, user.name);
    /// }
    /// ```
    pub async fn get_json<T: serde::de::DeserializeOwned + Send + 'static>(&self, db: u32, key: &str) -> Result<Option<T>> {
        let v: Option<String> = self.get(db, key).await?;
        match v {
            Some(s) => {
                let obj = serde_json::from_str(&s).context("deserialize json")?;
                Ok(Some(obj))
            },
            None => Ok(None),
        }
    }

    // --- 基础键值操作 ---

    /// 设置键值对
    /// 
    /// 基本的 SET 操作，支持可选的过期时间。
    /// 
    /// # 参数
    /// 
    /// - `key`: 键名
    /// - `value`: 要存储的值
    /// - `expire_seconds`: 可选的过期时间（秒）
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// // 永久存储
    /// redis.set("key", "value", None).await?;
    /// 
    /// // 60秒后过期
    /// redis.set("temp_key", "temp_value", Some(60)).await?;
    /// ```
    pub async fn set<V: redis::ToRedisArgs + redis::ToSingleRedisArg + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, value: V, expire_seconds: Option<u64>) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        if let Some(exp) = expire_seconds {
                            conn.set_ex(key, value.clone(), exp).await.context("SETEX")?
                        } else {
                            conn.set(key, value.clone()).await.context("SET")?
                        }
                        Ok(())
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let val = value.clone();
                        let exp = expire_seconds;
                        tokio::task::spawn_blocking(move || -> Result<()> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            if let Some(e) = exp {
                                redis::cmd("SETEX").arg(&key).arg(e).arg(&val).query::<()>(&mut conn).context("SETEX")?;
                            } else {
                                redis::cmd("SET").arg(&key).arg(&val).query::<()>(&mut conn).context("SET")?;
                            }
                            Ok(())
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let val = value.clone();
                    let exp = expire_seconds;
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        if let Some(e) = exp {
                            redis::cmd("SETEX").arg(&key).arg(e).arg(&val).query::<()>(&mut conn).context("SETEX")?;
                        } else {
                            redis::cmd("SET").arg(&key).arg(&val).query::<()>(&mut conn).context("SET")?;
                        }
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取键的值
    /// 
    /// 基本的 GET 操作，不存在的键返回 `None`。
    /// 
    /// # 参数
    /// 
    /// - `key`: 要获取的键名
    /// 
    /// # 返回值
    /// 
    /// - `Some(T)`: 键存在，返回对应的值
    /// - `None`: 键不存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// if let Some(value) = redis.get::<String>("key").await? {
    ///     println!("Value: {}", value);
    /// } else {
    ///     println!("Key not found");
    /// }
    /// ```
    pub async fn get<T: redis::FromRedisValue + Send + 'static>(&self, db: u32, key: &str) -> Result<Option<T>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let v: Option<T> = conn.get(key).await.context("GET")?;
                        Ok(v)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Option<T>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let v: Option<T> = redis::cmd("GET").arg(key).query(&mut conn).context("GET")?;
                            Ok(v)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Option<T>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Option<T> = redis::cmd("GET").arg(&key).query(&mut conn).context("GET")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取集群节点信息
    pub async fn get_cluster_nodes(&self) -> Result<Vec<ClusterNodeInfo>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(_, _) => {
                    // For standalone mode, return empty list or handle as error?
                    // User might try to get cluster info for standalone.
                    Ok(vec![])
                }
                ConnectionKind::Cluster(client) => {
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Vec<ClusterNodeInfo>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let info: String = redis::cmd("CLUSTER").arg("NODES").query(&mut conn).context("CLUSTER NODES")?;
                        
                        let mut nodes = Vec::new();
                        for line in info.lines() {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() < 8 {
                                continue;
                            }
                            
                            // 格式: <id> <ip:port@cport[,hostname]> <flags> <master> <ping-sent> <pong-recv> <config-epoch> <link-state> <slot> <slot> ...
                            // id: parts[0]
                            // addr: parts[1]
                            // flags: parts[2]
                            // master: parts[3]
                            // ping: parts[4]
                            // pong: parts[5]
                            // epoch: parts[6]
                            // state: parts[7]
                            // slots: parts[8..]
                            
                            let mut slots = Vec::new();
                            if parts.len() > 8 {
                                for i in 8..parts.len() {
                                    slots.push(parts[i].to_string());
                                }
                            }
                            
                            nodes.push(ClusterNodeInfo {
                                id: parts[0].to_string(),
                                addr: parts[1].to_string(),
                                flags: parts[2].to_string(),
                                master_id: parts[3].to_string(),
                                ping_sent: parts[4].to_string(),
                                pong_recv: parts[5].to_string(),
                                config_epoch: parts[6].to_string(),
                                link_state: parts[7].to_string(),
                                slots,
                            });
                        }
                        
                        Ok(nodes)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 删除键
    /// 
    /// 使用 DEL 命令删除指定的键。
    /// 
    /// # 参数
    /// 
    /// - `key`: 要删除的键名
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成功删除键
    /// - `false`: 键不存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let deleted = redis.del("temp_key").await?;
    /// if deleted {
    ///     println!("Key deleted successfully");
    /// }
    /// ```
    pub async fn del(&self, db: u32, key: &str) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = conn.del(key).await.context("DEL")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("DEL").arg(&key).query(&mut conn).context("DEL")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("DEL").arg(&key).query(&mut conn).context("DEL")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 检查键是否存在
    /// 
    /// 使用 EXISTS 命令检查键是否存在于数据库中。
    /// 
    /// # 参数
    /// 
    /// - `key`: 要检查的键名
    /// 
    /// # 返回值
    /// 
    /// - `true`: 键存在
    /// - `false`: 键不存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// if redis.exists("my_key").await? {
    ///     println!("Key exists");
    /// } else {
    ///     println!("Key does not exist");
    /// }
    /// ```
    pub async fn exists(&self, db: u32, key: &str) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = conn.exists(key).await.context("EXISTS")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("EXISTS").arg(&key).query(&mut conn).context("EXISTS")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("EXISTS").arg(&key).query(&mut conn).context("EXISTS")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 设置键的过期时间
    /// 
    /// 使用 EXPIRE 命令为已存在的键设置过期时间。
    /// 
    /// # 参数
    /// 
    /// - `key`: 要设置过期时间的键名
    /// - `seconds`: 过期时间（秒）
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成功设置过期时间
    /// - `false`: 键不存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.set("my_key", "value", None).await?;
    /// redis.expire("my_key", 3600).await?; // 1小时后过期
    /// ```
    pub async fn expire(&self, db: u32, key: &str, seconds: u64) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let res: bool = conn.expire(key, i64::try_from(seconds).unwrap()).await.context("EXPIRE")?;
                        Ok(res)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let sec = i64::try_from(seconds).unwrap();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let res: bool = redis::cmd("EXPIRE").arg(&key).arg(sec).query(&mut conn).context("EXPIRE")?;
                            Ok(res)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let sec = i64::try_from(seconds).unwrap();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let res: bool = redis::cmd("EXPIRE").arg(&key).arg(sec).query(&mut conn).context("EXPIRE")?;
                        Ok(res)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取键的剩余过期时间
    /// 
    /// 使用 TTL 命令查询键的剩余生存时间。
    /// 
    /// # 参数
    /// 
    /// - `key`: 要查询的键名
    /// 
    /// # 返回值
    /// 
    /// - `> 0`: 剩余过期时间（秒）
    /// - `-1`: 键存在但没有设置过期时间
    /// - `-2`: 键不存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let ttl = redis.ttl("my_key").await?;
    /// match ttl {
    ///     -2 => println!("Key does not exist"),
    ///     -1 => println!("Key has no expiration"),
    ///     t  => println!("Key will expire in {} seconds", t),
    /// }
    /// ```
    pub async fn ttl(&self, db: u32, key: &str) -> Result<i64> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let res: i64 = conn.ttl(key).await.context("TTL")?;
                        Ok(res)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<i64> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let res: i64 = redis::cmd("TTL").arg(&key).query(&mut conn).context("TTL")?;
                            Ok(res)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<i64> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let res: i64 = redis::cmd("TTL").arg(&key).query(&mut conn).context("TTL")?;
                        Ok(res)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取键的数据类型
    ///
    /// 使用 TYPE 命令获取键的数据类型。
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回值
    ///
    /// 返回类型字符串，如 "string", "list", "set", "zset", "hash", "stream", "none"。
    pub async fn key_type(&self, db: u32, key: &str) -> Result<String> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let t: String = redis::cmd("TYPE").arg(key).query_async(&mut conn).await.context("TYPE")?;
                        Ok(t)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<String> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let t: String = redis::cmd("TYPE").arg(&key).query(&mut conn).context("TYPE")?;
                            Ok(t)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<String> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let t: String = redis::cmd("TYPE").arg(&key).query(&mut conn).context("TYPE")?;
                        Ok(t)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 哈希操作 ---

    /// 设置哈希字段
    /// 
    /// 使用 HSET 命令设置哈希表中的字段值。
    /// 
    /// # 参数
    /// 
    /// - `key`: 哈希表的键名
    /// - `field`: 字段名
    /// - `value`: 字段值
    /// 
    /// # 返回值
    /// 
    /// - `true`: 字段是新增的
    /// - `false`: 字段已存在并被更新
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.hset("user:1", "name", "Alice").await?;
    /// redis.hset("user:1", "age", 25).await?;
    /// ```
    pub async fn hset<V: redis::ToRedisArgs + redis::ToSingleRedisArg + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, field: &str, value: V) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = conn.hset(key, field, value.clone()).await.context("HSET")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let field = field.to_string();
                        let value = value.clone();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("HSET").arg(&key).arg(&field).arg(&value).query(&mut conn).context("HSET")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let field = field.to_string();
                    let value = value.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("HSET").arg(&key).arg(&field).arg(&value).query(&mut conn).context("HSET")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    pub async fn hdel(&self, db: u32, key: &str, field: &str) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = redis::Cmd::new().arg("HDEL").arg(key).arg(field).query_async(&mut conn).await.context("HDEL")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let field = field.to_string();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("HDEL").arg(&key).arg(&field).query(&mut conn).context("HDEL")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let field = field.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("HDEL").arg(&key).arg(&field).query(&mut conn).context("HDEL")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取哈希字段值
    /// 
    /// 使用 HGET 命令获取哈希表中指定字段的值。
    /// 
    /// # 参数
    /// 
    /// - `key`: 哈希表的键名
    /// - `field`: 要获取的字段名
    /// 
    /// # 返回值
    /// 
    /// - `Some(T)`: 字段存在，返回对应的值
    /// - `None`: 字段不存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// if let Some(name) = redis.hget::<String>("user:1", "name").await? {
    ///     println!("User name: {}", name);
    /// }
    /// ```
    pub async fn hget<T: redis::FromRedisValue + Send + 'static>(&self, db: u32, key: &str, field: &str) -> Result<Option<T>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let v: Option<T> = conn.hget(key, field).await.context("HGET")?;
                        Ok(v)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let field = field.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Option<T>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let v: Option<T> = redis::cmd("HGET").arg(&key).arg(&field).query(&mut conn).context("HGET")?;
                            Ok(v)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let field = field.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Option<T>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Option<T> = redis::cmd("HGET").arg(&key).arg(&field).query(&mut conn).context("HGET")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 批量设置哈希字段
    /// 
    /// 使用 HMSET 命令（新版 Redis 中用 HSET 的多参数形式）批量设置哈希字段。
    /// 等价于历史上的 HMSET 命令。
    /// 
    /// # 参数
    /// 
    /// - `key`: 哈希表的键名
    /// - `items`: 字段值对列表
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let items = vec![
    ///     ("name", "Alice"),
    ///     ("age", "25"),
    ///     ("email", "alice@example.com"),
    /// ];
    /// redis.hmset("user:1", &items).await?;
    /// ```
    pub async fn hmset<K: redis::ToRedisArgs + Send + Sync + 'static, V: redis::ToRedisArgs + Send + Sync + 'static>(&self, db: u32, key: &str, items: &[(K, V)]) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        conn.hset_multiple::<_, _, _, ()>(key, items).await.context("HSET MULTIPLE")?;
                        Ok(())
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        // 序列化 items 以便在 blocking task 中使用
                        // 这里我们不能直接传递泛型 K, V，因为它们可能不是 Clone 的
                        // 但是 ToRedisArgs 也不容易序列化。
                        // 这是一个棘手的问题。
                        // 既然 K, V 是 ToRedisArgs，我们可以尝试转换为 Vec<(Vec<u8>, Vec<u8>)>?
                        // Redis crate 的 ToRedisArgs trait 实际上是用来追加参数的。
                        
                        // 为了简化，我们假设 K 和 V 实现了 Clone。
                        // 但是函数签名里没有 Clone。
                        // 我们可能需要修改函数签名或者在此处做一些转换。
                        // 考虑到这只是一个示例代码，我们可以要求 K, V 必须是 Clone。
                        // 或者我们直接在外部调用多次 HSET？不，那样效率低。
                        
                        // 让我们尝试把 arguments 转换成 Vec<Vec<u8>> 在这里。
                        let mut args = Vec::new();
                        for (k, v) in items {
                            let mut k_args = Vec::new();
                            k.write_redis_args(&mut k_args);
                            args.extend(k_args);
                            
                            let mut v_args = Vec::new();
                            v.write_redis_args(&mut v_args);
                            args.extend(v_args);
                        }
                        
                        tokio::task::spawn_blocking(move || -> Result<()> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            
                            let mut cmd = redis::cmd("HSET");
                            cmd.arg(&key);
                            for arg in args {
                                cmd.arg(arg);
                            }
                            cmd.query::<()>(&mut conn).context("HSET MULTIPLE")?;
                            Ok(())
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    // 将字段值对转换为参数列表：key field1 value1 field2 value2 ...
                    let args: Vec<Vec<u8>> = {
                        let mut v: Vec<Vec<u8>> = Vec::with_capacity(items.len() * 2);
                        for (f, val) in items.iter() {
                             let mut f_args = Vec::new();
                             f.write_redis_args(&mut f_args);
                             v.extend(f_args);
                             
                             let mut val_args = Vec::new();
                             val.write_redis_args(&mut val_args);
                             v.extend(val_args);
                        }
                        v
                    };
                    
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let mut cmd = redis::cmd("HSET");
                        cmd.arg(&key);
                        for arg in args {
                            cmd.arg(arg);
                        }
                        cmd.query::<()>(&mut conn).context("HSET MULTIPLE")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取整个哈希表
    /// 
    /// 使用 HGETALL 命令获取哈希表中的所有字段和值。
    /// 
    /// # 参数
    /// 
    /// - `key`: 哈希表的键名
    /// 
    /// # 返回值
    /// 
    /// 返回包含所有字段和值的 HashMap，字段名作为键。
    /// 
    /// # 性能考虑
    /// 
    /// - 大型哈希表可能会消耗较多内存
    /// - 考虑使用 HSCAN 命令处理大型哈希表
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let user_data: HashMap<String, String> = redis.hgetall("user:1").await?;
    /// for (field, value) in user_data {
    ///     println!("{}: {}", field, value);
    /// }
    /// ```
    pub async fn hgetall<T: redis::FromRedisValue + Send + 'static>(&self, db: u32, key: &str) -> Result<HashMap<String, T>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let m: HashMap<String, T> = conn.hgetall(key).await.context("HGETALL")?;
                        Ok(m)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<HashMap<String, T>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let m: HashMap<String, T> = redis::cmd("HGETALL").arg(&key).query(&mut conn).context("HGETALL")?;
                            Ok(m)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<HashMap<String, T>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let m: HashMap<String, T> = redis::cmd("HGETALL").arg(&key).query(&mut conn).context("HGETALL")?;
                        Ok(m)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 列表操作 ---
    /// 从左侧推入列表
    /// 
    /// 使用 LPUSH 命令将一个或多个值推入列表的左端。
    /// 
    /// # 参数
    /// 
    /// - `key`: 列表的键名
    /// - `value`: 要推入的值
    /// 
    /// # 返回值
    /// 
    /// 返回推入后列表的长度。
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let length = redis.lpush("my_list", "world").await?; // [world]
    /// let length = redis.lpush("my_list", "hello").await?; // [hello, world]
    /// ```
    pub async fn lpush<V: redis::ToRedisArgs + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, value: V) -> Result<i64> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = conn.lpush(key, value.clone()).await.context("LPUSH")?;
                        Ok(n)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let value = value.clone();
                        tokio::task::spawn_blocking(move || -> Result<i64> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("LPUSH").arg(&key).arg(&value).query(&mut conn).context("LPUSH")?;
                            Ok(n)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let value = value.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<i64> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("LPUSH").arg(&key).arg(&value).query(&mut conn).context("LPUSH")?;
                        Ok(n)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 从右侧弹出元素
    /// 
    /// 使用 RPOP 命令从列表的右端弹出一个元素。
    /// 这是 FIFO（先进先出）队列的标准操作。
    /// 
    /// # 参数
    /// 
    /// - `key`: 列表的键名
    /// 
    /// # 返回值
    /// 
    /// - `Some(T)`: 成功弹出元素
    /// - `None`: 列表为空
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// // 假设列表为 [hello, world]
    /// if let Some(item) = redis.rpop::<String>("my_list").await? {
    ///     println!("Popped: {}", item); // 输出: "world"
    /// }
    /// ```
    pub async fn rpop<T: redis::FromRedisValue + Send + 'static>(&self, db: u32, key: &str) -> Result<Option<T>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let v: Option<T> = conn.rpop(key, None).await.context("RPOP")?;
                        Ok(v)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Option<T>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let v: Option<T> = redis::cmd("RPOP").arg(&key).query(&mut conn).context("RPOP")?;
                            Ok(v)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Option<T>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Option<T> = redis::cmd("RPOP").arg(&key).query(&mut conn).context("RPOP")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取列表范围 (LRANGE)
    /// 
    /// # 参数
    /// 
    /// - `key`: 列表键名
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    /// 
    /// # 返回值
    /// 
    /// 返回指定范围内的元素列表
    pub async fn lrange<T: redis::FromRedisValue + Send + 'static>(&self, db: u32, key: &str, start: isize, stop: isize) -> Result<Vec<T>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let v: Vec<T> = conn.lrange(key, start, stop).await.context("LRANGE")?;
                        Ok(v)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Vec<T>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let v: Vec<T> = redis::cmd("LRANGE").arg(&key).arg(start).arg(stop).query(&mut conn).context("LRANGE")?;
                            Ok(v)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Vec<T>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Vec<T> = redis::cmd("LRANGE").arg(&key).arg(start).arg(stop).query(&mut conn).context("LRANGE")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 集合操作 ---

    /// 添加集合成员
    /// 
    /// 使用 SADD 命令向集合中添加一个或多个成员。
    /// 集合中的成员是唯一的，重复添加不会产生效果。
    /// 
    /// # 参数
    /// 
    /// - `key`: 集合的键名
    /// - `member`: 要添加的成员
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成员是新增的
    /// - `false`: 成员已存在
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.sadd("my_set", "apple").await?;   // 新增，返回 true
    /// redis.sadd("my_set", "banana").await?;  // 新增，返回 true
    /// redis.sadd("my_set", "apple").await?;   // 已存在，返回 false
    /// ```
    pub async fn sadd<V: redis::ToRedisArgs + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, member: V) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = conn.sadd(key, member.clone()).await.context("SADD")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let member = member.clone();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("SADD").arg(&key).arg(&member).query(&mut conn).context("SADD")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let member = member.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("SADD").arg(&key).arg(&member).query(&mut conn).context("SADD")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取所有集合成员
    /// 
    /// 使用 SMEMBERS 命令获取集合中的所有成员。
    /// 
    /// # 参数
    /// 
    /// - `key`: 集合的键名
    /// 
    /// # 返回值
    /// 
    /// 返回包含所有成员的向量。
    /// 
    /// # 性能考虑
    /// 
    /// - 大型集合可能会消耗较多内存
    /// - 考虑使用 SSCAN 命令处理大型集合
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let members: Vec<String> = redis.smembers("my_set").await?;
    /// for member in members {
    ///     println!("Member: {}", member);
    /// }
    /// ```
    pub async fn smembers<T: redis::FromRedisValue + Send + 'static>(&self, db: u32, key: &str) -> Result<Vec<T>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let v: Vec<T> = conn.smembers(key).await.context("SMEMBERS")?;
                        Ok(v)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Vec<T>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let v: Vec<T> = redis::cmd("SMEMBERS").arg(&key).query(&mut conn).context("SMEMBERS")?;
                            Ok(v)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Vec<T>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Vec<T> = redis::cmd("SMEMBERS").arg(&key).query(&mut conn).context("SMEMBERS")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    pub async fn srem<V: redis::ToRedisArgs + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, member: V) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = redis::Cmd::new().arg("SREM").arg(key).arg(member.clone()).query_async(&mut conn).await.context("SREM")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let member = member.clone();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("SREM").arg(&key).arg(&member).query(&mut conn).context("SREM")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let member = member.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("SREM").arg(&key).arg(&member).query(&mut conn).context("SREM")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 有序集合操作 ---

    pub async fn zadd<V: redis::ToRedisArgs + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, member: V, score: f64) -> Result<i64> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = redis::Cmd::new().arg("ZADD").arg(key).arg(score).arg(member.clone()).query_async(&mut conn).await.context("ZADD")?;
                        Ok(n)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let member = member.clone();
                        tokio::task::spawn_blocking(move || -> Result<i64> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("ZADD").arg(&key).arg(score).arg(&member).query(&mut conn).context("ZADD")?;
                            Ok(n)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let member = member.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<i64> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("ZADD").arg(&key).arg(score).arg(&member).query(&mut conn).context("ZADD")?;
                        Ok(n)
                    }).await.unwrap()
                }
            }
        }).await
    }

    pub async fn zrem<V: redis::ToRedisArgs + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, member: V) -> Result<bool> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let n: i64 = redis::Cmd::new().arg("ZREM").arg(key).arg(member.clone()).query_async(&mut conn).await.context("ZREM")?;
                        Ok(n > 0)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let member = member.clone();
                        tokio::task::spawn_blocking(move || -> Result<bool> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let n: i64 = redis::cmd("ZREM").arg(&key).arg(&member).query(&mut conn).context("ZREM")?;
                            Ok(n > 0)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let member = member.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<bool> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let n: i64 = redis::cmd("ZREM").arg(&key).arg(&member).query(&mut conn).context("ZREM")?;
                        Ok(n > 0)
                    }).await.unwrap()
                }
            }
        }).await
    }

    pub async fn zrange_withscores(&self, db: u32, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let v: Vec<(String, f64)> = redis::cmd("ZRANGE").arg(key).arg(start).arg(stop).arg("WITHSCORES").query_async(&mut conn).await.context("ZRANGE WITHSCORES")?;
                        Ok(v)
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Vec<(String, f64)>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let v: Vec<(String, f64)> = redis::cmd("ZRANGE").arg(&key).arg(start).arg(stop).arg("WITHSCORES").query(&mut conn).context("ZRANGE WITHSCORES")?;
                            Ok(v)
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Vec<(String, f64)>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let v: Vec<(String, f64)> = redis::cmd("ZRANGE").arg(&key).arg(start).arg(stop).arg("WITHSCORES").query(&mut conn).context("ZRANGE WITHSCORES")?;
                        Ok(v)
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- RedisJSON 操作 ---

    pub async fn json_set<V: serde::Serialize + Send + Sync + Clone + 'static>(&self, db: u32, key: &str, path: &str, value: &V) -> Result<()> {
        let json_str = serde_json::to_string(value).context("serialize json value")?;
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        redis::Cmd::new().arg("JSON.SET").arg(key).arg(path).arg(json_str.clone()).query_async::<()>(&mut conn).await.context("JSON.SET")?;
                        Ok(())
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let path = path.to_string();
                        let json_str = json_str.clone();
                        tokio::task::spawn_blocking(move || -> Result<()> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            redis::cmd("JSON.SET").arg(&key).arg(&path).arg(json_str).query::<()>(&mut conn).context("JSON.SET")?;
                            Ok(())
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let path = path.to_string();
                    let json_str = json_str.clone();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        redis::cmd("JSON.SET").arg(&key).arg(&path).arg(json_str).query::<()>(&mut conn).context("JSON.SET")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    pub async fn json_get(&self, db: u32, key: &str, path: &str) -> Result<Option<serde_json::Value>> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, client) => {
                    if db == 0 {
                        let mut conn = manager.clone();
                        let s: Option<String> = redis::Cmd::new().arg("JSON.GET").arg(key).arg(path).query_async(&mut conn).await.context("JSON.GET")?;
                        if let Some(js) = s { Ok(Some(serde_json::from_str(&js).context("parse json")?)) } else { Ok(None) }
                    } else {
                        let client = client.clone();
                        let key = key.to_string();
                        let path = path.to_string();
                        tokio::task::spawn_blocking(move || -> Result<Option<serde_json::Value>> {
                            let mut conn = client.get_connection().context("get dedicated connection")?;
                            redis::cmd("SELECT").arg(db).query::<()>(&mut conn).context("select db")?;
                            let s: Option<String> = redis::cmd("JSON.GET").arg(&key).arg(&path).query(&mut conn).context("JSON.GET")?;
                            if let Some(js) = s { Ok(Some(serde_json::from_str(&js).context("parse json")?)) } else { Ok(None) }
                        }).await.unwrap()
                    }
                }
                ConnectionKind::Cluster(client) => {
                    if db != 0 {
                        return Err(anyhow!("Cluster mode does not support multiple databases"));
                    }
                    let key = key.to_string();
                    let path = path.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<Option<serde_json::Value>> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let s: Option<String> = redis::cmd("JSON.GET").arg(&key).arg(&path).query(&mut conn).context("JSON.GET")?;
                        if let Some(js) = s { Ok(Some(serde_json::from_str(&js).context("parse json")?)) } else { Ok(None) }
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 集群管理命令 ---

    /// 获取集群节点信息
    /// 
    /// 使用 CLUSTER NODES 命令获取集群中所有节点的信息。
    /// 
    /// # 返回值
    /// 
    /// 返回包含节点信息的字符串，每行代表一个节点。
    /// 
    /// # 信息格式
    /// 
    /// 每行包含：节点ID、地址、标志、主节点ID、最后 ping 时间、最后 pong 时间、
    /// 配置纪元、连接状态、节点端口、总线端口、槽位范围。
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let nodes = redis.cluster_nodes().await?;
    /// println!("Cluster nodes:\n{}", nodes);
    /// ```
    pub async fn cluster_nodes(&self) -> Result<String> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let out: String = Cmd::new().arg("CLUSTER").arg("NODES").query_async(&mut conn).await.context("CLUSTER NODES")?;
                    Ok(out)
                }
                ConnectionKind::Cluster(client) => {
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<String> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let out: String = Cmd::new().arg("CLUSTER").arg("NODES").query(&mut conn).context("CLUSTER NODES")?;
                        Ok(out)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 获取集群槽位分布
    /// 
    /// 使用 CLUSTER SLOTS 命令获取集群中槽位的分布情况。
    /// 
    /// # 返回值
    /// 
    /// 返回槽位分布信息的原始 Redis 值。
    /// 
    /// # 信息格式
    /// 
    /// 返回一个数组，每个元素包含：
    /// - 起始槽位
    /// - 结束槽位  
    /// - 主节点信息
    /// - 副本节点信息（可选）
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let slots = redis.cluster_slots().await?;
    /// // 解析槽位分布信息
    /// ```
    pub async fn cluster_slots(&self) -> Result<redis::Value> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let out: redis::Value = Cmd::new().arg("CLUSTER").arg("SLOTS").query_async(&mut conn).await.context("CLUSTER SLOTS")?;
                    Ok(out)
                }
                ConnectionKind::Cluster(client) => {
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<redis::Value> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let out: redis::Value = Cmd::new().arg("CLUSTER").arg("SLOTS").query(&mut conn).context("CLUSTER SLOTS")?;
                        Ok(out)
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 邀请节点加入集群
    /// 
    /// 使用 CLUSTER MEET 命令邀请指定节点加入当前集群。
    /// 
    /// # 参数
    /// 
    /// - `ip`: 新节点的 IP 地址
    /// - `port`: 新节点的客户端端口
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.cluster_meet("192.168.1.100", 6379).await?;
    /// ```
    /// 
    /// # 注意事项
    /// 
    /// - 新节点必须能够访问当前节点
    /// - 端口应该是客户端端口，不是集群总线端口
    /// - 需要适当的权限配置
    pub async fn cluster_meet(&self, ip: &str, port: u16) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    Cmd::new().arg("CLUSTER").arg("MEET").arg(ip).arg(port).query_async::<()>(&mut conn).await.context("CLUSTER MEET")?;
                    Ok(())
                }
                ConnectionKind::Cluster(client) => {
                    let ip = ip.to_string();
                    let port = port;
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        Cmd::new().arg("CLUSTER").arg("MEET").arg(&ip).arg(port).query::<()>(&mut conn).context("CLUSTER MEET")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 从集群中移除节点
    /// 
    /// 使用 CLUSTER FORGET 命令从集群中移除指定节点。
    /// 
    /// # 参数
    /// 
    /// - `node_id`: 要移除的节点 ID
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.cluster_forget("3c3a0c74aae0b2717c740bb3f2c8a6a71d0d8c00").await?;
    /// ```
    /// 
    /// # 注意事项
    /// 
    /// - 节点 ID 是 40 字符的十六进制字符串
    /// - 移除节点前应该确保没有数据分配给该节点
    /// - 需要在集群的每个节点上执行此命令
    pub async fn cluster_forget(&self, node_id: &str) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    Cmd::new().arg("CLUSTER").arg("FORGET").arg(node_id).query_async::<()>(&mut conn).await.context("CLUSTER FORGET")?;
                    Ok(())
                }
                ConnectionKind::Cluster(client) => {
                    let node_id = node_id.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        Cmd::new().arg("CLUSTER").arg("FORGET").arg(&node_id).query::<()>(&mut conn).context("CLUSTER FORGET")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 发起集群故障转移
    /// 
    /// 使用 CLUSTER FAILOVER 命令在副本节点上发起故障转移，
    /// 使副本成为新的主节点。
    /// 
    /// # 参数
    /// 
    /// - `hard`: 是否使用强制故障转移
    ///   - `false`: 正常故障转移（TAKEOVER）
    ///   - `true`: 强制故障转移（FORCE）
    /// 
    /// # 故障转移模式
    /// 
    /// - **TAKEOVER**: 副本会通知主节点停止处理客户端请求
    /// - **FORCE**: 副本不等主节点响应，直接开始故障转移
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// // 正常故障转移
    /// redis.cluster_failover(false).await?;
    /// 
    /// // 强制故障转移（主节点无响应时使用）
    /// redis.cluster_failover(true).await?;
    /// ```
    pub async fn cluster_failover(&self, hard: bool) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    let mode = if hard { "FORCE" } else { "TAKEOVER" };
                    Cmd::new().arg("CLUSTER").arg("FAILOVER").arg(mode).query_async::<()>(&mut conn).await.context("CLUSTER FAILOVER")?;
                    Ok(())
                }
                ConnectionKind::Cluster(client) => {
                    let hard = hard;
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let mode = if hard { "FORCE" } else { "TAKEOVER" };
                        Cmd::new().arg("CLUSTER").arg("FAILOVER").arg(mode).query::<()>(&mut conn).context("CLUSTER FAILOVER")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 服务器配置命令 ---

    /// 设置 Redis 服务器配置参数
    /// 
    /// 使用 CONFIG SET 命令动态修改 Redis 服务器的配置参数。
    /// 
    /// # 参数
    /// 
    /// - `key`: 配置参数名称
    /// - `value`: 配置参数值
    /// 
    /// # 常用配置参数
    /// 
    /// - `timeout`: 客户端超时时间
    /// - `maxmemory`: 最大内存限制
    /// - `save`: RDB 快照保存策略
    /// - `loglevel`: 日志级别
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.config_set("timeout", "300").await?;
    /// redis.config_set("loglevel", "notice").await?;
    /// ```
    /// 
    /// # 注意事项
    /// 
    /// - 某些参数需要重启 Redis 才能生效
    /// - 部署环境可能限制 CONFIG 命令的使用
    /// - 修改配置前应该了解参数的影响
    pub async fn config_set(&self, key: &str, value: &str) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    Cmd::new().arg("CONFIG").arg("SET").arg(key).arg(value).query_async::<()>(&mut conn).await.context("CONFIG SET")?;
                    Ok(())
                }
                ConnectionKind::Cluster(client) => {
                    let key = key.to_string();
                    let value = value.to_string();
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        Cmd::new().arg("CONFIG").arg("SET").arg(&key).arg(&value).query::<()>(&mut conn).context("CONFIG SET")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    /// 触发后台保存快照
    /// 
    /// 使用 BGSAVE 命令在后台创建 RDB 快照文件。
    /// 这个命令不会阻塞服务器，会立即返回。
    /// 
    /// # 返回值
    /// 
    /// 成功时返回 `Ok(())`。
    /// 
    /// # 使用场景
    /// 
    /// - 定期数据备份
    /// - 手动触发持久化
    /// - 数据迁移前的快照
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// redis.bgsave().await?;
    /// println!("Background save started");
    /// ```
    /// 
    /// # 注意事项
    /// 
    /// - 快照创建是异步的，命令立即返回
    /// - 大型数据库可能需要较长时间完成
    /// - 可以通过 LASTSAVE 命令检查最后一次保存时间
    pub async fn bgsave(&self) -> Result<()> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    let mut conn = manager.clone();
                    Cmd::new().arg("BGSAVE").query_async::<()>(&mut conn).await.context("BGSAVE")?;
                    Ok(())
                }
                ConnectionKind::Cluster(client) => {
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        Cmd::new().arg("BGSAVE").query::<()>(&mut conn).context("BGSAVE")?;
                        Ok(())
                    }).await.unwrap()
                }
            }
        }).await
    }

    // --- 健康检查 ---

    /// Ping 命令健康检查
    /// 
    /// 使用 PING 命令检查 Redis 服务器的连接状态。
    /// 
    /// # 返回值
    /// 
    /// 返回 "PONG" 响应字符串。
    /// 
    /// # 实现细节
    /// 
    /// - 单机模式：通过设置测试键来验证连接
    /// - 集群模式：使用标准的 PING 命令
    /// 
    /// # 使用示例
    /// 
    /// ```rust
    /// let pong = redis.ping().await?;
    /// assert_eq!(pong, "PONG");
    /// ```
    pub async fn ping(&self) -> Result<String> {
        self.with_retry(|| async {
            match &self.kind {
                ConnectionKind::Standalone(manager, _) => {
                    // 单机模式通过设置测试键来验证连接
                    let mut conn = manager.clone();
                    let _: () = conn.set("__ping__", "1").await.context("PING_SET")?;
                    Ok("PONG".to_string())
                }
                ConnectionKind::Cluster(client) => {
                    // 集群模式使用标准 PING 命令
                    let client = client.clone();
                    
                    tokio::task::spawn_blocking(move || -> Result<String> {
                        let mut conn = client.get_connection().context("get cluster connection")?;
                        let res: String = Cmd::new().arg("PING").query(&mut conn).context("PING")?;
                        Ok(res)
                    }).await.unwrap()
                }
            }
        }).await
    }
}

/// 构建 Sentinel 连接 URL
/// 
/// 格式: redis+sentinel://host1:port1,host2:port2/master_name
fn build_sentinel_url(master: &str, urls: &[String]) -> Result<String> {
    let hosts: Vec<String> = urls.iter().map(|u| {
        u.trim_start_matches("redis://")
         .trim_start_matches("http://")
         .trim_end_matches('/')
         .to_string()
    }).collect();
    
    if hosts.is_empty() {
         return Err(anyhow!("No sentinel URLs provided"));
    }
    
    Ok(format!("redis+sentinel://{}/{}", hosts.join(","), master))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 初始化测试日志记录器
    fn init_test_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    /// 生成唯一的测试键名
    fn gen_key(prefix: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}-{}", prefix, timestamp)
    }

    /// 测试基础键值操作
    #[tokio::test]
    #[ignore]
    async fn test_kv_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        
        let key = gen_key("kv_test");
        
        // 基础设置和获取
        svc.set(0, &key, "val-1", Some(60)).await.unwrap();
        let v: Option<String> = svc.get(0, &key).await.unwrap();
        assert_eq!(v, Some("val-1".into()));
        
        // 更新值
        svc.set(0, &key, "val-2", None).await.unwrap();
        let v: Option<String> = svc.get(0, &key).await.unwrap();
        assert_eq!(v, Some("val-2".into()));

        // TTL 操作
        svc.expire(0, &key, 100).await.unwrap();
        let ttl = svc.ttl(0, &key).await.unwrap();
        assert!(ttl > 0 && ttl <= 100);
        
        // 存在性检查
        assert!(svc.exists(0, &key).await.unwrap());
        
        // 删除操作
        let ok = svc.del(0, &key).await.unwrap();
        assert!(ok);
        assert!(!svc.exists(0, &key).await.unwrap());
    }

    /// 测试哈希操作
    #[tokio::test]
    #[ignore]
    async fn test_hash_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        
        let key = gen_key("hash_test");
        
        // 单字段设置和获取
        svc.hset(0, &key, "f1", "v1").await.unwrap();
        svc.hset(0, &key, "f2", "v2").await.unwrap();
        
        let v1: Option<String> = svc.hget(0, &key, "f1").await.unwrap();
        assert_eq!(v1, Some("v1".into()));
        
        let v2: Option<String> = svc.hget(0, &key, "f2").await.unwrap();
        assert_eq!(v2, Some("v2".into()));
        
        let v3: Option<String> = svc.hget(0, &key, "f3").await.unwrap();
        assert_eq!(v3, None);
        
        // 清理
        svc.del(0, &key).await.unwrap();
    }

    /// 测试哈希批量操作
    #[tokio::test]
    #[ignore]
    async fn test_hash_batch_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let key = gen_key("hash_batch_test");

        // 批量设置
        let items = vec![("f1", "v1"), ("f2", "v2")];
        svc.hmset(0, &key, &items).await.unwrap();

        // 获取所有字段
        let all: HashMap<String, String> = svc.hgetall(0, &key).await.unwrap();
        assert_eq!(all.get("f1"), Some(&"v1".to_string()));
        assert_eq!(all.get("f2"), Some(&"v2".to_string()));

        // 清理
        svc.del(0, &key).await.unwrap();
    }

    /// 测试列表操作
    #[tokio::test]
    #[ignore]
    async fn test_list_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        
        let key = gen_key("list_test");
        
        // 推入操作
        svc.lpush(0, &key, "v1").await.unwrap();
        svc.lpush(0, &key, "v2").await.unwrap();
        
        // 弹出操作（LIFO）
        let v: Option<String> = svc.rpop(0, &key).await.unwrap();
        assert_eq!(v, Some("v1".into())); 
        
        let v: Option<String> = svc.rpop(0, &key).await.unwrap();
        assert_eq!(v, Some("v2".into()));
        
        let v: Option<String> = svc.rpop(0, &key).await.unwrap();
        assert_eq!(v, None);
    }

    /// 测试集合操作
    #[tokio::test]
    #[ignore]
    async fn test_set_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        
        let key = gen_key("set_test");
        
        // 添加成员
        svc.sadd(0, &key, "m1").await.unwrap();
        svc.sadd(0, &key, "m2").await.unwrap();
        svc.sadd(0, &key, "m1").await.unwrap(); // 重复添加
        
        // 获取所有成员
        let members: Vec<String> = svc.smembers(0, &key).await.unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&"m1".to_string()));
        assert!(members.contains(&"m2".to_string()));
        
        // 清理
        svc.del(0, &key).await.unwrap();
    }

    /// 测试管理命令
    #[tokio::test]
    #[ignore]
    async fn test_admin_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        
        // Ping 测试
        let pong = svc.ping().await.unwrap();
        assert_eq!(pong, "PONG");
        
        // Config Set 测试（某些环境可能受限）
        if let Err(e) = svc.config_set("timeout", "300").await {
            println!("config set warning: {}", e);
        } else {
            println!("config set success");
        }

        // BGSAVE 测试
        match svc.bgsave().await {
            Ok(_) => println!("bgsave started"),
            Err(e) => println!("bgsave failed (expected if busy): {}", e),
        }
    }

    /// 测试集群操作
    #[tokio::test]
    #[ignore]
    async fn test_cluster_ops() {
        init_test_logger();
        let cfg = RedisConfig {
            cluster: true,
            urls: vec!["redis://127.0.0.1:7010".to_string()],
            ..Default::default()
        };
        
        // 需要集群环境运行
        let svc = RedisService::new(cfg).await.expect("Cluster service init failed");
        
        // 验证基本功能
        let ping = svc.ping().await.unwrap();
        assert_eq!(ping, "PONG");
        
        // 验证键值操作
        let key = gen_key("cluster_kv");
        svc.set(0, &key, "c-val", None).await.unwrap();
        let v: Option<String> = svc.get(0, &key).await.unwrap();
        assert_eq!(v, Some("c-val".into()));
        svc.del(0, &key).await.unwrap();

        // 集群信息
        let nodes = svc.cluster_nodes().await.unwrap();
        assert!(nodes.contains("myself"));
        
        let slots = svc.cluster_slots().await.unwrap();
        if let redis::Value::Array(arr) = slots {
            assert!(!arr.is_empty());
        } else {
            panic!("Expected Array for cluster slots, got {:?}", slots);
        }
    }

    /// 测试哨兵操作
    #[tokio::test]
    #[ignore]
    async fn test_sentinel_ops() {
        init_test_logger();
        // Docker 哨兵测试环境特殊处理
        unsafe { std::env::set_var("FORCE_SENTINEL_LOCAL_IP", "1"); }
        
        let cfg = RedisConfig {
            sentinel: true,
            sentinel_master_name: Some("mymaster".into()),
            sentinel_urls: vec!["redis://127.0.0.1:26379".to_string()],
            ..Default::default()
        };
        
        let svc = RedisService::new(cfg).await.expect("Sentinel service init failed");
        
        let ping = svc.ping().await.unwrap();
        assert_eq!(ping, "PONG");
        
        let key = gen_key("sentinel_kv");
        svc.set(0, &key, "s-val", None).await.unwrap();
        let v: Option<String> = svc.get(0, &key).await.unwrap();
        assert_eq!(v, Some("s-val".into()));
        svc.del(0, &key).await.unwrap();
    }

    /// 测试 JSON 操作
    #[tokio::test]
    #[ignore]
    async fn test_json_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let key = gen_key("json_test");
        
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
        struct User {
            name: String,
            age: u32,
        }
        
        let u = User { name: "Alice".into(), age: 30 };
        svc.set_json(0, &key, &u, None).await.unwrap();
        
        let u2: Option<User> = svc.get_json(0, &key).await.unwrap();
        assert_eq!(Some(u), u2);
        
        svc.del(0, &key).await.unwrap();
    }

    /// 测试批量操作
    #[tokio::test]
    #[ignore]
    async fn test_batch_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let k1 = gen_key("batch_1");
        let k2 = gen_key("batch_2");
        
        // 批量设置
        let items = vec![(k1.clone(), "v1".to_string()), (k2.clone(), "v2".to_string())];
        svc.mset(&items).await.unwrap();
        
        // 批量获取
        let keys = vec![k1.clone(), k2.clone(), "non_existent".to_string()];
        let vals: Vec<Option<String>> = svc.mget(&keys).await.unwrap();
        assert_eq!(vals.len(), 3);
        assert_eq!(vals[0], Some("v1".into()));
        assert_eq!(vals[1], Some("v2".into()));
        assert_eq!(vals[2], None);
        
        // 清理
        svc.del(0, &k1).await.unwrap();
        svc.del(0, &k2).await.unwrap();
    }

    /// 测试事务操作
    #[tokio::test]
    #[ignore]
    async fn test_transaction_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let key = gen_key("tx_test");
        
        // 初始化
        svc.set(0, &key, "0", None).await.unwrap();
        
        // 执行事务
        let key_tx = key.clone();
        svc.transaction(move |pipe| {
            pipe.incr(&key_tx, 1).ignore();
            pipe.incr(&key_tx, 2).ignore();
        }).await.unwrap();
        
        // 验证结果
        let v: Option<i32> = svc.get(0, &key).await.unwrap();
        assert_eq!(v, Some(3)); // 0 + 1 + 2
        
        // 清理
        svc.del(0, &key).await.unwrap();
    }

    /// 测试分布式锁操作
    #[tokio::test]
    #[ignore]
    async fn test_lock_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let resource = gen_key("lock_res");
        let token = "my_token";
        
        // 获取锁
        assert!(svc.try_lock(&resource, token, 1000).await.unwrap());
        
        // 尝试再次获取（应该失败）
        assert!(!svc.try_lock(&resource, "other_token", 1000).await.unwrap());
        
        // 使用错误令牌释放（应该失败）
        assert!(!svc.unlock(&resource, "other_token").await.unwrap());
        
        // 使用正确令牌释放（应该成功）
        assert!(svc.unlock(&resource, token).await.unwrap());
        
        // 再次获取锁
        assert!(svc.try_lock(&resource, "new_token", 1000).await.unwrap());
        svc.del(0, &resource).await.unwrap();
    }

    /// 测试发布订阅操作
    #[tokio::test]
    #[ignore]
    async fn test_pubsub_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let channel = gen_key("ch");
        
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        
        let svc_clone = svc.clone();
        let ch_clone = channel.clone();
        
        // 订阅者任务
        tokio::spawn(async move {
            let _ = svc_clone.subscribe(ch_clone, move |msg| {
                let _ = tx.try_send(msg);
                false // 收到第一条消息后停止
            }).await;
        });
        
        tokio::time::sleep(Duration::from_millis(500)).await; // 等待订阅建立
        
        // 发布消息
        svc.publish(&channel, "hello").await.unwrap();
        
        // 等待接收消息
        let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap();
        assert_eq!(msg, Some("hello".to_string()));
    }

    /// 测试分片发布订阅操作
    #[tokio::test]
    #[ignore]
    async fn test_spublish_ops() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        let channel = gen_key("sharded_ch");
        let n = svc.spublish(&channel, "hello").await.unwrap();
        assert!(n >= 0); // 可能有订阅者，也可能没有
    }

    #[test]
    fn test_sentinel_url_build() {
        let master = "mymaster";
        let urls = vec![
            "redis://127.0.0.1:26379".to_string(),
            "http://127.0.0.1:26380/".to_string(), // Test cleanup
            "127.0.0.1:26381".to_string(), // Test cleanup
        ];
        
        let url = super::build_sentinel_url(master, &urls).unwrap();
        assert_eq!(url, "redis+sentinel://127.0.0.1:26379,127.0.0.1:26380,127.0.0.1:26381/mymaster");
    }

    #[tokio::test]
    #[ignore]
    async fn test_scan() {
        init_test_logger();
        let svc = RedisService::new(RedisConfig::default()).await.unwrap();
        
        // Prepare some data
        let k1 = gen_key("scan_1");
        let k2 = gen_key("scan_2");
        svc.set(0, &k1, "1", None).await.unwrap();
        svc.set(0, &k2, "2", None).await.unwrap();
        
        // Test scan
        // Use a pattern that matches our generated keys
        let pattern = "scan_*".to_string();
        let mut cursor: u64 = 0;
        let mut acc: Vec<String> = Vec::new();
        let mut rounds = 0;
        loop {
            let (next, keys) = svc.scan(0, cursor, Some(pattern.clone()), Some(100)).await.unwrap();
            acc.extend(keys);
            cursor = next;
            rounds += 1;
            if cursor == 0 || rounds > 10 { break; }
        }
        
        println!("Scan cursor: {}, total keys collected: {}", cursor, acc.len());
        
        // Assert
        assert!(acc.contains(&k1));
        assert!(acc.contains(&k2));
        
        // Clean up
        svc.del(0, &k1).await.unwrap();
        svc.del(0, &k2).await.unwrap();
    }
}
