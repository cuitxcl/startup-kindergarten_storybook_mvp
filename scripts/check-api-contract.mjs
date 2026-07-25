#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const rootDir = path.resolve(import.meta.dirname, "..");
const controllersDir = path.join(rootDir, "server", "src", "controllers");
const contractPath = path.join(rootDir, "docs", "03-后端接口契约.md");

function normalizeApiPath(value) {
  return value.replaceAll("{", ":").replaceAll("}", "");
}

function readControllerRoutes() {
  const controllerFiles = fs
    .readdirSync(controllersDir)
    .filter((fileName) => fileName.endsWith(".rs"))
    .sort();

  const routes = new Set();
  for (const fileName of controllerFiles) {
    const content = fs.readFileSync(path.join(controllersDir, fileName), "utf8");
    for (const match of content.matchAll(/"((?:\/api)\/[^"]+)"/g)) {
      routes.add(normalizeApiPath(match[1]));
    }
  }

  return [...routes].sort();
}

function readDocumentedApiPaths() {
  const content = fs.readFileSync(contractPath, "utf8");
  const paths = new Set();

  for (const match of content.matchAll(/`(\/api\/[\s\S]*?)`/g)) {
    const rawPath = match[1];
    if (rawPath.includes("\n")) continue;
    paths.add(normalizeApiPath(rawPath));
  }

  return paths;
}

const controllerRoutes = readControllerRoutes();
const documentedPaths = readDocumentedApiPaths();
const missingRoutes = controllerRoutes.filter((route) => !documentedPaths.has(route));

if (missingRoutes.length > 0) {
  console.error("API contract is missing controller route(s):");
  for (const route of missingRoutes) {
    console.error(`  - ${route}`);
  }
  console.error(`\nUpdate ${path.relative(rootDir, contractPath)} or mark the route as intentionally internal.`);
  process.exit(1);
}

console.log(`api contract ok: ${controllerRoutes.length} controller route(s) covered`);
