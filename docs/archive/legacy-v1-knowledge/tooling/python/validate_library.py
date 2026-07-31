#!/usr/bin/env python3
"""Read-only structural validator for the root Sapphirus Markdown library."""

from __future__ import annotations

import json
import hashlib
import re
import sys
from pathlib import Path

from living_knowledge import validate_living_knowledge


ROOT = Path(__file__).resolve().parents[1]
WIKILINK = re.compile(r"\[\[([^\]]+)\]\]")


def canonical_markdown_bytes(payload: bytes) -> bytes:
    return payload.replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def unquote(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def frontmatter(lines: list[str]) -> tuple[list[str], int] | None:
    if not lines or lines[0].strip() != "---":
        return None
    for index in range(1, len(lines)):
        if lines[index].strip() == "---":
            return lines[1:index], index
    return None


def table_columns(line: str) -> int:
    escaped = False
    in_code = False
    in_wikilink = False
    count = 0
    index = 0
    while index < len(line):
        char = line[index]
        if escaped:
            escaped = False
            index += 1
            continue
        if char == "\\":
            escaped = True
            index += 1
            continue
        if char == "`":
            in_code = not in_code
            index += 1
            continue
        pair = line[index : index + 2]
        if not in_code and pair == "[[":
            in_wikilink = True
            index += 2
            continue
        if not in_code and pair == "]]":
            in_wikilink = False
            index += 2
            continue
        if char == "|" and not in_code and not in_wikilink:
            count += 1
        index += 1
    return max(count - 1, 0) if line.rstrip().endswith("|") else count


def collect_names(files: list[Path]) -> set[str]:
    names: set[str] = {path.stem.casefold() for path in files}
    for path in files:
        lines = path.read_text(encoding="utf-8-sig").splitlines()
        fm = frontmatter(lines)
        if not fm:
            continue
        meta, _ = fm
        in_aliases = False
        for line in meta:
            if line.startswith("title:"):
                names.add(unquote(line.split(":", 1)[1]).casefold())
            if line.startswith("aliases:"):
                in_aliases = True
                continue
            if in_aliases and re.match(r"^\s+-\s+", line):
                names.add(unquote(re.sub(r"^\s+-\s+", "", line)).casefold())
                continue
            if in_aliases and line and not line[0].isspace():
                in_aliases = False
    return names


def main() -> int:
    files = sorted(ROOT.glob("*.md"), key=lambda path: path.name.casefold())
    names = collect_names(files)
    errors: list[str] = []
    warnings: list[str] = []

    for path in files:
        text = path.read_text(encoding="utf-8-sig")
        lines = text.splitlines()
        fm = frontmatter(lines)
        if fm is None:
            errors.append(f"{path.name}: missing or unclosed YAML frontmatter")
            body_start = 0
        else:
            _, closing = fm
            body_start = closing + 1

        in_fence = False
        fence_language = ""
        json_buffer: list[str] = []
        table_start = 0
        table_width: int | None = None
        h1_count = 0

        for line_no, line in enumerate(lines[body_start:], start=body_start + 1):
            stripped = line.strip()
            if stripped.startswith("```"):
                if in_fence:
                    if fence_language == "json":
                        try:
                            json.loads("\n".join(json_buffer))
                        except json.JSONDecodeError as exc:
                            warnings.append(
                                f"{path.name}:{line_no}: illustrative JSON block is not strict JSON ({exc.msg})"
                            )
                    in_fence = False
                    fence_language = ""
                    json_buffer = []
                else:
                    in_fence = True
                    fence_language = stripped[3:].strip().casefold()
                continue
            if in_fence:
                if fence_language == "json":
                    json_buffer.append(line)
                continue

            if line.startswith("# "):
                h1_count += 1

            if stripped.startswith("|") and stripped.endswith("|"):
                width = table_columns(stripped)
                if table_width is None:
                    table_width = width
                    table_start = line_no
                elif width != table_width:
                    errors.append(
                        f"{path.name}:{line_no}: table has {width} columns; expected {table_width} from line {table_start}"
                    )
            else:
                table_width = None

        if in_fence:
            errors.append(f"{path.name}: unclosed fenced code block")
        if h1_count != 1:
            warnings.append(f"{path.name}: expected one body H1, found {h1_count}")
        if re.search(r"^(<<<<<<<|=======|>>>>>>>)", text, flags=re.MULTILINE):
            errors.append(f"{path.name}: merge-conflict marker present")

        for match in WIKILINK.finditer(text):
            raw = match.group(1)
            target = raw.split("|", 1)[0].split("#", 1)[0].strip()
            if target and target.casefold() not in names:
                warnings.append(f"{path.name}: unresolved wikilink target {target!r}")

    # V6.17 dual-delivery authority checks. These catch terminology that would
    # silently reintroduce the retired single-runtime or proposed-file model.
    required_legacy_evidence = {
        "93 - Split Web and Windows Desktop Architecture Plans.md",
        "94 - Windows Desktop Native Host and IPC.md",
        "95 - Windows Local Workspace and Execution.md",
        "96 - Windows Local State, Evidence, Checkpoint, and Rollback.md",
        "97 - Windows Desktop Security and Trust Model.md",
        "98 - Azure Support Plane for Windows Desktop.md",
        "99 - Dual-Delivery Contract and Conformance Specification.md",
    }
    for name in required_legacy_evidence:
        path = ROOT / name
        if not path.exists():
            errors.append(f"{name}: required V6.17 legacy evidence document is missing")

    retired_patterns = {
        "retired desktop identity document title": r"Desktop Identity, Licensing, Model Access, Sync, and Privacy",
        "retired desktop packaging document title": r"Desktop Packaging, Updates, Telemetry, and Release",
        "retired cross-delivery document title": r"Cross-Delivery Contract Compatibility",
        "retired local result type": r"(?<!Windows)\bLocalExecutionResultManifest\b",
        "ambiguous delivery field assignment": r"delivery_model\s*=\s*windows_local",
        "atomic patch component claim": r"Atomic Patch \+ Checkpoint Engine",
        "ambiguous trusted-local simulation": r"trusted local simulation",
    }
    for path in files:
        text = path.read_text(encoding="utf-8-sig")
        for label, pattern in retired_patterns.items():
            if re.search(pattern, text, flags=re.IGNORECASE):
                errors.append(f"{path.name}: {label}")

    manifest_verified = False
    manifest_path = ROOT / "manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        records = {record["name"]: record for record in manifest["files"]}
        expected_names = {path.name for path in files}
        if set(records) != expected_names:
            errors.append("manifest.json: root Markdown file set does not match")
        total_lines = 0
        total_bytes = 0
        for path in files:
            payload = canonical_markdown_bytes(path.read_bytes())
            line_count = len(payload.decode("utf-8-sig").splitlines())
            byte_count = len(payload)
            total_lines += line_count
            total_bytes += byte_count
            record = records.get(path.name, {})
            expected = {
                "lines": line_count,
                "bytes": byte_count,
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
            for field, value in expected.items():
                if record.get(field) != value:
                    errors.append(f"manifest.json: {path.name} {field} mismatch")
        expected_metrics = {
            "markdown_files": len(files),
            "markdown_lines": total_lines,
            "markdown_bytes": total_bytes,
        }
        if manifest.get("metrics") != expected_metrics:
            errors.append("manifest.json: aggregate metrics mismatch")
        manifest_verified = not any(item.startswith("manifest.json:") for item in errors)
    except (FileNotFoundError, KeyError, TypeError, json.JSONDecodeError) as exc:
        errors.append(f"manifest.json: invalid or unreadable ({exc})")

    living_result = validate_living_knowledge(ROOT, ROOT.parent)
    errors.extend(living_result.errors)
    warnings.extend(living_result.warnings)

    print(
        json.dumps(
            {
                "root_markdown_files": len(files),
                "manifest_verified": manifest_verified,
                "errors": errors,
                "warnings": warnings,
            },
            indent=2,
            ensure_ascii=False,
        )
    )
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
