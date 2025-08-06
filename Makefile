# Makefile for Docker UI

.PHONY: help build clean dev release install uninstall deb list-builds clean-builds

# Variables
APP_NAME := docker-ui
BUILD_DIR := builds
DEB_SCRIPT := ./build-deb.sh
CLEAN_SCRIPT := ./clean-builds.sh

# Colors
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m

help: ## Show this help message
	@echo -e "$(BLUE)Docker UI Build System$(NC)\n"
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}'

build: ## Build application in release mode
	@echo -e "$(BLUE)Building application...$(NC)"
	cargo build --release

dev: ## Run application in development mode
	@echo -e "$(BLUE)Running in development mode...$(NC)"
	cargo run

watch: ## Run application in watch mode (requires cargo-watch)
	@echo -e "$(BLUE)Running in watch mode...$(NC)"
	@command -v cargo-watch >/dev/null 2>&1 || { \
		echo -e "$(YELLOW)Installing cargo-watch...$(NC)"; \
		cargo install cargo-watch; \
	}
	cargo watch -c -x run

clean: ## Clean build artifacts
	@echo -e "$(BLUE)Cleaning build artifacts...$(NC)"
	cargo clean

test: ## Run tests
	@echo -e "$(BLUE)Running tests...$(NC)"
	cargo test

check: ## Run cargo check and clippy
	@echo -e "$(BLUE)Running cargo check...$(NC)"
	cargo check
	@echo -e "$(BLUE)Running clippy...$(NC)"
	cargo clippy -- -D warnings

fmt: ## Format code
	@echo -e "$(BLUE)Formatting code...$(NC)"
	cargo fmt

deb: ## Build .deb package
	@echo -e "$(BLUE)Building .deb package...$(NC)"
	$(DEB_SCRIPT)

release: check test build deb ## Full release build (check, test, build, package)
	@echo -e "$(GREEN)Release build completed!$(NC)"

list-builds: ## List all builds
	$(CLEAN_SCRIPT) list

clean-builds: ## Clean old builds (keep 5 most recent)
	$(CLEAN_SCRIPT) clean

clean-all-builds: ## Clean all builds
	$(CLEAN_SCRIPT) clean-all

install: deb ## Install .deb package locally
	@echo -e "$(BLUE)Installing package...$(NC)"
	@latest_deb=$$(ls -t $(BUILD_DIR)/$(APP_NAME)_*.deb 2>/dev/null | head -1); \
	if [ -n "$$latest_deb" ]; then \
		echo -e "Installing: $$(basename "$$latest_deb")"; \
		sudo dpkg -i "$$latest_deb"; \
		echo -e "$(GREEN)Installation completed!$(NC)"; \
	else \
		echo -e "$(YELLOW)No .deb package found. Run 'make deb' first.$(NC)"; \
	fi

uninstall: ## Uninstall application
	@echo -e "$(BLUE)Uninstalling $(APP_NAME)...$(NC)"
	sudo dpkg -r $(APP_NAME) || true
	@echo -e "$(GREEN)Uninstall completed!$(NC)"

reinstall: uninstall install ## Reinstall application

deps: ## Install system dependencies
	@echo -e "$(BLUE)Installing system dependencies...$(NC)"
	sudo apt update
	sudo apt install -y build-essential pkg-config libfontconfig1-dev

setup: deps ## Setup development environment
	@echo -e "$(BLUE)Setting up development environment...$(NC)"
	@command -v rustc >/dev/null 2>&1 || { \
		echo -e "$(YELLOW)Installing Rust...$(NC)"; \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; \
		source ~/.cargo/env; \
	}
	@command -v cargo-watch >/dev/null 2>&1 || { \
		echo -e "$(YELLOW)Installing cargo-watch...$(NC)"; \
		cargo install cargo-watch; \
	}
	@echo -e "$(GREEN)Development environment ready!$(NC)"

docker-start: ## Start Docker daemon if not running
	@echo -e "$(BLUE)Checking Docker status...$(NC)"
	@sudo systemctl is-active docker >/dev/null 2>&1 || { \
		echo -e "$(YELLOW)Starting Docker daemon...$(NC)"; \
		sudo systemctl start docker; \
	}
	@echo -e "$(GREEN)Docker is running!$(NC)"

info: ## Show project information
	@echo -e "$(BLUE)Project Information:$(NC)"
	@echo "  Name: Docker UI"
	@echo "  Language: Rust + Slint"
	@echo "  Version: $$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/' | head -1)"
	@echo "  Build directory: $(BUILD_DIR)/"
	@echo ""
	@echo -e "$(BLUE)System Information:$(NC)"
	@echo "  OS: $$(uname -s)"
	@echo "  Architecture: $$(uname -m)"
	@echo "  Rust version: $$(rustc --version 2>/dev/null || echo 'Not installed')"
	@echo "  Cargo version: $$(cargo --version 2>/dev/null || echo 'Not installed')"
	@echo "  Docker version: $$(docker --version 2>/dev/null || echo 'Not installed')"

# Default target
all: help