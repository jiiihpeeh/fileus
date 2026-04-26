# Development Guide - Fileus (Tauri + Solid + TypeScript + Web Frontend)

## Project Overview
- **Frontend (Tauri App)**: Vite + SolidJS + TypeScript (`src/`)
- **Frontend (Web)**: SolidJS + TypeScript standalone web app (`src/web-frontend/`)
- **Backend**: Tauri (Rust) - `src-tauri/`
- **Package Manager**: Bun
- **Build Tool**: Vite
- **TLS**: mkcert-generated localhost certificates

## Architecture
```
Fileus/
├── src/                      # Tauri frontend - SolidJS + TypeScript
│   ├── App.tsx              # Main Tauri component (desktop)
│   ├── App.css
│   ├── index.tsx            # Entry point
│   └── assets/
├── src/web-frontend/        # Standalone web frontend (SolidJS + TS)
│   ├── public/              # Static assets
│   ├── src/
│   │   ├── App.tsx          # Web app main component
│   │   ├── main.tsx         # Web app entry point
│   │   └── styles.css       # Web app styles
│   ├── server/
│   │   └── index.js         # Node.js server (HTTP + HTTPS + API)
│   ├── dist/                # Built static assets
│   ├── package.json
│   ├── tsconfig.json        # TypeScript config
│   └── vite.config.js       # Vite config (HTTPS enabled)
├── src-tauri/               # Backend - Tauri/Rust
│   ├── src/
│   │   ├── main.rs         # Tauri entry point
│   │   └── lib.rs          # Rust commands & Tauri builder
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── capabilities/
├── dist/                    # Tauri frontend build (gitignored)
└── DEVELOPMENT.md
```

## Quick Start

### Prerequisites
- [Bun](https://bun.sh) - package manager and runtime
- [Rust](https://rust-lang.org) + `rustc` and `cargo`
- [Tauri CLI](https://tauri.app/v2/guides/getting-started/prerequisites)
- mkcert (installed to `~/.local/bin/mkcert`)

### Installation
```bash
# Install main app dependencies
bun install

# Install web frontend dependencies
cd src/web-frontend && bun install
```

## Development Modes

### 1. Tauri Desktop App (Recommended)
```bash
# Terminal 1: Full Tauri dev environment (frontend + backend)
bun run tauri dev
```
- Opens a native desktop window
- Frontend served from Vite dev server (port 1420)
- Backend: Rust Tauri process
- Hot reload for both frontend and Rust code

### 2. Standalone Web Frontend (HTTPS)
```bash
# Terminal 1: Build web frontend (once)
cd src/web-frontend && bun run build

# Terminal 2: Start web server with HTTPS
cd src/web-frontend && node server/index.js
```
Access at:
- **HTTPS**: https://localhost:8443  (mkcert signed ✅)
- **HTTP**: http://localhost:3000
- **API**: http://localhost:8080/api/...

Features:
- ✅ Pure SolidJS + TypeScript web app
- ✅ HTTPS with mkcert-generated certificates
- ✅ HTTP API at `/api/*` endpoints
- ✅ Serves production build from `dist/`
- ✅ SPA fallback to `index.html`
- ✅ Tauri bridge detection (`window.__TAURI__`)
- ✅ HTTP fallback for `greet` when not in Tauri

### 3. Web Frontend Dev Mode (Vite + HTTPS)
```bash
cd src/web-frontend && bun run dev
```
Vite dev server with HTTPS on https://localhost:3000 (hot reload enabled)

### 4. Tauri Frontend Dev Only
```bash
bun run dev
```
Vite dev server on http://localhost:1420 (no Tauri backend)

### 5. Build Tauri App for Production
```bash
bun run tauri build
```

## Available Scripts

### Root Level
| Script | Description |
|--------|-------------|
| `bun run dev` | Vite dev server for Tauri frontend (port 1420) |
| `bun run build` | Build Tauri frontend for production → `dist/` |
| `bun run serve` | Preview Tauri frontend production build |
| `bun run tauri dev` | Full Tauri dev (frontend + backend) |
| `bun run tauri build` | Bundle Tauri app for release |
| `bun run tauri <args>` | Any Tauri CLI command |

### Web Frontend (`src/web-frontend/`)
| Script | Description |
|--------|-------------|
| `cd src/web-frontend && bun run dev` | Vite dev server with HTTPS (port 3000) |
| `cd src/web-frontend && bun run build` | Build web frontend → `dist/` |
| `cd src/web-frontend && bun run preview` | Preview built web frontend |

## Tauri Commands (Rust → Frontend)

Commands are Rust functions exposed to the frontend via `#[tauri::command]`.

### Current Commands

#### `greet(name: string) -> string`
Frontend (TypeScript):
```ts
import { invoke } from "@tauri-apps/api/core";
const msg = await invoke("greet", { name: "World" });
```

Rust (`src-tauri/src/lib.rs`):
```rust
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
```

#### `greet_json(name: string) -> { message: string }`
Returns a structured JSON response.

Frontend:
```ts
const result = await invoke("greet_json", { name: "World" });
// result = { message: "Hello, World! (from Tauri Rust backend)" }
```

Rust:
```rust
#[derive(Serialize, Deserialize)]
struct GreetResponse { message: String }

#[tauri::command]
fn greet_json(name: &str) -> GreetResponse {
    GreetResponse {
        message: format!("Hello, {}! (from Tauri Rust backend)", name),
    }
}
```

#### `get_tls_certificates() -> { cert, key, files }`
Returns the bundled mkcert TLS certificates as PEM strings. Used by web servers running inside/outside Tauri.

Frontend:
```ts
const result = await invoke("get_tls_certificates");
// result = {
//   cert: "-----BEGIN CERTIFICATE-----...",
//   key: "-----BEGIN PRIVATE KEY-----...",
//   files: { cert: ".../certs/localhost.pem", key: ".../certs/localhost-key.pem" }
// }
```

Register in `lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![greet, greet_json, get_tls_certificates])
```

### Adding New Commands

1. **Rust side** (`src-tauri/src/lib.rs`):
```rust
#[tauri::command]
fn my_command(arg: String) -> Result<String, String> {
    Ok(format!("Processed: {}", arg))
}

// Add to invoke_handler:
.invoke_handler(tauri::generate_handler![greet, greet_json, my_command])
```

2. **Frontend side** (TypeScript):
```ts
import { invoke } from "@tauri-apps/api/core";
const result = await invoke("my_command", { arg: "value" });
```

## Web Frontend (SolidJS + TypeScript)

### Project Structure
```
src/web-frontend/
├── src/
│   ├── App.tsx          # Main SolidJS component
│   ├── main.tsx         # App entry point
│   └── styles.css       # Component styles
├── server/
│   └── index.js         # Node.js server (HTTP + HTTPS + API)
├── index.html           # HTML entry point
├── package.json
├── tsconfig.json        # TypeScript config
└── vite.config.js       # Vite config (HTTPS)
```

### Key Features (Web Frontend)

**Client-side Detection:**
- Checks `window.__TAURI__` for Tauri runtime
- Falls back to HTTP API when not in Tauri
- Works in both desktop app and browser contexts

**Tauri Bridge (in App.tsx):**
```ts
const isTauri = typeof window !== "undefined" && "__TAURI__" in window;

if (isTauri) {
  const msg = await invoke("greet", { name });
} else {
  const res = await fetch(`http://localhost:8080/api/greet?name=${name}`);
}
```

### Web Frontend Server (`server/index.js`)

**Three servers running:**
1. **HTTPS Static Server** (port 8443) - `https://localhost:8443`
   - Serves static files from `src/web-frontend/dist/`
   - TLS via mkcert certificates
   - SPA fallback to `index.html`

2. **HTTP Static Server** (port 3000) - `http://localhost:3000`
   - Same as above, no TLS

3. **HTTP API Server** (port 8080) - `http://localhost:8080`
   - REST API endpoints
   - CORS enabled

### API Endpoints

| Endpoint | Method | Response |
|----------|--------|----------|
| `GET /api/health` | GET | `{"status": "ok", "timestamp": ...}` |
| `GET /api/greet?name=...` | GET | `{"message": "Hello, ..."}` |

### TLS Certificate Generation (Rust-native)

The Tauri app generates TLS certificates **in memory on demand** using the [`rcgen`](https://crates.io/crates/rcgen) crate — no external tools or file I/O required.

### Rust Commands

#### `generate_tls_certificates(domain?)` → `{ ca_cert, ca_key, domain, domain_cert, domain_key }`
Generates fresh certificates in memory. Returns PEM-encoded strings. No files created.

#### `generate_local_certs(domain?)` → `{ ca_cert, ca_key, domain, domain_cert, domain_key }`
Generates certificates and persists them to `src-tauri/certs/`. For development/debugging.

### Usage (Frontend)

```ts
import { invoke } from "@tauri-apps/api/core";

const certs = await invoke("generate_tls_certificates", { domain: "localhost" });
// certs = {
//   ca_cert: "-----BEGIN CERTIFICATE-----...",
//   ca_key: "-----BEGIN PRIVATE KEY-----...",
//   domain: "localhost",
//   domain_cert: "-----BEGIN CERTIFICATE-----...",
//   domain_key: "-----BEGIN PRIVATE KEY-----..."
// }
```

### Web Frontend

The standalone web server runs on HTTP (port 8080) with a simple REST API:
- `GET /api/health` - Health check
- `GET /api/greet?name=...` - Greet endpoint

When running inside Tauri, the frontend can invoke `generate_tls_certificates()` to get fresh certs and configure an HTTPS server dynamically.

### Running Web Frontend Server

```bash
# After building
cd src/web-frontend
bun run build
node server/index.js

# Or use Vite dev mode (with HTTPS)
cd src/web-frontend
bun run dev
```

## Adding Dependencies

```bash
# Main app (Tauri frontend)
bun add <package>
bun add -d <package>

# Web frontend
cd src/web-frontend
bun add <package>
bun add -d <package>

# Tauri plugin (Rust)
cd src-tauri
cargo add <crate>
```

## Backend (Rust/Tauri)

### Project Structure
- `src-tauri/src/main.rs` - Entry point (mobile support, calls `lib::run`)
- `src-tauri/src/lib.rs` - Command definitions and Tauri builder
- `src-tauri/Cargo.toml` - Rust dependencies
- `src-tauri/capabilities/` - Tauri deep linking / protocol capabilities

### Adding Rust Dependencies
```bash
cd src-tauri
cargo add <crate>
```

### Tauri Plugins (Enabled)
- `tauri_plugin_opener` - Open URLs/files with system default apps

## Vite Configuration

### Tauri Frontend (`vite.config.ts`)
- **Port**: 1420 (fixed, `strictPort: true`)
- **HMR**: Custom host support via `TAURI_DEV_HOST`
- **Watch ignore**: `src-tauri/**`
- **Clear screen**: `false` (shows Rust compile errors)

### Web Frontend (`src/web-frontend/vite.config.js`)
- **HTTPS**: Enabled with mkcert certificates
- **Port**: 3000 (HTTP dev + HTTPS)
- **Plugin**: `vite-plugin-solid`
- **Build output**: `dist/`

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `TAURI_DEV_HOST` | Custom HMR host for Tauri dev mode |
| `SSL_CERT_FILE` | Path to SSL certificate |
| `SSL_KEY_FILE` | Path to SSL key |
| `PORT` | HTTP server port (default: 3000) |
| `HTTPS_PORT` | HTTPS server port (default: 8443) |
| `HTTP_API_PORT` | API server port (default: 8080) |

## Debugging

### Frontend (Tauri App)
- Tauri app includes Chromium dev tools
- Or add `"devTools": true` in `tauri.conf.json` window config

### Frontend (Web)
- Standard browser dev tools (F12)
- Access via https://localhost:8443

### Backend (Rust)
```bash
# With debug logging
RUST_LOG=debug bun run tauri dev
```

## Testing

### Tauri Frontend
No test framework configured yet. Add:
```bash
bun add -D vitest @testing-library/solidjs jsdom
```

### Web Frontend
```bash
cd src/web-frontend
bun add -D vitest @testing-library/solidjs jsdom
```

### Rust Unit Tests
```bash
cd src-tauri
cargo test
```

## Mobile Support

`#[cfg_attr(mobile, tauri::mobile_entry_point)]` in `lib.rs` enables mobile (iOS/Android) support via Tauri Mobile.

## Deployment

### Desktop Platforms
```bash
bun run tauri build
```
Produces: `.app` (macOS), `.msi`/`.exe` (Windows), `.AppImage`/`.deb`/`.rpm` (Linux) in `src-tauri/target/release/bundle/`

### Web Deployment
Deploy the standalone web frontend separately:
```bash
cd src/web-frontend
bun run build
# Deploy dist/ + server/ to your hosting environment
```

## Security Notes

### TLS Certificates
- **Development**: mkcert self-signed certificates (trusted locally)
- **Production**: Use proper CA-signed certificates
- Never commit certs/keys to git

### CORS
- Web API has `Access-Control-Allow-Origin: *` (restrict in production)

### Tauri Security
- Validate all command inputs
- Use proper error handling with `invoke`
- Configure CSP in `tauri.conf.json` for production

## Project Conventions

1. **Tauri Commands**: Pure and focused. Use `Result<T, E>` for error handling.
2. **Async Operations**: `async/await` on frontend; spawn blocking in Rust for long ops.
3. **State Management**: Solid signals (`createSignal`, `createStore`)
4. **Type Safety**: TypeScript + Tauri generated types
5. **Dual Frontends**: Tauri app uses `src/`; Web uses `src/web-frontend/`

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Port 1420/3000/8443/8080 in use | Change respective port env vars |
| Rust compile errors | Run `cargo check` in `src-tauri/` |
| Tauri commands not found | Rebuild: `bun run tauri dev` |
| Build fails | Delete `dist/` and rebuild |
| HTTPS cert warnings | Install mkcert CA: `mkcert -install` |

## Resources

- [Tauri Documentation](https://tauri.app)
- [SolidJS Documentation](https://solidjs.com)
- [Vite Documentation](https://vite.dev)
- [mkcert](https://github.com/FiloSottile/mkcert)
