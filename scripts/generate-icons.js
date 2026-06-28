#!/usr/bin/env node
/**
 * Generates all icon files required by Tauri using only built-in Node.js APIs.
 * Produces a WhatsApp-green circle on a transparent background.
 *
 * Output: src-tauri/icons/{32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico}
 */

"use strict";
const zlib = require("zlib");
const fs = require("fs");
const path = require("path");

// ── CRC-32 (required by the PNG spec) ────────────────────────────────────────
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

// ── PNG helpers ───────────────────────────────────────────────────────────────
function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, "ascii");
  const len = Buffer.allocUnsafe(4);
  len.writeUInt32BE(data.length, 0);
  const crc = Buffer.allocUnsafe(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBytes, data])), 0);
  return Buffer.concat([len, typeBytes, data, crc]);
}

/**
 * Build a PNG buffer for a square image of `size` pixels.
 * `drawFn(pixels: Uint8Array, size: number)` fills RGBA pixels row-major.
 */
function buildPNG(size, drawFn) {
  const PNG_SIG = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  const ihdr = Buffer.allocUnsafe(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type: RGBA
  ihdr[10] = ihdr[11] = ihdr[12] = 0;

  const rgba = new Uint8Array(size * size * 4);
  drawFn(rgba, size);

  // Prepend filter byte (None = 0) to every scanline
  const rows = [];
  for (let y = 0; y < size; y++) {
    rows.push(Buffer.from([0]));
    rows.push(Buffer.from(rgba.buffer, y * size * 4, size * 4));
  }
  const compressed = zlib.deflateSync(Buffer.concat(rows), { level: 9 });

  return Buffer.concat([
    PNG_SIG,
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", compressed),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

// ── Icon artwork: WhatsApp-green circle ───────────────────────────────────────
function drawCircleIcon(rgba, size) {
  rgba.fill(0); // transparent background
  const cx = size / 2;
  const cy = size / 2;
  const r = size * 0.46;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist <= r) {
        const i = (y * size + x) * 4;
        rgba[i] = 0x00;   // R
        rgba[i + 1] = 0xa8; // G  (#00a884 = WhatsApp green)
        rgba[i + 2] = 0x84; // B
        rgba[i + 3] = 255;  // A
      }
    }
  }
}

// ── ICO file builder (embeds a PNG image directly — valid since Vista) ────────
function buildICO(pngBuf) {
  // ICONDIR
  const dir = Buffer.allocUnsafe(6);
  dir.writeUInt16LE(0, 0); // reserved
  dir.writeUInt16LE(1, 2); // type: icon
  dir.writeUInt16LE(1, 4); // image count

  // ICONDIRENTRY
  const entry = Buffer.allocUnsafe(16);
  entry[0] = 32;  // width  (use 32 px)
  entry[1] = 32;  // height
  entry[2] = 0;   // colour count (0 = >8 bpp)
  entry[3] = 0;   // reserved
  entry.writeUInt16LE(1, 4);             // planes
  entry.writeUInt16LE(32, 6);            // bits per pixel
  entry.writeUInt32LE(pngBuf.length, 8); // data size
  entry.writeUInt32LE(22, 12);           // data offset (6 + 16)

  return Buffer.concat([dir, entry, pngBuf]);
}

// ── Write everything ──────────────────────────────────────────────────────────
const OUT = path.resolve(__dirname, "..", "src-tauri", "icons");
fs.mkdirSync(OUT, { recursive: true });

const SIZES = [
  { file: "32x32.png", px: 32 },
  { file: "128x128.png", px: 128 },
  { file: "128x128@2x.png", px: 256 },
];

const pngCache = {};

for (const { file, px } of SIZES) {
  const buf = buildPNG(px, drawCircleIcon);
  pngCache[px] = buf;
  fs.writeFileSync(path.join(OUT, file), buf);
  console.log(`  ✓ icons/${file}`);
}

// icon.icns — macOS only; on Windows this file is present but unused.
// We store the 256 px PNG inside an ICNS shell (enough for Tauri's bundler).
const icnsData = pngCache[256];
// Minimal ICNS: magic + length + ic08 chunk (256×256 PNG)
const icnsChunkType = Buffer.from("ic08");
const icnsChunkLen = Buffer.allocUnsafe(4);
icnsChunkLen.writeUInt32BE(8 + icnsData.length, 0);
const icnsMagic = Buffer.from("icns");
const icnsTotalLen = Buffer.allocUnsafe(4);
icnsTotalLen.writeUInt32BE(8 + 8 + icnsData.length, 0);
const icnsBuf = Buffer.concat([
  icnsMagic,
  icnsTotalLen,
  icnsChunkType,
  icnsChunkLen,
  icnsData,
]);
fs.writeFileSync(path.join(OUT, "icon.icns"), icnsBuf);
console.log("  ✓ icons/icon.icns");

// icon.ico — embed the 32×32 PNG
const ico = buildICO(pngCache[32]);
fs.writeFileSync(path.join(OUT, "icon.ico"), ico);
console.log("  ✓ icons/icon.ico");

console.log("\nIcons ready.");
