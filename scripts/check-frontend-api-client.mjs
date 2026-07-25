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

function routeKey(method, apiPath) {
  return `${method} ${apiPath}`;
}

function readContractRoutes() {
  const content = fs.readFileSync(contractPath, "utf8");
  const routes = new Set();

  for (const match of content.matchAll(
    /\|\s*`(GET|POST|PATCH|DELETE)`\s*\|\s*`(\/api\/[^`]+)`\s*\|/g,
  )) {
    routes.add(routeKey(match[1], normalizeApiPath(match[2])));
  }

  return routes;
}

function findRequestCalls(content) {
  const calls = [];
  const requestCallPattern = /\brequest(?:Envelope|Blob)?(?:<[^>]+>)?\s*\(/g;
  let match;

  while ((match = requestCallPattern.exec(content)) !== null) {
    const callStart = match.index + match[0].length;
    let depth = 1;
    let cursor = callStart;

    for (; cursor < content.length && depth > 0; cursor += 1) {
      if (content[cursor] === "(") {
        depth += 1;
      } else if (content[cursor] === ")") {
        depth -= 1;
      }
    }

    if (depth === 0) {
      calls.push(content.slice(callStart, cursor - 1));
    }

    requestCallPattern.lastIndex = cursor;
  }

  return calls;
}

function readClientRoutes() {
  const content = fs.readFileSync(clientPath, "utf8");
  const routes = new Set();

  for (const callBody of findRequestCalls(content)) {
    const pathMatch = callBody.match(/[`"](\/api\/[^`"\n]*)[`"]/) ?? callBody.match(/['](\/api\/[^'\n]*)[']/);
    if (!pathMatch) continue;

    const methodMatch = callBody.match(/\bmethod\s*:\s*["'](GET|POST|PATCH|DELETE)["']/);
    const method = methodMatch?.[1] ?? "GET";

    routes.add(routeKey(method, normalizeApiPath(pathMatch[1])));
  }

  return routes;
}

const contractRoutes = readContractRoutes();
const clientRoutes = readClientRoutes();
const unknownClientRoutes = [...clientRoutes]
  .filter((route) => !contractRoutes.has(route))
  .sort();

if (unknownClientRoutes.length > 0) {
  console.error("Frontend API client method route(s) not documented in API contract:");
  for (const route of unknownClientRoutes) {
    console.error(`  - ${route}`);
  }
  console.error(`\nUpdate ${path.relative(rootDir, contractPath)} or fix the client path in ${path.relative(rootDir, clientPath)}.`);
  process.exit(1);
}

const clientPaths = new Set([...clientRoutes].map((route) => route.replace(/^[A-Z]+ /, "")));

console.log(`frontend api client ok: ${clientRoutes.size} method route(s), ${clientPaths.size} path(s) match API contract`);
