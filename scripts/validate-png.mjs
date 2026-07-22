#!/usr/bin/env node
import fs from "node:fs";

const filePath = process.argv[2];
const label = process.argv[3] || filePath || "png";

if (!filePath) {
  console.error("usage: validate-png.mjs <file> [label]");
  process.exit(2);
}

const bytes = fs.readFileSync(filePath);
const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const crcTable = Array.from({ length: 256 }, (_, index) => {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  }
  return value >>> 0;
});

function fail(message) {
  console.error(`invalid PNG for ${label}: ${message}`);
  process.exit(1);
}

if (bytes.length < 8 || !bytes.subarray(0, 8).equals(signature)) {
  fail("missing PNG signature");
}

let offset = 8;
let seenIhdr = false;
let seenIend = false;

while (offset < bytes.length) {
  if (offset + 12 > bytes.length) {
    fail("truncated chunk header");
  }

  const length = bytes.readUInt32BE(offset);
  const typeStart = offset + 4;
  const dataStart = offset + 8;
  const dataEnd = dataStart + length;
  const crcStart = dataEnd;
  const nextOffset = crcStart + 4;

  if (nextOffset > bytes.length) {
    fail("chunk length exceeds file size");
  }

  const type = bytes.subarray(typeStart, dataStart).toString("ascii");
  const expectedCrc = bytes.readUInt32BE(crcStart);
  const actualCrc = crc32(bytes.subarray(typeStart, dataEnd));

  if (actualCrc !== expectedCrc) {
    fail(`${type} CRC mismatch`);
  }

  if (!seenIhdr && type !== "IHDR") {
    fail("first chunk is not IHDR");
  }
  if (type === "IHDR") {
    if (seenIhdr) {
      fail("duplicate IHDR");
    }
    if (length !== 13) {
      fail("invalid IHDR length");
    }
    seenIhdr = true;
  }
  if (type === "IEND") {
    if (length !== 0) {
      fail("invalid IEND length");
    }
    seenIend = true;
    if (nextOffset !== bytes.length) {
      fail("trailing bytes after IEND");
    }
    break;
  }

  offset = nextOffset;
}

if (!seenIhdr) {
  fail("missing IHDR");
}
if (!seenIend) {
  fail("missing IEND");
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = (crc >>> 8) ^ crcTable[(crc ^ byte) & 0xff];
  }
  return (crc ^ 0xffffffff) >>> 0;
}
