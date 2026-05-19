import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { sourceApiManifestEvidence, sourceRouteAuditCases } from "./parity-fixtures.js";

const repoRoot = new URL("../../", import.meta.url);
const manifestUrl = new URL(".workflow/.maestro/maestro-20260518-full-closure-tasks/dashboard-api-manifest.md", repoRoot);
const contractUrl = new URL(".workflow/.maestro/maestro-20260518-full-closure-tasks/parity-contract.md", repoRoot);

test("dashboard API manifest families have concrete adapter-test evidence", async () => {
  const manifest = await readText(manifestUrl);
  const expectedKeys = [
    ...parseHttpFamilies(manifest),
    ...parseWebSocketEndpoints(manifest).map((endpoint) => `ws:${endpoint}`),
  ].sort();
  const actualKeys = Object.keys(sourceApiManifestEvidence).sort();

  assert.deepEqual(actualKeys, expectedKeys);

  const fileCache = new Map();
  for (const [family, entries] of Object.entries(sourceApiManifestEvidence)) {
    assert.ok(entries.length > 0, `${family} must reference at least one evidence file`);
    for (const entry of entries) {
      const fileText = await readEvidenceFile(entry.file, fileCache);
      for (const pattern of entry.patterns) {
        assert.ok(
          fileText.includes(pattern),
          `${family} evidence ${entry.file} is missing pattern ${pattern}`,
        );
      }
    }
  }
});

test("route audit cases cover source static fallbacks and Vue router contract", async () => {
  const [manifest, contract] = await Promise.all([
    readText(manifestUrl),
    readText(contractUrl),
  ]);
  const routeKeys = new Set(sourceRouteAuditCases.flatMap((route) => [route.path, route.sourcePattern]));

  for (const sourceRoute of parseSourceRoutes(contract)) {
    assert.ok(routeKeys.has(sourceRoute), `missing route audit case for ${sourceRoute}`);
  }

  for (const fallbackRoute of parseStaticFallbackRoutes(manifest)) {
    assert.ok(routeKeys.has(fallbackRoute), `missing static fallback smoke for ${fallbackRoute}`);
  }
});

async function readText(url) {
  return readFile(url, "utf8");
}

async function readEvidenceFile(path, cache) {
  if (!cache.has(path)) {
    cache.set(path, await readText(new URL(path, repoRoot)));
  }
  return cache.get(path);
}

function parseHttpFamilies(manifest) {
  const section = between(manifest, "## HTTP Routes", "## WebSocket Routes");
  return Array.from(section.matchAll(/^\| `([^`]+\.py)` \|/gm), (match) => match[1]);
}

function parseWebSocketEndpoints(manifest) {
  const section = between(manifest, "## WebSocket Routes", "## Static SPA Fallback Routes");
  return Array.from(section.matchAll(/`(\/api\/[^`]+)`/g), (match) => match[1]);
}

function parseStaticFallbackRoutes(manifest) {
  const section = manifest.split("## Static SPA Fallback Routes")[1] || "";
  return Array.from(section.matchAll(/`([^`]+)`/g), (match) => match[1])
    .flatMap((value) => value.split(","))
    .map((value) => value.trim())
    .filter((value) => value.startsWith("/"));
}

function parseSourceRoutes(contract) {
  const section = between(contract, "## Vue Router Contract", "## Vue Component Ownership");
  return Array.from(section.matchAll(/^\| `([^`]+)` \|/gm), (match) => match[1])
    .filter((value) => value.startsWith("/"));
}

function between(text, start, end) {
  const afterStart = text.split(start)[1] || "";
  return afterStart.split(end)[0] || "";
}
