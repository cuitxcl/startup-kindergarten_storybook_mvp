#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const rootDir = path.resolve(import.meta.dirname, "..");
const clientPath = path.join(rootDir, "frontend", "src", "api", "client.ts");
const contractPath = path.join(rootDir, "docs", "03-后端接口契约.md");

function camelToSnake(value) {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function normalizeApiPath(value) {
  return value
    .replace(/\$\{suffix\}/g, "")
    .replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}/g, (_, name) => `:${camelToSnake(name)}`)
    .replaceAll("{", ":")
    .replaceAll("}", "");
}

function readContractPaths() {
  const content = fs.readFileSync(contractPath, "utf8");
  const paths = new Set();

  for (const match of content.matchAll(
    /\|\s*`(?:GET|POST|PATCH|DELETE)`\s*\|\s*`(\/api\/[^`]+)`\s*\|/g,
  )) {
    paths.add(normalizeApiPath(match[1]));
  }

  return paths;
}

function readClientPaths() {
  const content = fs.readFileSync(clientPath, "utf8");
  const paths = new Set();

  for (const match of content.matchAll(/[`"](\/api\/[\s\S]*?)[`"](?:[,)}]|\s)/g)) {
    const rawPath = match[1];
    if (rawPath.includes("\n")) continue;
    paths.add(normalizeApiPath(rawPath));
  }

  return paths;
}

const contractPaths = readContractPaths();
const clientPaths = readClientPaths();
const unknownClientPaths = [...clientPaths]
  .filter((route) => !contractPaths.has(route))
  .sort();

if (unknownClientPaths.length > 0) {
  console.error("Frontend API client calls path(s) not documented in API contract:");
  for (const route of unknownClientPaths) {
    console.error(`  - ${route}`);
  }
  console.error(`\nUpdate ${path.relative(rootDir, contractPath)} or fix the client path in ${path.relative(rootDir, clientPath)}.`);
  process.exit(1);
}

console.log(`frontend api client ok: ${clientPaths.size} path(s) match API contract`);
