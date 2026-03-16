# TinyPNG Compressor

A Windows desktop app that compresses images using the [TinyPNG API](https://tinify.com/developers). Built with **Tauri v2** (Rust + HTML/CSS/JS).

Supports drag-and-drop, file browser, Windows "Send To" integration, and persistent settings.

**Supported formats:** PNG, JPEG, WebP, AVIF

## Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
- [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

## Setup

```powershell
npm install
```

## Development

```powershell
npm run tauri dev
```

First launch will compile ~450 Rust crates (takes a few minutes). Subsequent builds are incremental (~7s).

## Build for Production

```powershell
npm run tauri build
```

The installer and portable `.exe` are output to:

```
src-tauri/target/release/
├── tinypng-compressor.exe   # Portable (no install needed)
└── bundle/
    ├── msi/                 # MSI installer
    └── nsis/                # NSIS installer
```

> The standalone `.exe` is fully portable — just copy it anywhere and run. No installation required.

## Regenerate Icons

If you update the app icon (`icon_dark_bg.png`):

```powershell
npx -y @tauri-apps/cli@latest icon icon_dark_bg.png
```

## Project Structure

```
├── src/                    # Frontend (HTML/CSS/JS)
│   ├── index.html
│   ├── styles.css
│   ├── main.js
│   └── assets/logo.svg
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── lib.rs          # Tauri commands
│   │   ├── api.rs          # TinyPNG API client
│   │   └── config.rs       # Persistent config (%APPDATA%)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── logo_dark.svg           # Source logo
└── icon_dark_bg.png        # Source icon
```

## Configuration

Settings are stored in `%APPDATA%/tinypng-compressor/config.json`:

- **API Key** — get one free at [tinypng.com/developers](https://tinypng.com/developers)
- **Output directory** — defaults to a `compressed/` subfolder next to source files

## Windows "Send To"

### Automatic (in-app)

Use the **Install** button in Settings to add the shortcut automatically.

### Manual setup

1. Press `Win + R`, type `shell:sendto`, press Enter
2. Copy `tinypng-compressor.exe` (or create a shortcut to it) into the opened folder
3. Rename it to `TinyPNG Compressor` (optional)

Then right-click any image in Explorer → **Send to → TinyPNG Compressor** to compress immediately.
