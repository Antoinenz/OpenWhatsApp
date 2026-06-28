#!/usr/bin/env node
/**
 * Generates all icon files required by Tauri from openwhatsapp.svg.
 *
 * Requires: rsvg-convert (librsvg) — available on Linux/macOS.
 * Icons are pre-committed so Windows CI runners don't need this tool.
 *
 * Output:
 *   src-tauri/icons/16x16.png
 *   src-tauri/icons/32x32.png
 *   src-tauri/icons/48x48.png
 *   src-tauri/icons/128x128.png
 *   src-tauri/icons/128x128@2x.png   (256 px)
 *   src-tauri/icons/icon.icns
 *   src-tauri/icons/icon.ico         (multi-size: 16, 32, 48, 256)
 */

"use strict";
const { execFileSync } = require("child_process");
const fs   = require("fs");
const path = require("path");

const SVG = path.resolve(__dirname, "..", "openwhatsapp.svg");
const OUT = path.resolve(__dirname, "..", "src-tauri", "icons");
fs.mkdirSync(OUT, { recursive: true });

// ── Rasterise SVG → PNG via rsvg-convert ─────────────────────────────────────
function rasterise(sizePx, outFile) {
  execFileSync("rsvg-convert", [
    "--width",  String(sizePx),
    "--height", String(sizePx),
    "--output", outFile,
    SVG,
  ]);
}

// ── Generate all PNG sizes ────────────────────────────────────────────────────
const SIZES = [
  { file: "16x16.png",      px: 16  },
  { file: "32x32.png",      px: 32  },
  { file: "48x48.png",      px: 48  },
  { file: "128x128.png",    px: 128 },
  { file: "128x128@2x.png", px: 256 },
];

const pngCache = {};
for (const { file, px } of SIZES) {
  const dest = path.join(OUT, file);
  rasterise(px, dest);
  pngCache[px] = fs.readFileSync(dest);
  console.log(`  ✓ icons/${file}`);
}

// ── icon.ico — multi-size ICO (16, 32, 48, 256 px embedded as PNGs) ──────────
// Each entry is a PNG blob directly (valid since Windows Vista).
function buildMultiICO(entries) {
  const count = entries.length;
  // ICONDIR header (6 bytes) + ICONDIRENTRY × count (16 bytes each)
  const headerSize = 6 + count * 16;

  const dir = Buffer.allocUnsafe(6);
  dir.writeUInt16LE(0, 0);     // reserved
  dir.writeUInt16LE(1, 2);     // type: icon
  dir.writeUInt16LE(count, 4); // image count

  const entryBufs = [];
  let offset = headerSize;

  for (const { px, png } of entries) {
    const entry = Buffer.allocUnsafe(16);
    // Width/height: 0 means 256 in the ICO spec
    entry[0] = px >= 256 ? 0 : px;
    entry[1] = px >= 256 ? 0 : px;
    entry[2] = 0;   // colour count
    entry[3] = 0;   // reserved
    entry.writeUInt16LE(1,  4);           // planes
    entry.writeUInt16LE(32, 6);           // bits per pixel
    entry.writeUInt32LE(png.length,  8);  // data size
    entry.writeUInt32LE(offset,     12);  // data offset
    entryBufs.push(entry);
    offset += png.length;
  }

  return Buffer.concat([dir, ...entryBufs, ...entries.map(e => e.png)]);
}

const icoEntries = [
  { px: 16,  png: pngCache[16]  },
  { px: 32,  png: pngCache[32]  },
  { px: 48,  png: pngCache[48]  },
  { px: 256, png: pngCache[256] },
];
fs.writeFileSync(path.join(OUT, "icon.ico"), buildMultiICO(icoEntries));
console.log("  ✓ icons/icon.ico  (16, 32, 48, 256 px)");

// ── icon.icns — macOS: wrap 256 px PNG in a minimal ICNS shell ───────────────
const png256 = pngCache[256];
const icnsChunkType = Buffer.from("ic08");
const icnsChunkLen  = Buffer.allocUnsafe(4);
icnsChunkLen.writeUInt32BE(8 + png256.length, 0);
const icnsMagic    = Buffer.from("icns");
const icnsTotalLen = Buffer.allocUnsafe(4);
icnsTotalLen.writeUInt32BE(8 + 8 + png256.length, 0);
fs.writeFileSync(
  path.join(OUT, "icon.icns"),
  Buffer.concat([icnsMagic, icnsTotalLen, icnsChunkType, icnsChunkLen, png256])
);
console.log("  ✓ icons/icon.icns");

console.log("\nIcons ready.");
