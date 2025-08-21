# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Docker UI management application built with Rust and Slint UI framework. The application provides a graphical interface for managing Docker containers, images, networks, and volumes both locally and remotely via SSH connections.

## Development Commands

### Build and Run
```bash
# Development mode
make dev
cargo run

# Development with auto-reload
make watch
cargo watch -c -x run

# Release build
make build
cargo build --release

# Full release process (check, test, build, package)
make release
```

### Code Quality
```bash
# Run all checks and linting
make check
cargo check
cargo clippy -- -D warnings

# Format code
make fmt
cargo fmt

# Run tests
make test
cargo test
```

### Packaging
```bash
# Build .deb package (Ubuntu/Debian)
make deb
./build-deb.sh

# Build .rpm package (openSUSE)
make rpm
./build-rpm.sh

# Install locally
make install        # .deb
make install-rpm    # .rpm
```

### System Dependencies
```bash
# Auto-detect and install dependencies
make deps

# Setup complete development environment
make setup
```

## Architecture

### Core Components

- **main.rs** - Application entry point, state management, and main event loop
- **ui.rs** - UI callback setup and interface integration
- **chart.rs** - Real-time chart rendering for metrics visualization

### Docker Management
- **docker/mod.rs** - Local Docker API client and statistics collection
- **docker/types.rs** - Docker-related type definitions
- **list_containers.rs** - Container management operations
- **list_images.rs** - Image management operations  
- **list_networks.rs** - Network management operations
- **list_volumes.rs** - Volume management operations

### SSH Remote Management
- **ssh/mod.rs** - SSH module exports
- **ssh/client.rs** - SSH client implementation for remote connections
- **ssh/error.rs** - SSH error handling
- **ssh/types.rs** - SSH-related type definitions
- **ssh_persistence.rs** - SSH server configuration persistence (saves to `ssh_servers.json`)
- **ssh_ui_integration.rs** - SSH UI integration and state management

### UI Structure (Slint Components)
- **ui/app.slint** - Main application window and navigation
- **ui/dashboard.slint** - Real-time metrics dashboard with charts
- **ui/containers.slint** - Container list and management interface
- **ui/container-details.slint** - Individual container details view
- **ui/create-container.slint** - Container creation form
- **ui/images.slint** - Docker images management
- **ui/network.slint** - Network management interface
- **ui/volumes.slint** - Volume management interface
- **ui/ssh-servers.slint** - SSH server configuration interface
- **ui/ssh-simple.slint** - Simplified SSH connection view
- **ui/notification.slint** - Notification system component

## Key Dependencies

- **slint** (1.6) - UI framework
- **tokio** - Async runtime with full features
- **bollard** (0.19.2) - Docker API client
- **ssh2** (0.9) - SSH client library
- **plotters** (0.3) - Chart rendering
- **serde/serde_json** - JSON serialization for SSH config persistence
- **chrono** - Date/time handling
- **uuid** - Unique ID generation

## Development Patterns

### State Management
- Global app state stored in `AppState` struct with `Arc<Mutex<>>` for thread safety
- Chart data maintained in `ChartData` with `VecDeque` for efficient point management
- SSH state managed through `SshUiState` for connection tracking

### UI Integration
- Slint components are modular and imported into main `app.slint`
- Callbacks set up in `ui.rs` using `setup_global_callbacks()`
- Real-time updates via timer-based polling every second

### Remote Management
- SSH connections enable full Docker management on remote servers
- Automatic toggle between local and remote modes based on connection status
- SSH server configurations persisted to `ssh_servers.json`

## Build System

The project uses a comprehensive Makefile with:
- Automated dependency detection (zypper for openSUSE, apt for Ubuntu/Debian)
- Cross-platform package building (.deb and .rpm)
- Build artifact management with versioning
- Development workflow automation

Generated packages are stored in `builds/` directory with automatic cleanup scripts.

## Testing

```bash
# Run unit tests
cargo test

# Run specific example
cargo run --example ssh_test
cargo run --example ssh_monitor
cargo run --example remote_docker_test
```

## Configuration Files

- **Cargo.toml** - Rust project configuration with dependencies
- **build.rs** - Build script for Slint UI compilation
- **Makefile** - Build automation and development workflow
- **docker-ui.spec.template** - RPM package specification template
- **ssh_servers.json** - Auto-generated SSH server configurations (created at runtime)