#!/usr/bin/env python3
"""Offline validation for the Sapphirus living knowledge layer."""

from __future__ import annotations

import json
import hashlib
import re
import subprocess
from dataclasses import dataclass, field
from datetime import date
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlparse


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
AUTHORITY_CLASSES = {
    "current_authority",
    "supporting_reference",
    "source_evidence",
    "planned",
    "superseded",
    "historical",
    "preserved_verbatim",
}
OFFICIAL_SOURCE_HOSTS = {
    "devblogs.microsoft.com",
    "nodejs.org",
    "react.dev",
    "vite.dev",
}
GitBlobReader = Callable[[Path, str, str], bytes]


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


def _validate_sources(
    records: list[Any],
    repository_root: Path,
    result: ValidationResult,
    git_blob_reader: GitBlobReader,
) -> dict[str, dict[str, Any]]:
    identifiers: set[str] = set()
    valid_records: dict[str, dict[str, Any]] = {}
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
        if record.get("authority") != "primary":
            result.errors.append(
                f"sources.json: source {identifier!r} authority must be primary"
            )
        try:
            date.fromisoformat(record.get("retrievedAt"))
        except (TypeError, ValueError):
            result.errors.append(
                f"sources.json: source {identifier!r} has invalid retrievedAt"
            )
        source_type = record.get("type")
        locator = record.get("locator")
        if source_type == "repository" and isinstance(locator, str):
            target = _repository_path(repository_root, locator, identifier, result, registry="sources.json")
            commit = record.get("repositoryCommit")
            if not isinstance(commit, str) or re.fullmatch(r"[0-9a-f]{40}", commit) is None:
                result.errors.append(
                    f"sources.json: source {identifier!r} requires repositoryCommit"
                )
            elif target is not None:
                try:
                    git_blob_reader(repository_root, commit, locator)
                except (OSError, subprocess.SubprocessError, ValueError) as exc:
                    result.errors.append(
                        f"sources.json: source {identifier!r} is not available at repositoryCommit ({exc})"
                    )
            if target is not None and not target.is_file():
                result.errors.append(
                    f"sources.json: source {identifier!r} locator does not exist in the working tree"
                )
        elif source_type == "external_official" and isinstance(locator, str):
            parsed = urlparse(locator)
            if (
                parsed.scheme != "https"
                or not parsed.netloc
                or parsed.hostname not in OFFICIAL_SOURCE_HOSTS
            ):
                result.errors.append(
                    f"sources.json: source {identifier!r} is not on the reviewed official-source allowlist"
                )
        elif source_type not in {"repository", "external_official"}:
            result.errors.append(
                f"sources.json: source {identifier!r} has invalid type"
            )
        valid_records[identifier] = record
    return valid_records


def _read_git_blob(repository_root: Path, commit: str, locator: str) -> bytes:
    completed = subprocess.run(
        [
            "git",
            "-c",
            f"safe.directory={repository_root.resolve().as_posix()}",
            "show",
            f"{commit}:{locator}",
        ],
        cwd=repository_root,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise ValueError(detail or "git object lookup failed")
    return completed.stdout


def _validate_claims(
    records: list[Any], sources: dict[str, dict[str, Any]], result: ValidationResult
) -> set[str]:
    identifiers: set[str] = set()
    claims_by_id: dict[str, dict[str, Any]] = {}
    subjects: dict[str, list[str]] = {}
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
        claims_by_id[identifier] = record
        subject = record.get("subject")
        if not isinstance(subject, str) or not subject.strip():
            result.errors.append(f"claims.json: claim {identifier!r} requires subject")
        else:
            subjects.setdefault(subject, []).append(identifier)
        if record.get("classification") not in CLASSIFICATIONS:
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid classification"
            )
        if record.get("implementationStatus") not in IMPLEMENTATION_STATUSES:
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid implementationStatus"
            )
        if (
            record.get("classification") == "WORKTREE_CANDIDATE"
            and record.get("implementationStatus") == "implemented"
        ):
            result.errors.append(
                f"claims.json: claim {identifier!r} cannot present WORKTREE_CANDIDATE as implemented"
            )
        if record.get("confidence") not in CONFIDENCE_LEVELS:
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid confidence"
            )
        for field_name in ("statement", "observedAt", "revalidateBy", "limitations"):
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
                if source_id not in sources:
                    result.errors.append(
                        f"claims.json: claim {identifier!r} references unknown source {source_id!r}"
                    )
            exception = record.get("singleSourceException")
            if len(set(linked_sources)) < 2 and (
                not isinstance(exception, str) or not exception.strip()
            ):
                result.errors.append(
                    f"claims.json: claim {identifier!r} requires two distinct sources or singleSourceException"
                )
            if record.get("classification") == "VERIFIED_EXTERNAL_FACT":
                official_count = len(
                    {
                        source_id
                        for source_id in linked_sources
                        if sources.get(source_id, {}).get("type") == "external_official"
                    }
                )
                if official_count < 2 and (
                    not isinstance(exception, str) or not exception.strip()
                ):
                    result.errors.append(
                        f"claims.json: external claim {identifier!r} requires two official sources or singleSourceException"
                    )
            if isinstance(exception, str) and exception.strip():
                if len(exception.strip()) < 40:
                    result.errors.append(
                        f"claims.json: claim {identifier!r} singleSourceException is not sufficiently justified"
                    )
                try:
                    date.fromisoformat(record.get("singleSourceExceptionReviewedAt"))
                except (TypeError, ValueError):
                    result.errors.append(
                        f"claims.json: claim {identifier!r} requires singleSourceExceptionReviewedAt"
                    )
        deadline = record.get("revalidateBy")
        try:
            parsed_deadline = date.fromisoformat(deadline)
        except (TypeError, ValueError):
            result.errors.append(
                f"claims.json: claim {identifier!r} has invalid revalidateBy"
            )
        else:
            if parsed_deadline < date.today():
                result.errors.append(
                    f"claims.json: claim {identifier!r} revalidation is overdue"
                )
        if not isinstance(record.get("supersedes"), list):
            result.errors.append(
                f"claims.json: claim {identifier!r} requires supersedes array"
            )
    graph: dict[str, list[str]] = {}
    for identifier, record in claims_by_id.items():
        targets = record.get("supersedes")
        if not isinstance(targets, list):
            continue
        graph[identifier] = []
        for target in targets:
            if target not in claims_by_id:
                result.errors.append(
                    f"claims.json: claim {identifier!r} supersedes unknown claim {target!r}"
                )
            elif target == identifier:
                result.errors.append(
                    f"claims.json: claim {identifier!r} cannot supersede itself"
                )
            else:
                graph[identifier].append(target)
    _validate_acyclic_graph(graph, "claims.json: supersession cycle", result)
    for subject, claim_ids in subjects.items():
        if len(claim_ids) < 2:
            continue
        related = all(
            right in graph.get(left, []) or left in graph.get(right, [])
            for index, left in enumerate(claim_ids)
            for right in claim_ids[index + 1 :]
        )
        if not related:
            result.errors.append(
                f"claims.json: subject {subject!r} has conflicting current claims"
            )
    return identifiers


def _validate_acyclic_graph(
    graph: dict[str, list[str]], label: str, result: ValidationResult
) -> None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> None:
        if node in visiting:
            result.errors.append(f"{label} involving {node!r}")
            return
        if node in visited:
            return
        visiting.add(node)
        for target in graph.get(node, []):
            visit(target)
        visiting.remove(node)
        visited.add(node)

    for node in graph:
        visit(node)


def _frontmatter_value(text: str, field_name: str) -> str | None:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    for line in lines[1:]:
        if line.strip() == "---":
            return None
        if line.startswith(f"{field_name}:"):
            return line.split(":", 1)[1].strip().strip('"')
    return None


def _claim_id_list(value: str | None) -> list[str] | None:
    if value is None or not value.startswith("[") or not value.endswith("]"):
        return None
    return [item.strip() for item in value[1:-1].split(",") if item.strip()]


def _validate_current_notes(
    vault_root: Path, claim_ids: set[str], result: ValidationResult
) -> set[str]:
    current_root = vault_root / "knowledge-base" / "current"
    notes = sorted(current_root.glob("*.md")) if current_root.is_dir() else []
    if len(notes) != 8:
        result.errors.append(
            f"knowledge-base/current: expected 8 authority notes, found {len(notes)}"
        )
    referenced: set[str] = set()
    commits: set[str] = set()
    for path in notes:
        text = path.read_text(encoding="utf-8-sig")
        relative = path.relative_to(vault_root).as_posix()
        if _frontmatter_value(text, "authority") != "current":
            result.errors.append(f"{relative}: authority must be current")
        commit = _frontmatter_value(text, "repository_commit")
        if commit is None or re.fullmatch(r"[0-9a-f]{40}", commit) is None:
            result.errors.append(f"{relative}: invalid repository_commit")
        else:
            commits.add(commit)
        cutoff = _frontmatter_value(text, "research_cutoff")
        if cutoff is None or re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", cutoff) is None:
            result.errors.append(f"{relative}: invalid research_cutoff")
        note_claim_ids = _claim_id_list(_frontmatter_value(text, "claim_ids"))
        if not note_claim_ids:
            result.errors.append(f"{relative}: claim_ids must be a non-empty inline list")
            continue
        for claim_id in note_claim_ids:
            if claim_id not in claim_ids:
                result.errors.append(f"{relative}: unknown claim id {claim_id!r}")
            referenced.add(claim_id)
        _validate_markdown_links(path, vault_root, text, result)
    for claim_id in sorted(claim_ids - referenced):
        result.errors.append(f"claims.json: claim {claim_id!r} is not referenced by current notes")
    return commits


def _validate_markdown_links(
    path: Path, vault_root: Path, text: str, result: ValidationResult
) -> None:
    for target in re.findall(r"(?<!!)\[[^\]]+\]\(([^)]+)\)", text):
        clean = target.split("#", 1)[0].strip()
        if not clean or urlparse(clean).scheme:
            continue
        resolved = (path.parent / clean).resolve()
        root = vault_root.resolve()
        relative = path.relative_to(vault_root).as_posix()
        if not resolved.is_relative_to(root) or not resolved.exists():
            result.errors.append(f"{relative}: broken Markdown link {target!r}")


def _validate_authority_locations(vault_root: Path, result: ValidationResult) -> None:
    candidates = list(vault_root.glob("*.md"))
    knowledge_root = vault_root / "knowledge-base"
    if knowledge_root.is_dir():
        candidates.extend(
            path
            for path in knowledge_root.rglob("*.md")
            if "current" not in path.relative_to(knowledge_root).parts
        )
    for path in candidates:
        text = path.read_text(encoding="utf-8-sig")
        relative = path.relative_to(vault_root).as_posix()
        if _frontmatter_value(text, "authority") == "current":
            result.errors.append(
                f"{relative}: current authority is allowed only in knowledge-base/current"
            )
        if (
            path.parent == vault_root
            and path.name != "06 - Preserved Critical Review.md"
            and _frontmatter_value(text, "status") == "current"
        ):
            result.errors.append(
                f"{relative}: legacy root note cannot declare status current"
            )


def living_manifest_files(vault_root: Path) -> list[Path]:
    """Return the closed set covered by the living-layer manifest."""

    knowledge_root = vault_root / "knowledge-base"
    files: list[Path] = []
    for relative in ("current", "evidence"):
        directory = knowledge_root / relative
        if directory.is_dir():
            files.extend(path for path in directory.rglob("*") if path.is_file())
    return sorted(files, key=lambda path: path.relative_to(knowledge_root).as_posix())


def living_manifest_document(vault_root: Path) -> dict[str, Any]:
    """Build the deterministic living-layer manifest document."""

    knowledge_root = vault_root / "knowledge-base"
    records = []
    for path in living_manifest_files(vault_root):
        payload = path.read_bytes()
        records.append(
            {
                "path": path.relative_to(knowledge_root).as_posix(),
                "bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        )
    return {
        "schemaVersion": "sapphirus.living-knowledge-manifest.v1",
        "files": records,
    }


def _validate_living_manifest(vault_root: Path, result: ValidationResult) -> None:
    path = vault_root / "knowledge-base" / "manifest.json"
    try:
        actual = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        result.errors.append("knowledge-base/manifest.json is missing")
        return
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        result.errors.append(f"knowledge-base/manifest.json is invalid ({exc})")
        return
    expected = living_manifest_document(vault_root)
    actual_paths = {
        record.get("path")
        for record in actual.get("files", [])
        if isinstance(record, dict)
    } if isinstance(actual, dict) and isinstance(actual.get("files"), list) else set()
    expected_paths = {record["path"] for record in expected["files"]}
    if actual_paths != expected_paths:
        result.errors.append("knowledge-base/manifest.json: file set mismatch")
        return
    if actual != expected:
        result.errors.append("knowledge-base/manifest.json: content hash or size mismatch")


def _repository_path(
    repository_root: Path,
    relative: Any,
    pin_id: Any,
    result: ValidationResult,
    registry: str = "pins.json",
) -> Path | None:
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
        result.errors.append(f"{registry}: pin {pin_id} has invalid path")
        return None
    root = repository_root.resolve()
    candidate = (root / relative).resolve()
    if not candidate.is_relative_to(root):
        result.errors.append(f"{registry}: pin {pin_id} path escapes repository root")
        return None
    return candidate


def _json_path_value(document: Any, selector: Any) -> Any:
    if not isinstance(selector, list) or not selector:
        raise ValueError("json_path selector must be a non-empty array")
    value = document
    for part in selector:
        if not isinstance(part, str) or not isinstance(value, dict) or part not in value:
            raise ValueError("json_path selector does not resolve")
        value = value[part]
    return value


def _validate_pins(
    repository_root: Path, records: list[Any], result: ValidationResult
) -> None:
    identifiers: set[str] = set()
    for record in records:
        if not isinstance(record, dict):
            result.errors.append("pins.json: pin records must be objects")
            continue
        pin_id = record.get("id")
        if not isinstance(pin_id, str) or not pin_id:
            result.errors.append("pins.json: pin record requires id")
            continue
        if pin_id in identifiers:
            result.errors.append(f"pins.json: duplicate pin id {pin_id!r}")
        identifiers.add(pin_id)
        mode = record.get("mode")
        expected = record.get("expected")
        if mode not in {"exact_text", "json_path", "regex"}:
            result.errors.append(f"pins.json: pin {pin_id} has invalid mode")
            continue
        if not isinstance(expected, str):
            result.errors.append(f"pins.json: pin {pin_id} requires string expected")
            continue
        path = _repository_path(repository_root, record.get("path"), pin_id, result)
        if path is None:
            continue
        try:
            text = path.read_text(encoding="utf-8")
            if mode == "exact_text":
                observed: Any = text.rstrip("\r\n")
            elif mode == "json_path":
                observed = _json_path_value(json.loads(text), record.get("selector"))
            else:
                selector = record.get("selector")
                if not isinstance(selector, str):
                    raise ValueError("regex selector must be a string")
                pattern = re.compile(selector)
                if pattern.groups != 1:
                    raise ValueError("regex selector must contain one capture group")
                match = pattern.search(text)
                if match is None:
                    raise ValueError("regex selector does not match")
                observed = match.group(1)
        except (OSError, UnicodeDecodeError, json.JSONDecodeError, re.error, ValueError) as exc:
            result.errors.append(f"pins.json: pin {pin_id} cannot be evaluated ({exc})")
            continue
        if observed != expected:
            result.errors.append(
                f"pins.json: pin {pin_id} mismatch: expected {expected!r}, observed {observed!r}"
            )


def _validate_catalog(
    vault_root: Path, document: Any, records: list[Any], result: ValidationResult
) -> None:
    paths: set[str] = set()
    supersession_graph: dict[str, list[str]] = {}
    for record in records:
        if not isinstance(record, dict):
            result.errors.append("note-catalog.json: note records must be objects")
            continue
        path = record.get("path")
        if not isinstance(path, str) or not path.strip():
            result.errors.append("note-catalog.json: note record requires path")
            continue
        if path in paths:
            result.errors.append(f"note-catalog.json: duplicate path {path!r}")
        paths.add(path)
        if record.get("authorityClass") not in AUTHORITY_CLASSES:
            result.errors.append(
                f"note-catalog.json: {path!r} has invalid authorityClass"
            )
        if not isinstance(record.get("reason"), str) or not record["reason"].strip():
            result.errors.append(f"note-catalog.json: {path!r} requires reason")
        superseded_by = record.get("supersededBy")
        if superseded_by is not None and not isinstance(superseded_by, str):
            result.errors.append(
                f"note-catalog.json: {path!r} has invalid supersededBy"
            )
        elif isinstance(superseded_by, str):
            target = (vault_root / superseded_by).resolve()
            if not target.is_relative_to(vault_root.resolve()) or not target.is_file():
                result.errors.append(
                    f"note-catalog.json: {path!r} supersedes to missing target {superseded_by!r}"
                )
            if superseded_by in paths:
                supersession_graph.setdefault(path, []).append(superseded_by)

    manifest_path = vault_root / "manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest_paths = {
            record["name"]
            for record in manifest["files"]
            if isinstance(record, dict) and isinstance(record.get("name"), str)
        }
    except (FileNotFoundError, KeyError, TypeError, json.JSONDecodeError):
        return
    if paths != manifest_paths or document.get("rootNoteCount") != len(manifest_paths):
        result.errors.append("note-catalog.json: root-note coverage mismatch")
    catalog_targets = {
        record.get("path"): record.get("supersededBy")
        for record in records
        if isinstance(record, dict)
    }
    graph = {
        path: [target]
        for path, target in catalog_targets.items()
        if isinstance(path, str) and isinstance(target, str) and target in catalog_targets
    }
    _validate_acyclic_graph(graph, "note-catalog.json: supersession cycle", result)


def validate_living_knowledge(
    vault_root: Path,
    repository_root: Path,
    git_blob_reader: GitBlobReader | None = None,
) -> ValidationResult:
    """Validate living-knowledge registries without network or mutation."""

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
    sources = _validate_sources(
        source_records,
        repository_root,
        result,
        git_blob_reader or _read_git_blob,
    )

    claim_records = _records(
        documents.get("claims.json", {}),
        "claims.json",
        "claims",
        result,
    ) if "claims.json" in documents else []
    claim_ids = _validate_claims(claim_records, sources, result)

    if "note-catalog.json" in documents:
        catalog_records = _records(
            documents["note-catalog.json"],
            "note-catalog.json",
            "notes",
            result,
        )
        _validate_catalog(
            vault_root,
            documents["note-catalog.json"],
            catalog_records,
            result,
        )
    if "pins.json" in documents:
        pin_records = _records(documents["pins.json"], "pins.json", "pins", result)
        _validate_pins(repository_root, pin_records, result)
    note_commits = _validate_current_notes(vault_root, claim_ids, result)
    source_commits = {
        record.get("repositoryCommit")
        for record in sources.values()
        if record.get("type") == "repository"
    }
    anchors = note_commits | source_commits
    if len(anchors) != 1:
        result.errors.append(
            "living knowledge must use exactly one repository commit anchor"
        )
    _validate_authority_locations(vault_root, result)
    _validate_living_manifest(vault_root, result)
    return result
