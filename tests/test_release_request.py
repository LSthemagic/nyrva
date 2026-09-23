"""Exercise release selection via actual files and CLI subprocesses."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/release_request.py"


class ReleaseRequestTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(SCRIPT.is_file(), "release_request.py has not been implemented")
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "nyrva").mkdir()
        (self.root / ".github").mkdir()
        (self.root / "docs/releases").mkdir(parents=True)
        (self.root / "nyrva/tauri.conf.json").write_text(
            '{"version":"0.3.0"}', encoding="utf-8")
        (self.root / "nyrva/Cargo.toml").write_text(
            '[package]\nversion="0.3.0"\n', encoding="utf-8")
        (self.root / "docs/releases/v0.3.0.md").write_text(
            '# Nyrva v0.3.0\nCredits and acceptance notes.\n', encoding="utf-8")
        self.request({"tag": "v0.3.0"})

    def request(self, data):
        (self.root / ".github/release-request.json").write_text(
            json.dumps(data), encoding="utf-8")

    def run_cli(self, event="push", ref_type="branch", ref_name="main"):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root),
             "--event-name", event, "--ref-type", ref_type, "--ref-name", ref_name],
            text=True, capture_output=True, check=False)

    def reject(self, result, reason):
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(reason, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertNotIn("Traceback", result.stderr)

    def test_main_request_selects_version_and_requires_tag_creation(self):
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {
            "tag": "v0.3.0", "create_tag": True,
            "notes": "docs/releases/v0.3.0.md"})

    def test_tag_push_does_not_request_tag_creation(self):
        result = self.run_cli(ref_type="tag", ref_name="v0.3.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(json.loads(result.stdout)["create_tag"])

    def test_tag_dispatch_is_supported(self):
        result = self.run_cli(event="workflow_dispatch", ref_type="tag", ref_name="v0.3.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(json.loads(result.stdout)["create_tag"])

    def test_tag_push_ignores_previous_request(self):
        self.request({"tag": "v9.9.9"})
        result = self.run_cli(ref_type="tag", ref_name="v0.3.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["tag"], "v0.3.0")

    def test_feature_branch_is_rejected(self):
        self.reject(self.run_cli(ref_name="feat/release"), "Unsupported release event")

    def test_main_workflow_dispatch_uses_reviewed_request(self):
        result = self.run_cli(event="workflow_dispatch")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {
            "tag": "v0.3.0", "create_tag": True,
            "notes": "docs/releases/v0.3.0.md"})

    def test_pull_request_and_other_branch_events_are_rejected(self):
        for event in ("pull_request", "pull_request_target", "workflow_run"):
            with self.subTest(event=event):
                self.reject(self.run_cli(event=event), "Unsupported release event")
        self.reject(
            self.run_cli(event="workflow_dispatch", ref_name="feat/release"),
            "Unsupported release event")

    def test_request_schema_rejects_publish_flag_and_invalid_types(self):
        for data in ({"tag": "v0.3.0", "publish": True}, {}, [], {"tag": 3}):
            with self.subTest(data=data):
                self.request(data)
                self.reject(self.run_cli(), "Request must contain only a string tag")

    def test_unsafe_tag_is_rejected_before_path_resolution(self):
        for tag in ("../../secrets", "v0.3.0;echo bad", "v0.3.0\n"):
            with self.subTest(tag=tag):
                self.request({"tag": tag})
                self.reject(self.run_cli(), "Invalid version tag")

    def test_mismatched_version_is_rejected(self):
        self.request({"tag": "v0.3.1"})
        self.reject(self.run_cli(), "does not match")

    def test_missing_or_empty_release_notes_are_rejected(self):
        path = self.root / "docs/releases/v0.3.0.md"
        path.unlink()
        self.reject(self.run_cli(), "Release notes must be a non-empty regular file")
        path.write_text(" \n", encoding="utf-8")
        self.reject(self.run_cli(), "Release notes must be a non-empty regular file")

    def test_malformed_request_is_a_clean_failure(self):
        (self.root / ".github/release-request.json").write_text("{", encoding="utf-8")
        self.reject(self.run_cli(), "Release request failed")

    def test_notes_symlink_is_rejected(self):
        path = self.root / "docs/releases/v0.3.0.md"
        path.unlink()
        target = self.root / "other.md"
        target.write_text("not the committed version notes", encoding="utf-8")
        path.symlink_to(target)
        self.reject(self.run_cli(), "Release notes must be a non-empty regular file")


if __name__ == "__main__":
    unittest.main()
