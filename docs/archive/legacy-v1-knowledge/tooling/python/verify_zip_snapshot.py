"""Verify that a ZIP source snapshot was extracted without regular-file drift.

Usage:
    python _source_review/verify_zip_snapshot.py ARCHIVE.zip EXTRACT_PARENT

EXTRACT_PARENT is the directory below which the ZIP's own root directory was
created. Symlinks are inventoried separately because Windows may not
materialize them without Developer Mode or elevated privileges.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import stat
import sys
import zipfile
from pathlib import Path, PurePosixPath


CHUNK_SIZE = 1024 * 1024


def sha256_stream(stream) -> str:
    digest = hashlib.sha256()
    while chunk := stream.read(CHUNK_SIZE):
        digest.update(chunk)
    return digest.hexdigest().upper()


def archive_sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return sha256_stream(stream)


def is_unsafe_member(name: str) -> bool:
    path = PurePosixPath(name)
    return path.is_absolute() or ".." in path.parts or bool(path.drive)


def verify(archive: Path, extract_parent: Path) -> dict[str, object]:
    archive = archive.resolve(strict=True)
    extract_parent = extract_parent.resolve(strict=True)

    regular_files = 0
    directories = 0
    symlinks: list[dict[str, str]] = []
    missing: list[str] = []
    size_mismatches: list[str] = []
    hash_mismatches: list[str] = []
    unsafe_members: list[str] = []
    casefold_names: dict[str, str] = {}
    case_collisions: list[list[str]] = []

    with zipfile.ZipFile(archive) as source:
        for entry in source.infolist():
            name = entry.filename
            if is_unsafe_member(name):
                unsafe_members.append(name)
                continue

            folded = name.casefold()
            prior = casefold_names.get(folded)
            if prior is not None and prior != name:
                case_collisions.append([prior, name])
            else:
                casefold_names[folded] = name

            mode = entry.external_attr >> 16
            if entry.is_dir():
                directories += 1
                continue
            if stat.S_ISLNK(mode):
                symlinks.append(
                    {
                        "path": name,
                        "target": source.read(entry).decode("utf-8", errors="replace"),
                    }
                )
                continue

            regular_files += 1
            extracted = extract_parent.joinpath(*PurePosixPath(name).parts)
            if not extracted.is_file():
                missing.append(name)
                continue
            if extracted.stat().st_size != entry.file_size:
                size_mismatches.append(name)
                continue

            with source.open(entry, "r") as expected, extracted.open("rb") as actual:
                if sha256_stream(expected) != sha256_stream(actual):
                    hash_mismatches.append(name)

    return {
        "archive": str(archive),
        "archive_sha256": archive_sha256(archive),
        "extract_parent": str(extract_parent),
        "regular_files": regular_files,
        "directories": directories,
        "symlinks": len(symlinks),
        "symlink_entries": symlinks,
        "missing_regular_files": missing,
        "size_mismatches": size_mismatches,
        "hash_mismatches": hash_mismatches,
        "unsafe_members": unsafe_members,
        "case_collisions": case_collisions,
        "regular_file_verification": (
            "passed"
            if not missing
            and not size_mismatches
            and not hash_mismatches
            and not unsafe_members
            and not case_collisions
            else "failed"
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("extract_parent", type=Path)
    args = parser.parse_args()
    result = verify(args.archive, args.extract_parent)
    print(json.dumps(result, indent=2))
    return 0 if result["regular_file_verification"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
