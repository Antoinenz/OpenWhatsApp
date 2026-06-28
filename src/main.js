// This file is only used during the brief splash screen before the Rust
// backend navigates the WebView to https://web.whatsapp.com.
// The real work (notification bridging, tray, etc.) happens in src-tauri/src/.

import { invoke } from "@tauri-apps/api/core";

// Tell the backend we're ready — it will immediately navigate to WhatsApp Web.
invoke("navigate_to_whatsapp").catch(console.error);
