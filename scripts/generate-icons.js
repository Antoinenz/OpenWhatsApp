#!/usr/bin/env node
/**
 * Generates all icon files required by Tauri from openwhatsapp.svg.
 *
 * Requires: ImageMagick (`convert`) — pre-installed on most Linux/macOS systems
 * and on GitHub Actions windows-latest via the pre-installed toolchain.
 *
 * Output: src-tauri/icons/{32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico}
 */

"use strict";
const { execFileSync } = require("child_process");
const zlib = require("zlib");
const fs   = require("fs");
const path = require("path");

const SVG = path.resolve(__dirname, "..", "openwhatsapp.svg");
const OUT = path.resolve(__dirname, "..", "src-tauri", "icons");
fs.mkdirSync(OUT, { recursive: true });

// ── CRC-32 ────────────────────────────────────────────────────────────────────
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++)
    c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

// ── ICO builder (embeds a PNG directly — valid since Vista) ──────────────────
function buildICO(pngBuf) {
  const dir   = Buffer.allocUnsafe(6);
  dir.writeUInt16LE(0, 0); dir.writeUInt16LE(1, 2); dir.writeUInt16LE(1, 4);

  const entry = Buffer.allocUnsafe(16);
  entry[0] = 32; entry[1] = 32; entry[2] = 0; entry[3] = 0;
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(pngBuf.length, 8);
  entry.writeUInt32LE(22, 12);

  return Buffer.concat([dir, entry, pngBuf]);
}

// ── Rasterise SVG → PNG via rsvg-convert (handles gradients correctly) ───────
function rasterise(sizePx, outFile) {
  execFileSync("rsvg-convert", [
    "--width",  String(sizePx),
    "--height", String(sizePx),
    "--output", outFile,
    SVG,
  ]);
}

// ── Generate PNGs ─────────────────────────────────────────────────────────────
const SIZES = [
  { file: "32x32.png",      px: 32  },
  { file: "128x128.png",    px: 128 },
  { file: "128x128@2x.png", px: 256 },
];

for (const { file, px } of SIZES) {
  const dest = path.join(OUT, file);
  rasterise(px, dest);
  console.log(`  ✓ icons/${file}`);
}

// ── icon.icns (macOS) — wrap the 256 px PNG in a minimal ICNS shell ──────────
const png256   = fs.readFileSync(path.join(OUT, "128x128@2x.png"));
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

// ── icon.ico (Windows) — embed the 32 px PNG ─────────────────────────────────
const png32 = fs.readFileSync(path.join(OUT, "32x32.png"));
fs.writeFileSync(path.join(OUT, "icon.ico"), buildICO(png32));
console.log("  ✓ icons/icon.ico");

console.log("\nIcons ready.");
