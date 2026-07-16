from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


SOURCE_REVIEW = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SOURCE_REVIEW))

from living_knowledge import validate_living_knowledge


class LivingKnowledgeValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        base = Path(self.temp.name)
        self.repo = base / "repo"
        self.vault = self.repo / "bmad-runtime-lib"
        (self.vault / "knowledge-base" / "evidence").mkdir(parents=True)

    def write_json(self, name: str, value: object) -> None:
        path = self.vault / "knowledge-base" / "evidence" / name
        path.write_text(json.dumps(value), encoding="utf-8", newline="\n")

    def write_minimum_registries(self, claim_id: str = "KB-CORE-001") -> None:
        self.write_json(
            "claims.json",
            {
                "schemaVersion": "sapphirus.living-knowledge.v1",
                "claims": [
                    {
                        "id": claim_id,
                        "statement": "The fixture claim is evidence-backed.",
                        "classification": "IMPLEMENTED_FACT",
                        "implementationStatus": "implemented",
                        "confidence": "high",
                        "sourceIds": ["SRC-REPO-001"],
                        "observedAt": "2026-07-16",
                        "limitations": "Fixture only.",
                        "supersedes": [],
                        "singleSourceException": "Fixture uses one local source.",
                    }
                ],
            },
        )
        self.write_json(
            "sources.json",
            {
                "schemaVersion": "sapphirus.living-knowledge.v1",
                "sources": [
                    {
                        "id": "SRC-REPO-001",
                        "type": "repository",
                        "authority": "primary",
                        "locator": "README.md",
                        "retrievedAt": "2026-07-16",
                        "repositoryCommit": "a" * 40,
                    }
                ],
            },
        )
        self.write_json(
            "note-catalog.json",
            {
                "schemaVersion": "sapphirus.living-knowledge.v1",
                "rootNoteCount": 0,
                "notes": [],
            },
        )
        self.write_json(
            "pins.json",
            {"schemaVersion": "sapphirus.living-knowledge.v1", "pins": []},
        )

    def write_root_manifest(self, names: list[str]) -> None:
        (self.vault / "manifest.json").write_text(
            json.dumps(
                {
                    "files": [
                        {"name": name, "lines": 1, "bytes": 1, "sha256": "0" * 64}
                        for name in names
                    ]
                }
            ),
            encoding="utf-8",
            newline="\n",
        )

    def test_missing_registries_fail_closed(self) -> None:
        result = validate_living_knowledge(self.vault, self.repo)
        self.assertIn(
            "knowledge-base/evidence/claims.json is missing",
            result.errors,
        )
        self.assertIn(
            "knowledge-base/evidence/sources.json is missing",
            result.errors,
        )
        self.assertIn(
            "knowledge-base/evidence/note-catalog.json is missing",
            result.errors,
        )
        self.assertIn(
            "knowledge-base/evidence/pins.json is missing",
            result.errors,
        )

    def test_claim_ids_are_closed(self) -> None:
        self.write_minimum_registries(claim_id="claim-one")

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn("claims.json: invalid claim id 'claim-one'", result.errors)

    def test_catalog_covers_every_root_manifest_note(self) -> None:
        self.write_minimum_registries()
        self.write_root_manifest(["00 - Example.md"])

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "note-catalog.json: root-note coverage mismatch",
            result.errors,
        )

    def test_current_authority_requires_eight_notes(self) -> None:
        self.write_minimum_registries()

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "knowledge-base/current: expected 8 authority notes, found 0",
            result.errors,
        )

    def test_claim_requires_two_sources_or_an_exception(self) -> None:
        self.write_minimum_registries()
        claims_path = self.vault / "knowledge-base" / "evidence" / "claims.json"
        claims = json.loads(claims_path.read_text(encoding="utf-8"))
        claims["claims"][0]["singleSourceException"] = ""
        claims_path.write_text(json.dumps(claims), encoding="utf-8", newline="\n")

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "claims.json: claim 'KB-CORE-001' requires two sources or singleSourceException",
            result.errors,
        )

    def test_json_pin_drift_is_reported(self) -> None:
        self.write_minimum_registries()
        self.write_json(
            "pins.json",
            {
                "schemaVersion": "sapphirus.living-knowledge.v1",
                "pins": [
                    {
                        "id": "pnpm",
                        "mode": "json_path",
                        "path": "package.json",
                        "selector": ["packageManager"],
                        "expected": "pnpm@11.12.0",
                    }
                ],
            },
        )
        (self.repo / "package.json").write_text(
            json.dumps({"packageManager": "pnpm@11.9.0"}),
            encoding="utf-8",
            newline="\n",
        )

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "pins.json: pin pnpm mismatch: expected 'pnpm@11.12.0', observed 'pnpm@11.9.0'",
            result.errors,
        )

    def test_current_authority_is_rejected_outside_current_directory(self) -> None:
        self.write_minimum_registries()
        (self.vault / "Legacy.md").write_text(
            "---\nauthority: current\n---\n\n# Legacy\n",
            encoding="utf-8",
            newline="\n",
        )

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "Legacy.md: current authority is allowed only in knowledge-base/current",
            result.errors,
        )

    def test_legacy_status_current_is_rejected(self) -> None:
        self.write_minimum_registries()
        (self.vault / "Legacy.md").write_text(
            "---\nstatus: current\n---\n\n# Legacy\n",
            encoding="utf-8",
            newline="\n",
        )

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "Legacy.md: legacy root note cannot declare status current",
            result.errors,
        )

    def test_living_manifest_rejects_stale_file_set(self) -> None:
        self.write_minimum_registries()
        manifest_path = self.vault / "knowledge-base" / "manifest.json"
        manifest_path.write_text(
            json.dumps(
                {
                    "schemaVersion": "sapphirus.living-knowledge-manifest.v1",
                    "files": [],
                }
            ),
            encoding="utf-8",
            newline="\n",
        )

        result = validate_living_knowledge(self.vault, self.repo)

        self.assertIn(
            "knowledge-base/manifest.json: file set mismatch",
            result.errors,
        )


if __name__ == "__main__":
    unittest.main()
