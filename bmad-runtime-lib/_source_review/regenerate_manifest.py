#!/usr/bin/env python3
"""Regenerate the root Markdown integrity manifest deterministically."""

from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

from living_knowledge import living_manifest_document


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "manifest.json"
LIVING_MANIFEST = ROOT / "knowledge-base" / "manifest.json"


def canonical_markdown_bytes(payload: bytes) -> bytes:
    return payload.replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def main() -> None:
    created_at = "2026-07-09T00:00:00+00:00"
    if MANIFEST.exists():
        try:
            created_at = json.loads(MANIFEST.read_text(encoding="utf-8"))["created_at_utc"]
        except (KeyError, json.JSONDecodeError):
            pass

    records: list[dict[str, object]] = []
    total_lines = 0
    total_bytes = 0
    for path in sorted(ROOT.glob("*.md"), key=lambda item: item.name.casefold()):
        payload = canonical_markdown_bytes(path.read_bytes())
        line_count = len(payload.decode("utf-8-sig").splitlines())
        byte_count = len(payload)
        total_lines += line_count
        total_bytes += byte_count
        records.append(
            {
                "name": path.name,
                "lines": line_count,
                "bytes": byte_count,
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        )

    output = {
        "library": "sapphirus-bmad-runtime-obsidian-vault-v6.17-dual-delivery-architecture-contracts",
        "created_at_utc": created_at,
        "updated_at_utc": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "metrics": {
            "markdown_files": len(records),
            "markdown_lines": total_lines,
            "markdown_bytes": total_bytes,
        },
        "files": records,
    }
    MANIFEST.write_text(
        json.dumps(output, indent=4, ensure_ascii=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    LIVING_MANIFEST.write_text(
        json.dumps(living_manifest_document(ROOT), indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )


if __name__ == "__main__":
    main()
