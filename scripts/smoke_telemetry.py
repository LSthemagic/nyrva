#!/usr/bin/env python3
"""Black-box telemetry acceptance. Uses synthetic data and an isolated directory only."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import time


def run(binary: Path, output: Path | None) -> None:
    results: list[dict[str, str]] = []
    with tempfile.TemporaryDirectory(prefix="nyrva-smoke-") as temporary:
        root = Path(temporary) / "data"
        env = dict(os.environ, NYRVA_DATA_DIR=str(root))

        def call(*args: str, payload: str | None = None, expected: int = 0) -> str:
            completed = subprocess.run([str(binary), *args], input=payload, text=True, encoding="utf-8",
                                       capture_output=True, env=env, timeout=15, check=False)
            assert completed.returncode == expected, f"command {args[0]} exited {completed.returncode}: {completed.stderr[:300]}"
            return completed.stdout

        def passed(name: str) -> None:
            results.append({"scenario": name, "status": "passed"})

        value = json.loads(call("status", "--json"))
        assert value["schema_version"] == 1 and value["providers"] == []
        assert not root.exists()
        passed("read-only status does not create data or launch a GUI")

        payload = json.dumps({
            "quota": {"five_hour": {"remaining_fraction": 0.25, "reset_in_seconds": 3600},
                      "weekly": {"remaining_fraction": 0.6, "reset_in_seconds": 86400}},
            "model": {"id": "fixture-model"},
            "context_window": {"used_percentage": 42, "context_window_size": 1000000},
            "access_token": "TOP_SECRET_IN_FIXTURE", "prompt": "CONFIDENTIAL_PROMPT_IN_FIXTURE",
        })
        rendered = call("statusline", "antigravity", payload=payload)
        assert "25%" in rendered and "60%" in rendered and "context 42%" in rendered
        value = json.loads(call("status", "--json"))
        assert len(value["providers"]) == 1
        assert value["providers"][0]["status"] == "live"
        assert len(value["providers"][0]["buckets"]) == 2
        passed("statusline persists distinct quota buckets and context across processes")

        resets = json.loads(call("resets", "--json"))["resets"]
        assert len(resets) == 2 and all(r["reset_in_ms"] > 0 for r in resets)
        passed("reset query returns absolute timestamps and countdowns")

        call("ingest", "antigravity", "--account", "work", payload=payload)
        value = json.loads(call("status", "--account", "work", "--json"))
        assert len(value["providers"]) == 1 and value["providers"][0]["account_id"] == "work"
        assert len(json.loads(call("status", "--json"))["providers"]) == 2
        passed("explicit account aliases remain isolated")

        samples = json.loads(call("history", "antigravity", "--account", "active", "--json"))["samples"]
        assert len(samples) == 1
        serialized = json.dumps(samples)
        assert "TOP_SECRET_IN_FIXTURE" not in serialized and "CONFIDENTIAL_PROMPT_IN_FIXTURE" not in serialized
        for path in root.rglob("*"):
            if path.is_file():
                data = path.read_bytes()
                assert b"TOP_SECRET_IN_FIXTURE" not in data and b"CONFIDENTIAL_PROMPT_IN_FIXTURE" not in data
        passed("credentials and prompts are absent from output and storage")

        call("ingest", "antigravity", payload="SECRET MALFORMED INPUT", expected=1)
        call("status", "--unknown-flag", expected=1)
        call("ingest", "antigravity", "--account", "../unsafe", payload="{}", expected=1)
        call("ingest", "antigravity", payload=" " * 262145, expected=1)
        passed("invalid JSON, aliases, flags and oversized payloads fail explicitly")

        # Simulate an old observation, rather than waiting ten minutes in CI.
        database = root / "telemetry" / "history.sqlite3"
        with sqlite3.connect(database) as connection:
            old = int(time.time() * 1000) - 700000
            row = connection.execute("SELECT payload FROM observations WHERE account_id='active'").fetchone()
            cached = json.loads(row[0])
            cached["observed_at_ms"] = old
            for bucket in cached["buckets"]:
                bucket["resets_at_ms"] = old - 1000
            connection.execute("UPDATE observations SET observed_at_ms=?,payload=? WHERE account_id='active'", (old, json.dumps(cached)))
        cached = json.loads(call("status", "--account", "active", "--json"))["providers"][0]
        assert cached["status"] == "stale"
        assert cached["buckets"][0]["remaining_fraction"] == 0.25
        assert "reset awaiting confirmation" in call("status", "--account", "active")
        passed("expired resets preserve quota and stale observation time")

        with sqlite3.connect(database) as connection:
            connection.execute("PRAGMA user_version=99")
        call("status", "--json", expected=1)
        with sqlite3.connect(database) as connection:
            assert connection.execute("PRAGMA user_version").fetchone()[0] == 99
        passed("future database schema is refused without destructive migration")

    result = {"binary": binary.name, "status": "passed", "scenarios": results}
    text = json.dumps(result, indent=2, ensure_ascii=False)
    if output:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text + "\n", encoding="utf-8")
    print(text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    run(arguments.binary.resolve(strict=True), arguments.output)
