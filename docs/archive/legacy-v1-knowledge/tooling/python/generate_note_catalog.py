#!/usr/bin/env python3
"""Generate explicit authority classifications for frozen root notes."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "knowledge-base" / "evidence" / "note-catalog.json"
SCHEMA_VERSION = "sapphirus.living-knowledge.v1"
CURRENT_ENTRYPOINT = "knowledge-base/current/00-current-product-state.md"

PRESERVED = {
    "05 - Preserved Source Context.md",
    "06 - Preserved Critical Review.md",
}
SOURCE_PREFIXES = tuple(f"{index} - " for index in range(83, 93)) + ("100 - ",)
SUPPORTING = {
    "73 - Verification Register.md",
    "75 - Library Validation Protocol.md",
    "Library Quality Report.md",
    "Start Here.md",
    "Vault Map.md",
}


def classify(name: str) -> tuple[str, str, str | None]:
    if name in PRESERVED:
        return (
            "preserved_verbatim",
            "Preserved source or critical-review evidence; retain byte identity.",
            None,
        )
    if name.startswith(SOURCE_PREFIXES):
        return (
            "source_evidence",
            "Reviewed source evidence supporting current clean-room decisions.",
            None,
        )
    if name in SUPPORTING:
        return (
            "supporting_reference",
            "Operational navigation or validation evidence; not product authority.",
            CURRENT_ENTRYPOINT,
        )
    return (
        "historical",
        "Legacy planning guidance retained for provenance and supersession review.",
        CURRENT_ENTRYPOINT,
    )


def main() -> None:
    manifest = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))
    notes = []
    for record in manifest["files"]:
        name = record["name"]
        authority_class, reason, superseded_by = classify(name)
        notes.append(
            {
                "path": name,
                "authorityClass": authority_class,
                "reason": reason,
                "supersededBy": superseded_by,
            }
        )
    output = {
        "schemaVersion": SCHEMA_VERSION,
        "rootNoteCount": len(notes),
        "notes": notes,
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        json.dumps(output, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )


if __name__ == "__main__":
    main()
