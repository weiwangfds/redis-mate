# RedisMate

<div align="center">

![RedisMate Logo](https://img.shields.io/badge/RedisMate-2.0-red?style=for-the-badge&logo=redis)
![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=for-the-badge)
![Version](https://img.shields.io/badge/version-0.1.0-orange?style=for-the-badge)

**A modern, lightweight, and cross-platform Redis desktop client built with Tauri and React**

[Download Release](https://github.com/weiwangfds/redis-mate/releases) · [Report Bug](https://github.com/weiwangfds/redis-mate/issues) · [Request Feature](https://github.com/weiwangfds/redis-mate/issues/new)

</div>

## Table of Contents

- [About](#about)
- [Features](#features)
- [Screenshots](#screenshots)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## About

**RedisMate** is a high-performance, cross-platform Redis client desktop application that combines the power of Rust's Tauri framework with React's modern UI capabilities. It provides a native-feeling experience while maintaining the performance benefits of Rust for Redis operations.

RedisMate supports all major Redis deployment modes including Standalone, Sentinel, and Cluster configurations, making it suitable for development, testing, and production environments.

### Why RedisMate?

- **🚀 Performance**: Built with Tauri for native performance and minimal memory footprint
- **🎨 Modern UI**: Clean, intuitive interface built with React and Tailwind CSS
- **🌍 Cross-Platform**: One codebase for Windows, macOS, and Linux
- **🔧 Full-Featured**: Complete Redis operations support including Pub/Sub, Clustering, and all data types
- **🌐 Internationalization**: English and Chinese language support
- **⚡ Real-time**: Live updates for Pub/Sub messages and cluster monitoring

## Features

### 🔗 Connection Management
- **Multiple Redis Modes**: Support for Standalone, Sentinel, and Cluster deployments
- **Secure Authentication**: Password protection and TLS support
- **Connection Testing**: Validate connections before saving
- **Connection Health**: PING command to monitor connection status
- **Persistent Storage**: Save and organize multiple connections

### 🔑 Key Operations
- **All Data Types**: String, Hash, List, Set, Sorted Set, JSON (RedisJSON)
- **TTL Management**: Set, view, and persist key expiration times
- **Batch Operations**: MGET, MSET for bulk operations
- **Key Scanning**: SCAN command with pattern matching
- **Visual Editor**: Intuitive interface for editing different data types

### 📡 Advanced Features
- **Pub/Sub Support**: Subscribe to channels and publish messages in real-time
- **Cluster Management**: View cluster topology, nodes, and slots distribution
- **Distributed Locks**: Atomic lock/unlock operations
- **Database Switching**: Support for multiple Redis databases (0-15)
- **JSON Operations**: Native RedisJSON module support

### 🎨 User Experience
- **Bilingual Support**: English and Chinese (Simplified) languages
- **Dark Mode**: Easy on the eyes during long sessions
- **Responsive Design**: Adapts to different screen sizes
- **Keyboard Shortcuts**: Productivity-enhancing shortcuts
- **Auto-Updates**: Built-in update mechanism

## Screenshots

<div align="center">
  <img src="https://via.placeholder.com/800x500/1e1e1e/ffffff?text=Connection+Management" alt="Connection Management" width="45%">
  <img src="https://via.placeholder.com/800x500/1e1e1e/ffffff?text=Key+Operations" alt="Key Operations" width="45%">
</div>

<div align="center">
  <img src="https://via.placeholder.com/800x500/1e1e1e/ffffff?text=Pub/Sub+Interface" alt="Pub/Sub Interface" width="45%">
  <img src="https://via.placeholder.com/800x500/1e1e1e/ffffff?text=Cluster+Info" alt="Cluster Information" width="45%">
</div>

## Installation

### Pre-built Binaries (Recommended)

Download the latest release from the [GitHub Releases](https://github.com/weiwangfds/redis-mate/releases) page:

- **Windows**: `redis-mate-x86_64.msi`
- **macOS**: `redis-mate-aarch64.dmg` (Apple Silicon) or `redis-mate-x86_64.dmg` (Intel)
- **Linux**: `redis-mate.AppImage` (universal) or `.deb`/`.rpm` packages

### Build from Source

#### Prerequisites

1. **Node.js** (LTS version recommended)
2. **Rust** (latest stable version)
3. **Tauri CLI**
4. **pnpm** package manager

#### Setup Steps

1. **Clone the repository**
   ```bash
   git clone https://github.com/weiwangfds/redis-mate.git
   cd redis-mate
   ```

2. **Install Rust**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

3. **Install Node.js**
   ```bash
   # Using nvm (recommended)
   nvm install --lts
   nvm use --lts
   ```

4. **Install Tauri CLI**
   ```bash
   cargo install tauri-cli
   ```

5. **Install pnpm**
   ```bash
   npm install -g pnpm
   ```

6. **Install dependencies**
   ```bash
   pnpm install
   ```

7. **Run in development mode**
   ```bash
   pnpm tauri dev
   ```

8. **Build for production**
   ```bash
   pnpm tauri build
   ```

The built application will be in the `src-tauri/target/release/bundle/` directory.

## Usage

### Getting Started

1. **Launch RedisMate** from your applications menu or by running the executable
2. **Add a Redis Connection**:
   - Click the "Add Connection" button
   - Select connection type (Standalone, Sentinel, or Cluster)
   - Enter connection details
   - Test connection and save

### Connection Types

#### Standalone Redis
```bash
# Basic connection
redis://localhost:6379

# With password
redis://:password@localhost:6379

# With database
redis://localhost:6379/1
```

#### Redis Sentinel
```
Master Name: mymaster
Sentinel Nodes:
- redis://sentinel1:26379
- redis://sentinel2:26379
```

#### Redis Cluster
```
Seed Nodes (one per line):
- redis://cluster-node1:6379
- redis://cluster-node2:6379
- redis://cluster-node3:6379
```

### Common Operations

#### Key Management
```javascript
// String operations
SET mykey "Hello Redis"
GET mykey

// Hash operations
HSET user:1001 name "John Doe" age 30
HGETALL user:1001

// List operations
LPUSH mylist "item1" "item2" "item3"
LRANGE mylist 0 -1

// TTL operations
EXPIRE mykey 3600
TTL mykey
PERSIST mykey
```

#### Pub/Sub
```javascript
// Subscribe to a channel
SUBSCRIBE notifications

// Publish a message
PUBLISH notifications "Hello World!"
```

## Configuration

### Application Settings

RedisMate stores configuration in:

- **Windows**: `%APPDATA%\redis-mate\`
- **macOS**: `~/Library/Application Support/redis-mate/`
- **Linux**: `~/.config/redis-mate/`

### Environment Variables

- `REDIS_LOG_LEVEL`: Set logging level (trace, debug, info, warn, error)
- `REDIS_MAX_CONNECTIONS`: Maximum concurrent Redis connections

### Docker Setup for Testing

Included Docker configurations for testing:

```bash
# Standalone Redis
docker-compose -f docker/standalone.yml up -d

# Redis Cluster
docker-compose -f docker/cluster.yml up -d

# Redis with Sentinel
docker-compose -f docker/sentinel.yml up -d
```

## Development

### Project Structure

```
redis-mate/
├── src/                     # Frontend source code
│   ├── components/          # React components
│   ├── locales/            # Translation files
│   └── utils/              # Utility functions
├── src-tauri/              # Tauri backend
│   ├── src/                # Rust source code
│   └── Cargo.toml          # Rust dependencies
├── docker/                 # Docker configurations
├── docs/                   # Documentation
└── README.md              # This file
```

### Available Scripts

```bash
# Install dependencies
pnpm install

# Development mode
pnpm tauri dev

# Build application
pnpm tauri build

# Run tests
pnpm test

# Lint code
pnpm lint

# Format code
pnpm format
```

### Architecture

The application follows a clean architecture pattern:

- **Frontend**: React + TypeScript with Tailwind CSS
- **Backend**: Rust with Tauri framework
- **IPC Communication**: Type-safe commands between frontend and backend
- **Database**: SQLite for storing connection configurations

### Adding New Features

1. **Frontend Components**: Add React components in `src/components/`
2. **Backend Commands**: Add new Tauri commands in `src-tauri/src/command.rs`
3. **Redis Operations**: Extend Redis service in `src-tauri/src/redis_service.rs`

## Contributing

We welcome contributions from the community! Whether it's a bug fix, new feature, or documentation improvement, we appreciate your help.

### How to Contribute

1. **Fork the Repository**
   ```bash
   git clone https://github.com/your-username/redis-mate.git
   ```

2. **Create a Feature Branch**
   ```bash
   git checkout -b feature/amazing-feature
   ```

3. **Make Your Changes**
   - Follow the existing code style and patterns
   - Add tests for new functionality
   - Update documentation as needed

4. **Run Tests**
   ```bash
   pnpm test
   ```

5. **Commit Your Changes**
   ```bash
   git commit -m "feat: add amazing feature"
   ```

6. **Push to Your Branch**
   ```bash
   git push origin feature/amazing-feature
   ```

7. **Open a Pull Request**
   - Provide a clear description of your changes
   - Link any relevant issues
   - Include screenshots if applicable

### Code Style Guidelines

- **TypeScript**: Use strict type checking
- **Rust**: Follow `rustfmt` and `clippy` recommendations
- **React**: Use functional components with hooks
- **CSS**: Use Tailwind CSS utility classes
- **Commits**: Follow [Conventional Commits](https://www.conventionalcommits.org/) specification

### Reporting Issues

When reporting bugs, please include:

- Operating system and version
- RedisMate version
- Steps to reproduce
- Expected behavior
- Actual behavior
- Screenshots if applicable

### Feature Requests

We love hearing ideas for new features! Please:

- Check if the feature already exists
- Search existing feature requests
- Provide a clear use case and description

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

### License Summary

- ✅ **Commercial use**: You can use this software in commercial products
- ✅ **Modification**: You can modify the source code
- ✅ **Distribution**: You can distribute the software
- ✅ **Private use**: You can use the software privately
- ⚠️ **Liability**: The software is provided "as is" without warranty
- ⚠️ **Copyright**: Must include original copyright and license notices

## Acknowledgments

- [Tauri](https://tauri.app/) - The amazing framework that makes this project possible
- [React](https://reactjs.org/) - The UI library for building user interfaces
- [Redis](https://redis.io/) - The incredible in-memory data structure store
- [Lucide Icons](https://lucide.dev/) - Beautiful and consistent icons
- All [contributors](https://github.com/weiwangfds/redis-mate/graphs/contributors) who help improve this project

---

<div align="center">

**Made with ❤️ by the RedisMate team**

[⭐ Star this repo](https://github.com/weiwangfds/redis-mate) · [🐛 Report issues](https://github.com/weiwangfds/redis-mate/issues) · [📖 Read documentation](https://github.com/weiwangfds/redis-mate/wiki)

</div>