# Contributing to Ossue

Thanks for your interest in contributing! This guide covers everything you need to get started.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/) (v10+)
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/) (`cargo install tauri-cli --version "^2"`)

## Development Setup

```bash
# Clone the repository
git clone https://github.com/kaplanelad/ossue.git
cd ossue

# Install frontend dependencies
pnpm install

# Run the app in development mode
pnpm run dev
```

This starts the Vite dev server with HMR (frontend) and the Tauri Rust backend with auto-rebuild on changes.

## Architecture

Ossue is a [Tauri v2](https://v2.tauri.app/) desktop application with a clear separation between frontend and backend.

### Frontend

- **React 19** + **TypeScript** — UI framework
- **Vite** — Build tool and dev server
- **Tailwind CSS v4** + **shadcn/ui** — Styling and component library
- **Zustand** — State management
- **Tauri Events** — Real-time communication with the backend (streaming AI, sync progress)

### Backend

- **Rust** — Core application logic
- **SeaORM** + **SQLite** — Database ORM and local storage
- **Tauri Commands** — IPC bridge between frontend and backend
- **git2** — Git operations for repository management
- **reqwest** — HTTP client for GitHub/GitLab APIs

## Project Structure

```
ossue/
├── src/                    # Frontend (React + TypeScript)
│   ├── main.tsx            # React entry point
│   ├── App.tsx             # Main app layout and routing
│   ├── components/         # UI components
│   │   ├── chat/           # AI chat panel
│   │   ├── inbox/          # Inbox item list and details
│   │   ├── layout/         # Sidebar, panels, splash screen
│   │   ├── onboarding/     # First-run setup wizard
│   │   ├── settings/       # Settings page
│   │   └── ui/             # shadcn/ui primitives
│   ├── hooks/              # Custom React hooks
│   ├── stores/             # Zustand state stores
│   ├── lib/                # Utilities and Tauri API bindings
│   └── types/              # TypeScript type definitions
├── src-tauri/              # Tauri backend (Rust)
│   ├── tauri.conf.json     # Tauri app configuration
│   └── src/
│       ├── main.rs         # Tauri entry point
│       ├── lib.rs          # App setup, state, tray, DB init
│       └── commands/       # Tauri IPC command handlers
│           ├── ai.rs       # AI analysis and chat
│           ├── auth.rs     # GitHub/GitLab authentication
│           ├── connectors.rs
│           ├── items.rs    # Inbox item CRUD
│           ├── repos.rs    # Project/repo management
│           ├── settings.rs # App settings
│           └── ...
├── crates/core/            # Shared Rust library
│   └── src/
│       ├── models/         # SeaORM entities (item, project, etc.)
│       ├── services/       # Business logic (GitHub, GitLab, AI, git)
│       ├── migration/      # Database migrations
│       ├── sync.rs         # Project sync engine
│       └── logging.rs      # Structured logging setup
├── docs/                   # Documentation and design docs
├── Cargo.toml              # Rust workspace configuration
└── package.json            # Frontend dependencies (pnpm)
```

## Building for Production

```bash
# Build the frontend and package the app
pnpm run tauri build
```

The output binary will be in `src-tauri/target/release/bundle/`.

## Guidelines

### Code Style

- **Rust:** Follow standard Rust formatting (`cargo fmt`). Run `cargo clippy` before submitting.
- **TypeScript:** Follow the existing patterns in the codebase. Use TypeScript strict mode.

### Pull Requests

1. Fork the repository and create a feature branch
2. Make your changes with clear, focused commits
3. Ensure `cargo clippy` and `cargo fmt --check` pass
4. Open a PR with a clear description of what changed and why

### Reporting Issues

Open an issue on GitHub with:
- Steps to reproduce
- Expected vs actual behavior
- Platform and app version
