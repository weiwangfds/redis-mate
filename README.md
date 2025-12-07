## Project Name

RedisMate

### Project Overview

Cross-platform Redis client built with Tauri + React. Supports Standalone, Sentinel and Cluster modes. Provides key management, Pub/Sub, cluster insights, connection health checks and i18n (English/Chinese).

1. Connection Management: add/edit/remove, password injection, health check (PING)
2. Key Management: view/edit String, Hash, List, Set, ZSet, JSON (RedisJSON), TTL management, batch ops
3. Pub/Sub: subscribe channels, publish messages, live message stream
4. Cluster Info: nodes, replicas, slots layout visualization
5. Internationalization: English by default, toggle in Settings

### Key Benefits

- Technical: Tauri native shell with React + TypeScript; faster and lighter than Electron
- Performance: native window integration, low memory footprint; async Redis ops
- Usability: consistent UI with toasts and modals; visual key editing and search
- Extensibility: modular IPC commands; easy to add Redis types and operations

### Quick Start

1. Install dependencies
   ```bash
   pnpm install
   ```
2. Development
   ```bash
   pnpm tauri dev
   ```
3. Build release
   ```bash
   pnpm tauri build
   ```

### Configuration

- Standalone: `redis://host:port`, optional password injection
- Cluster: paste multiple seed nodes, one per line `redis://host:port`
- Sentinel: set master name and sentinel nodes
- Test Connection: click “Test Connection” in the add/edit modal to validate

### Contributing

We welcome contributions:
1. Fork
2. Create feature branch
3. Submit PR

### License

MIT

### Badges

![License](https://img.shields.io/badge/license-MIT-green)
![Repo](https://img.shields.io/github/stars/weiwangfds/redis-mate)
![Issues](https://img.shields.io/github/issues/weiwangfds/redis-mate)

### Screenshots/Demos

