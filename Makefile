.PHONY: local dev-web build test test-rust test-ui install

# Desktop app in dev mode (Vite + Rust hot reload, native window)
local:
	npm run dev

# Browser-only dev server with mocked IPC (no Rust build)
dev-web:
	npm --prefix ui run dev

# Production frontend build + desktop bundle
build:
	npm run build:app

test: test-rust test-ui

test-rust:
	cargo test --workspace

test-ui:
	npm --prefix ui test

# Install JS deps for root + ui workspaces
install:
	npm install
	npm --prefix ui install
