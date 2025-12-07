//! 数据库管理模块
//! 
//! 本模块负责管理 SQLite 数据库，提供 Redis 连接配置的持久化存储功能。
//! 使用 sqlx 库进行异步数据库操作，支持编译时 SQL 检查。
//! 
//! # 功能特性
//! 
//! - **异步操作**：使用 tokio 异步运行时，不阻塞主线程
//! - **连接池**：使用 sqlx 连接池管理数据库连接
//! - **类型安全**：编译时 SQL 检查和类型推断
//! - **错误处理**：详细的错误上下文信息
//! - **自动迁移**：自动创建必要的数据库表结构
//! 
//! # 数据库表结构
//! 
//! ```sql
//! CREATE TABLE redis_configs (
//!     id INTEGER PRIMARY KEY,           -- 自增主键
//!     name TEXT NOT NULL UNIQUE,        -- 连接名称（唯一）
//!     config_json TEXT NOT NULL,        -- 配置信息的 JSON 字符串
//!     created_at DATETIME DEFAULT CURRENT_TIMESTAMP  -- 创建时间
//! );
//! ```
//! 
//! # 使用示例
//! 
//! ```rust
//! let db = DbManager::new("app.db").await?;
//! 
//! // 保存配置
//! let config = RedisConfig { /* ... */ };
//! db.save_config("my_redis", &config).await?;
//! 
//! // 读取配置
//! let config = db.get_config("my_redis").await?;
//! 
//! // 列出所有配置
//! let configs = db.list_configs().await?;
//! ```

use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;
use crate::redis_service::RedisConfig;

/// SQLite 数据库管理器
/// 
/// 负责管理与 Redis 连接配置相关的所有数据库操作。
/// 使用连接池来提高性能，支持并发访问。
/// 
/// # 设计特点
/// 
/// - **连接池管理**：使用 `SqlitePool` 管理多个数据库连接
/// - **异步操作**：所有数据库操作都是异步的，不会阻塞事件循环
/// - **类型安全**：使用 sqlx 的编译时 SQL 检查
/// - **自动初始化**：创建时自动建立必要的表结构
/// 
/// # 配置说明
/// 
/// - **最大连接数**：设置为 5，适合桌面应用程序的使用场景
/// - **数据库模式**：使用 `rwc` 模式（读/写/创建），支持数据库文件不存在时的自动创建
/// 
/// # 字段
/// 
/// - `pool`: sqlx 连接池实例，用于执行数据库操作
pub struct DbManager {
    /// SQLx SQLite 连接池
    /// 
    /// 管理到 SQLite 数据库的连接池，支持并发访问和连接复用。
    /// 连接池的大小限制为 5 个连接，适合桌面应用的使用场景。
    pool: Pool<Sqlite>,
}

impl DbManager {
    /// 创建新的数据库管理器实例
    /// 
    /// 初始化数据库连接池并创建必要的数据表结构。
    /// 如果数据库文件不存在，会自动创建。
    /// 
    /// # 参数
    /// 
    /// - `db_path`: 数据库文件的路径，可以是相对路径或绝对路径
    /// 
    /// # 返回值
    /// 
    /// 返回初始化完成的 `DbManager` 实例。
    /// 
    /// # 错误处理
    /// 
    /// 可能的错误包括：
    /// - 数据库文件权限问题
    /// - 磁盘空间不足
    /// - SQL 语法错误（在表创建时）
    /// - 连接池配置错误
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let db = DbManager::new("app.db").await?;
    /// let db = DbManager::new("/path/to/database.db").await?;
    /// ```
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        // 转换路径为字符串
        let path_str = db_path.as_ref().to_str().unwrap();
        
        // 构建数据库连接 URL
        // mode=rwc: 读/写/创建模式，如果文件不存在会自动创建
        let url = format!("sqlite://{}?mode=rwc", path_str);
        
        // 创建连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)  // 最大连接数设置为 5
            .connect(&url)
            .await?;

        // 创建管理器实例
        let manager = Self { pool };
        
        // 初始化数据库结构
        manager.init().await?;
        
        Ok(manager)
    }

    /// 初始化数据库结构
    /// 
    /// 创建应用程序所需的数据表。使用 `IF NOT EXISTS` 语句，
    /// 确保在数据库已存在的情况下不会报错。
    /// 
    /// # 执行的 SQL
    /// 
    /// ```sql
    /// CREATE TABLE IF NOT EXISTS redis_configs (
    ///     id INTEGER PRIMARY KEY,
    ///     name TEXT NOT NULL UNIQUE,
    ///     config_json TEXT NOT NULL,
    ///     created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    /// )
    /// ```
    /// 
    /// # 表字段说明
    /// 
    /// - `id`: 自增主键，用于唯一标识每条记录
    /// - `name`: 连接名称，用户友好的标识符，必须唯一
    /// - `config_json`: Redis 配置的 JSON 序列化字符串
    /// - `created_at`: 记录创建时间，默认为当前时间戳
    /// 
    /// # 错误处理
    /// 
    /// 如果表创建失败，会返回详细的错误信息。
    async fn init(&self) -> Result<()> {
        sqlx::query!(
            r#"
            CREATE TABLE IF NOT EXISTS redis_configs (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                config_json TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 保存 Redis 连接配置
    /// 
    /// 将 Redis 连接配置保存到数据库中。如果配置已存在，
    /// 会更新现有记录；如果不存在，会创建新记录。
    /// 
    /// # UPSERT 操作
    /// 
    /// 使用 SQLite 的 `INSERT ... ON CONFLICT ... DO UPDATE` 语法实现 UPSERT 功能：
    /// - 如果 `name` 不存在：插入新记录
    /// - 如果 `name` 已存在：更新 `config_json` 字段
    /// 
    /// # 参数
    /// 
    /// - `name`: 连接的名称，用作唯一标识符
    /// - `config`: Redis 连接配置对象
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let config = RedisConfig {
    ///     urls: vec!["redis://localhost:6379".to_string()],
    ///     cluster: false,
    ///     ..Default::default()
    /// };
    /// db.save_config("local_redis", &config).await?;
    /// ```
    pub async fn save_config(&self, name: &str, config: &RedisConfig) -> Result<()> {
        // 将配置对象序列化为 JSON 字符串
        let json = serde_json::to_string(config)?;
        
        // 执行 UPSERT 操作
        sqlx::query!(
            r#"
            INSERT INTO redis_configs (name, config_json) 
            VALUES (?, ?)
            ON CONFLICT(name) DO UPDATE SET config_json = excluded.config_json
            "#,
            name,
            json
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 获取指定名称的 Redis 配置
    /// 
    /// 从数据库中查找指定名称的 Redis 连接配置。
    /// 
    /// # 参数
    /// 
    /// - `name`: 要查找的配置名称
    /// 
    /// # 返回值
    /// 
    /// - `Some(RedisConfig)`: 找到对应的配置
    /// - `None`: 没有找到指定名称的配置
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// if let Some(config) = db.get_config("my_redis").await? {
    ///     println!("Found config: {:?}", config.urls);
    /// } else {
    ///     println!("Config not found");
    /// }
    /// ```
    pub async fn get_config(&self, name: &str) -> Result<Option<RedisConfig>> {
        let row = sqlx::query!(
            "SELECT config_json FROM redis_configs WHERE name = ?", 
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            // 反序列化 JSON 字符串为 RedisConfig 对象
            let config: RedisConfig = serde_json::from_str(&r.config_json)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    /// 获取所有 Redis 配置列表
    /// 
    /// 返回数据库中存储的所有 Redis 连接配置，按名称排序。
    /// 
    /// # 返回值
    /// 
    /// 返回一个元组向量，每个元组包含：
    /// - 配置名称 (String)
    /// - 对应的 Redis 配置对象 (RedisConfig)
    /// 
    /// # 排序
    /// 
    /// 结果按配置名称的字母顺序排序，便于用户查找。
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let configs = db.list_configs().await?;
    /// for (name, config) in configs {
    ///     println!("Config '{}' has {} URLs", name, config.urls.len());
    /// }
    /// ```
    pub async fn list_configs(&self) -> Result<Vec<(String, RedisConfig)>> {
        let rows = sqlx::query!(
            "SELECT name, config_json FROM redis_configs ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            // 反序列化每个配置
            let config: RedisConfig = serde_json::from_str(&row.config_json)?;
            result.push((row.name, config));
        }
        Ok(result)
    }

    /// 删除指定的 Redis 配置
    /// 
    /// 从数据库中删除指定名称的 Redis 连接配置。
    /// 
    /// # 参数
    /// 
    /// - `name`: 要删除的配置名称
    /// 
    /// # 返回值
    /// 
    /// - `true`: 成功删除了一条记录
    /// - `false`: 没有找到要删除的记录
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let deleted = db.delete_config("old_config").await?;
    /// if deleted {
    ///     println!("Config successfully deleted");
    /// } else {
    ///     println!("Config was not found");
    /// }
    /// ```
    pub async fn delete_config(&self, name: &str) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM redis_configs WHERE name = ?", 
            name
        )
        .execute(&self.pool)
        .await?;
        
        // 检查是否影响了行数
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    
    /// 测试数据库的基本 CRUD 操作
    /// 
    /// 测试流程：
    /// 1. 创建临时数据库
    /// 2. 保存配置
    /// 3. 读取配置
    /// 4. 列出所有配置
    /// 5. 更新配置
    /// 6. 删除配置
    /// 7. 清理测试文件
    #[tokio::test]
    async fn test_db_ops() {
        // 使用临时数据库文件
        let db_path = "test_redis_config.db";
        
        // 清理之前可能存在的测试文件
        let _ = fs::remove_file(db_path);

        // 创建数据库管理器
        let db = DbManager::new(db_path).await.unwrap();

        // 创建测试配置
        let cfg = RedisConfig {
            urls: vec!["redis://localhost:6379".into()],
            cluster: false,
            pool_size: 10,
            ..Default::default()
        };

        // 测试：保存配置
        db.save_config("local", &cfg).await.unwrap();

        // 测试：读取配置
        let saved = db.get_config("local").await.unwrap().unwrap();
        assert_eq!(saved.urls, cfg.urls);
        assert_eq!(saved.pool_size, 10);

        // 测试：列出配置
        let list = db.list_configs().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "local");

        // 测试：更新配置
        let mut cfg2 = cfg.clone();
        cfg2.pool_size = 20;
        db.save_config("local", &cfg2).await.unwrap();
        
        let saved2 = db.get_config("local").await.unwrap().unwrap();
        assert_eq!(saved2.pool_size, 20);

        // 测试：删除配置
        let deleted = db.delete_config("local").await.unwrap();
        assert!(deleted);

        // 验证删除后的状态
        let list = db.list_configs().await.unwrap();
        assert!(list.is_empty());

        // 清理测试文件
        let _ = fs::remove_file(db_path);
    }
}