#!/usr/bin/env python3
"""Offline validation for the Sapphirus living knowledge layer."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "sapphirus.living-knowledge.v1"
CLAIM_ID = re.compile(r"^KB-[A-Z]+-[0-9]{3}$")
SOURCE_ID = re.compile(r"^SRC-[A-Z]+-[0-9]{3}$")
CLASSIFICATIONS = {
    "IMPLEMENTED_FACT",
    "VERIFIED_EXTERNAL_FACT",
    "ARCHITECTURE_DECISION",
    "PLANNED",
    "WORKTREE_CANDIDATE",
    "HISTORICAL",
    "UNKNOWN",
}
IMPLEMENTATION_STATUSES = {
    "implemented",
    "implemented_not_product_integrated",
    "scaffolded",
    "planned",
    "blocked",
    "unknown",
    "historical",
}
CONFIDENCE_LEVELS = {"high", "medium", "low"}
REGISTRY_FILES = ("claims.json", "sources.json", "note-catalog.json", "pins.json")


@dataclass
class ValidationResult:
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


def _read_json(path: Path, display_name: str, result: ValidationResult) -> Any | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        result.errors.append(f"knowledge-base/evidence/{display_name} is missing")
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        result.errors.append(f"{display_name}: invalid or unreadable JSON ({exc})")
    return None


def _records(
    document: Any,
    display_name: str,
    field_name: str,
    result: ValidationResult,
) -> list[Any]:
    if not isinstance(document, dict):
        result.errors.append(f"{display_name}: top level must be an object")
        return []
    if document.get("schemaVersion") != SCHEMA_VERSION:
        result.errors.append(f"{display_name}: unsupported schemaVersion")
    records = document.get(field_name)
    if not isinstance(records, list):
        result.errors.append(f"{display_name}: {field_name} must be an array")
        return []
    return records


def _validate_sources(records: list[Any], result: ValidationResult) -> set[str]:
    identifiers: set[str] = set()
    for record in records:
        if not isinstance(record, dict):
            result.errors.append("sources.json: source records must be objects")
            continue
        identifier = record.get("id")
        if not isinstance(identifier, str) or SOURCE_ID.fullmatch(identifier) is None:
            result.errors.append(f"sources.json: invalid source id {identifier!r}")
            continue
        if identifier in identifiers:
            result.errors.append(f"sources.json: duplicate source id {identifier!r}")
        identifiers.add(identifier)
        for field_name in ("type", "authority", "locator", "retrievedAt"):
            if not isinstance(record.get(field_name), str) or not record[field_name].strip():
                result.errors.append(
                    f"sources.json: source {identifier!r} requires {field_name}"
                )
    return identifiers


def _validate_claims(
    records: list[Any], source_ids: set[str], result: ValidationResult
) -> None:
    identifiers: set[str] = set()
    for record in records:
        if not isinstance(record, dict):
            result.errors.append("claims.json: claim records must be objects")
            continue
        identifier = record.get("id")
        if not isinstance(identifier, str) or CLAIM_ID.fullmatch(identifier) is None:
            result.errors.append(f"claims.json: invalid claim id {identifier!r}")
            continue
        if identifier in identifiers:
            result.errors.append(f"claims.json: duplicate claim id {identifier!r}")
        identifiers.add(identifier)
        if record.get("classification") not in CLASSIFICATIONS:
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid classification"
            )
        if record.get("implementationStatus") not in IMPLEMENTATION_STATUSES:
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid implementationStatus"
            )
        if record.get("confidence") not in CONFIDENCE_LEVELS:
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid confidence"
            )
        for field_name in ("statement", "observedAt", "limitations"):
            if not isinstance(record.get(field_name), str) or not record[field_name].strip():
                result.errors.append(
                    f"claims.json: claim {identifier!r} requires {field_name}"
                )
        linked_sources = record.get("sourceIds")
        if not isinstance(linked_sources, list) or not linked_sources:
            result.errors.append(
                f"claims.json: claim {identifier!r} requires sourceIds"
            )
        else:
            for source_id in linked_sources:
                if source_id not in source_ids:
                    result.errors.append(
                        f"claims.json: claim {identifier!r} references unknown source {source_id!r}"
                    )
        if not isinstance(record.get("supersedes"), list):
            result.errors.append(
                f"claims.json: claim {identifier!r} requires supersedes array"
            )


def validate_living_knowledge(
    vault_root: Path, repository_root: Path
) -> ValidationResult:
    """Validate living-knowledge registries without network or mutation."""

    del repository_root  # Pin and containment checks are added in a later slice.
    result = ValidationResult()
    evidence = vault_root / "knowledge-base" / "evidence"
    documents: dict[str, Any] = {}
    for name in REGISTRY_FILES:
        document = _read_json(evidence / name, name, result)
        if document is not None:
            documents[name] = document

    source_records = _records(
        documents.get("sources.json", {}),
        "sources.json",
        "sources",
        result,
    ) if "sources.json" in documents else []
    source_ids = _validate_sources(source_records, result)

    claim_records = _records(
        documents.get("claims.json", {}),
        "claims.json",
        "claims",
        result,
    ) if "claims.json" in documents else []
    _validate_claims(claim_records, source_ids, result)

    if "note-catalog.json" in documents:
        _records(
            documents["note-catalog.json"],
            "note-catalog.json",
            "notes",
            result,
        )
    if "pins.json" in documents:
        _records(documents["pins.json"], "pins.json", "pins", result)
    return result
