# OpenWhatsApp

A lightweight, native WhatsApp client for Windows built with [Tauri](https://tauri.app/).

## Why?

The official WhatsApp Desktop for Windows is an Electron app — it bundles its own copy of Chromium, eating 300–500 MB of RAM just to display a web page. OpenWhatsApp uses **Tauri + WebView2**, which is already installed on every Windows 10/11 machine, slashing RAM usage to under 80 MB.

| | Official WhatsApp | OpenWhatsApp |
|---|---|---|
| RAM usage | ~350 MB | ~70 MB |
| Installer size | ~150 MB | ~5 MB |
| Native notifications | ✅ | ✅ |
| Stays logged in | ❌ (logs out ~2 days) | ✅ |
| System tray | ✅ | ✅ |
| Auto-start with Windows | ✅ | ✅ |

## Features

- **Full WhatsApp Web** — same feature set as web.whatsapp.com
- **Persistent session** — stores your login in a dedicated WebView2 profile; you won't get logged out
- **Native Windows notifications** — intercepted from WhatsApp Web and routed through the Windows notification system
- **System tray** — close the window and it keeps running in the tray; right-click to quit
- **Auto-start** — optional launch at Windows startup
- **Tiny footprint** — ~5 MB installer, no bundled browser

## Requirements

- Windows 10 (1803+) or Windows 11
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (pre-installed on Windows 11 and most Windows 10 machines)

## Build from source

### Prerequisites

```
# Install Rust
winget install Rustlang.Rustup

# Install Node.js (LTS)
winget install OpenJS.NodeJS.LTS
```

### Steps

```bash
git clone https://github.com/Antoinenz/openwhatsapp.git
cd openwhatsapp
npm install
npm run tauri build
```

The installer will be at `src-tauri/target/release/bundle/nsis/OpenWhatsApp_*_x64-setup.exe`.

### Dev mode

```bash
npm run tauri dev
```

## How it works

1. Tauri spawns a WebView2 window pointed at `https://web.whatsapp.com`
2. A JavaScript preload script intercepts the browser's `Notification` API and tunnels notification data to the Rust backend via Tauri's IPC bridge
3. The Rust backend fires a native Windows toast notification
4. The WebView2 data directory is stored in `%APPDATA%\openwhatsapp\webview` — your session cookies and keys live there persistently, so you stay logged in

## License

MIT — see [LICENSE](LICENSE)
