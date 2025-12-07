# 中文版本 README

## 项目名称

RedisMate

### 项目简介

一个基于 Tauri + React 的跨平台 Redis 客户端，支持单机、哨兵与集群模式，提供键管理、发布订阅、集群信息查看、连接健康检查与多语言切换等核心能力。

1. 连接管理：支持新增、编辑、删除连接，支持密码注入与连接健康检查（PING）
2. 键管理：查看与编辑字符串、哈希、列表、集合、有序集合、JSON（RedisJSON）等类型，支持 TTL 管理与批量操作
3. 发布订阅：订阅频道与发布消息，实时查看消息流
4. 集群信息：展示节点、主从、槽位分布等拓扑信息
5. 国际化：内置中英文切换，默认英文，可在设置中更改

### 主要优势

- 技术优势：使用 Tauri 构建原生应用壳，前端 React + TypeScript；性能与体积优于传统 Electron
- 性能优势：原生窗口与系统资源集成，低内存占用；Redis 操作异步处理，响应实时
- 易用性：统一的 UI 组件与操作反馈（Toast、Modal），可视化键编辑与搜索
- 扩展性：模块化命令封装（前后端 IPC），方便新增 Redis 类型与操作

### 快速开始

1. 安装依赖
   ```bash
   pnpm install
   ```
2. 开发运行
   ```bash
   pnpm tauri dev
   ```
3. 构建发布
   ```bash
   pnpm tauri build
   ```

### 配置说明

- 单机模式：输入 `redis://host:port`，可选密码自动注入
- 集群模式：粘贴多行种子节点，每行一个 `redis://host:port`
- 哨兵模式：配置主节点名称与哨兵节点列表
- 测试连接：在新增/编辑连接对话框点击“测试连接”验证配置

### 贡献指南

欢迎贡献代码：
1. Fork 项目
2. 创建特性分支
3. 提交 Pull Request

### 许可证

MIT

### 徽章

![License](https://img.shields.io/badge/license-MIT-green)
![Repo](https://img.shields.io/github/stars/weiwangfds/redis-mate)
![Issues](https://img.shields.io/github/issues/weiwangfds/redis-mate)

### 截图/演示


