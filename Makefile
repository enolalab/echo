.PHONY: local dev-web build test test-rust test-ui install

# CUDA toolkit present → compile whisper-rs GPU backend (tauri accepts -f/--features)
NVCC := $(shell command -v nvcc 2>/dev/null)
TAURI_FEATURES := $(if $(NVCC),--features cuda)

# Desktop app in dev mode (Vite + Rust hot reload, native window)
local:
	npm run dev -- $(TAURI_FEATURES)

# Browser-only dev server with mocked IPC (no Rust build)
dev-web:
	npm --prefix ui run dev

# Production frontend build + desktop bundle
build:
	npm run build:app -- $(TAURI_FEATURES)

test: test-rust test-ui

test-rust:
	cargo test --workspace

test-ui:
	npm --prefix ui test

# Install JS deps for root + ui workspaces
install:
	npm install
	npm --prefix ui install
