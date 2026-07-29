#!/usr/bin/env node
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const verifyPath = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../verify-bmad-100.mjs");
process.argv[1] = verifyPath;
await import(pathToFileURL(verifyPath).href);
