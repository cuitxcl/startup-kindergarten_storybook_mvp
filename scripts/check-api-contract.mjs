#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const rootDir = path.resolve(import.meta.dirname, "..");
const controllersDir = path.join(rootDir, "server", "src", "controllers");
const contractPath = path.join(rootDir, "docs", "03-后端接口契约.md");
const httpMethods = ["get", "post", "patch", "delete"];

function normalizeApiPath(value) {
  return value.replaceAll("{", ":").replaceAll("}", "");
}

function routePair(method, apiPath) {
  return `${method.toUpperCase()} ${apiPath}`;
}

function findAddCallBodies(content) {
  const calls = [];
  let searchFrom = 0;

  while (true) {
    const addIndex = content.indexOf(".add(", searchFrom);
    if (addIndex === -1) break;

    const callStart = addIndex + ".add(".length;
    let depth = 1;
    let cursor = callStart;

    for (; cursor < content.length; cursor += 1) {
      const char = content[cursor];
      if (char === "(") depth += 1;
      if (char === ")") depth -= 1;
      if (depth === 0) break;
    }

    calls.push(content.slice(callStart, cursor));
    searchFrom = cursor + 1;
  }

  return calls;
}

function readControllerRoutePairs() {
  const controllerFiles = fs
    .readdirSync(controllersDir)
    .filter((fileName) => fileName.endsWith(".rs"))
    .sort();

  const pairs = new Set();
  for (const fileName of controllerFiles) {
    const content = fs.readFileSync(path.join(controllersDir, fileName), "utf8");
    for (const callBody of findAddCallBodies(content)) {
      const routeMatch = callBody.match(/"((?:\/api)\/[^"]+)"/);
      if (!routeMatch) continue;

      const apiPath = normalizeApiPath(routeMatch[1]);
      for (const method of httpMethods) {
        if (new RegExp(`\\b${method}\\s*\\(`).test(callBody)) {
          pairs.add(routePair(method, apiPath));
        }
      }
    }
  }

  return [...pairs].sort();
}

function readDocumentedRoutePairs() {
  const content = fs.readFileSync(contractPath, "utf8");
  const pairs = new Set();

  for (const match of content.matchAll(
    /\|\s*`(GET|POST|PATCH|DELETE)`\s*\|\s*`(\/api\/[^`]+)`\s*\|/g,
  )) {
    pairs.add(`${match[1]} ${normalizeApiPath(match[2])}`);
  }

  return [...pairs].sort();
}

const controllerRoutePairs = readControllerRoutePairs();
const documentedRoutePairs = readDocumentedRoutePairs();
const controllerPairSet = new Set(controllerRoutePairs);
const documentedPairSet = new Set(documentedRoutePairs);
const controllerPaths = new Set(controllerRoutePairs.map((pair) => pair.split(" ").slice(1).join(" ")));
const documentedPaths = new Set(documentedRoutePairs.map((pair) => pair.split(" ").slice(1).join(" ")));

const missingRoutePairs = controllerRoutePairs.filter((pair) => !documentedPairSet.has(pair));
const staleDocumentedRoutePairs = documentedRoutePairs
  .filter((pair) => !controllerPairSet.has(pair))
  .sort();
const missingPaths = [...controllerPaths].filter((route) => !documentedPaths.has(route)).sort();
const staleDocumentedPaths = [...documentedPaths]
  .filter((route) => !controllerPaths.has(route))
  .sort();

if (missingRoutePairs.length > 0) {
  console.error("API contract is missing controller method+path route(s):");
  for (const pair of missingRoutePairs) {
    console.error(`  - ${pair}`);
  }
  console.error(`\nUpdate ${path.relative(rootDir, contractPath)} or mark the route as intentionally internal.`);
  process.exit(1);
}

if (staleDocumentedRoutePairs.length > 0) {
  console.error("API contract documents method+path route(s) that are not registered by controllers:");
  for (const pair of staleDocumentedRoutePairs) {
    console.error(`  - ${pair}`);
  }
  console.error(`\nRemove stale route(s) from ${path.relative(rootDir, contractPath)} or add the missing controller route.`);
  process.exit(1);
}

if (missingPaths.length > 0) {
  console.error("API contract is missing controller path(s):");
  for (const route of missingPaths) {
    console.error(`  - ${route}`);
  }
  console.error(`\nUpdate ${path.relative(rootDir, contractPath)} or mark the path as intentionally internal.`);
  process.exit(1);
}

if (staleDocumentedPaths.length > 0) {
  console.error("API contract documents path(s) that are not registered by controllers:");
  for (const route of staleDocumentedPaths) {
    console.error(`  - ${route}`);
  }
  console.error(`\nRemove stale path(s) from ${path.relative(rootDir, contractPath)} or add the missing controller route.`);
  process.exit(1);
}

console.log(
  `api contract ok: ${controllerRoutePairs.length} controller method route(s) covered, ${controllerPaths.size} path(s) current`,
);
