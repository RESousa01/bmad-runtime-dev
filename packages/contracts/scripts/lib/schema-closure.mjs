import { createHash } from "node:crypto";
import { canonicalize } from "./canonical-json.mjs";

function sha256(value) {
  return `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`;
}

function resolveJsonPointer(document, encodedFragment, schemaId) {
  if (encodedFragment === "") return document;
  const fragment = decodeURIComponent(encodedFragment);
  if (!fragment.startsWith("/")) {
    throw new Error(`Unsupported non-pointer schema fragment in ${schemaId}#${encodedFragment}.`);
  }
  let current = document;
  for (const encodedToken of fragment.slice(1).split("/")) {
    const token = encodedToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object"
      || !Object.prototype.hasOwnProperty.call(current, token)) {
      throw new Error(`Schema reference does not resolve: ${schemaId}#${encodedFragment}.`);
    }
    current = current[token];
  }
  return current;
}

function referenceLocation(reference, baseSchemaId) {
  const resolved = new URL(reference, baseSchemaId);
  const encodedFragment = resolved.hash === "" ? "" : resolved.hash.slice(1);
  resolved.hash = "";
  return Object.freeze({
    schemaId: resolved.href,
    encodedFragment,
  });
}

export function buildSchemaClosureManifest({ rootSchemaId, schemas }) {
  if (typeof rootSchemaId !== "string" || rootSchemaId.length === 0) {
    throw new TypeError("A non-empty root schema ID is required.");
  }
  if (!Array.isArray(schemas)) {
    throw new TypeError("Schema closure input must be an array.");
  }

  const documents = new Map();
  for (const schema of schemas) {
    if (schema === null || typeof schema !== "object" || Array.isArray(schema)
      || typeof schema.$id !== "string" || schema.$id.length === 0) {
      throw new TypeError("Every schema closure document must have a non-empty $id.");
    }
    if (documents.has(schema.$id)) {
      throw new Error(`Duplicate schema ID in closure input: ${schema.$id}.`);
    }
    documents.set(schema.$id, schema);
  }
  if (!documents.has(rootSchemaId)) {
    throw new Error(`Unknown schema closure root: ${rootSchemaId}.`);
  }

  const memberIds = new Set();
  const visitedLocations = new Set();

  function visitNode(node, containingSchemaId) {
    if (node === null || typeof node !== "object") return;
    if (Array.isArray(node)) {
      for (const value of node) visitNode(value, containingSchemaId);
      return;
    }
    if (typeof node.$ref === "string") {
      const location = referenceLocation(node.$ref, containingSchemaId);
      visitLocation(location.schemaId, location.encodedFragment);
    }
    for (const [key, value] of Object.entries(node)) {
      if (key !== "$ref") visitNode(value, containingSchemaId);
    }
  }

  function visitLocation(schemaId, encodedFragment) {
    const locationKey = `${schemaId}#${encodedFragment}`;
    if (visitedLocations.has(locationKey)) return;
    visitedLocations.add(locationKey);
    const document = documents.get(schemaId);
    if (document === undefined) {
      throw new Error(`Schema closure reference leaves the registered schema set: ${locationKey}.`);
    }
    memberIds.add(schemaId);
    visitNode(resolveJsonPointer(document, encodedFragment, schemaId), schemaId);
  }

  visitLocation(rootSchemaId, "");
  const members = Object.freeze([...memberIds]
    .sort()
    .map((schemaId) => Object.freeze({
      schemaId,
      canonicalSha256: sha256(canonicalize(documents.get(schemaId))),
    })));
  const hashPreimage = Object.freeze({ rootSchemaId, members });
  return Object.freeze({
    ...hashPreimage,
    closureSha256: sha256(canonicalize(hashPreimage)),
  });
}
