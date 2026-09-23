#!/usr/bin/env python3
"""Resolve a draft release from a version tag or a reviewed main-branch request.

This selects identity only. The workflow must build this commit successfully,
then create/verify its tag before attaching packages to a draft release.
"""
import argparse
import json
from pathlib import Path
import sys

from release_tools import validate_tag


def resolve_release(root: Path, event_name: str, ref_type: str, ref_name: str) -> dict:
    if event_name in ("push", "workflow_dispatch") and ref_type == "tag":
        tag = ref_name
        create_tag = False
    elif event_name in ("push", "workflow_dispatch") and ref_type == "branch" and ref_name == "main":
        request = json.loads((root / ".github/release-request.json").read_text(encoding="utf-8"))
        if not isinstance(request, dict) or set(request) != {"tag"} or not isinstance(request["tag"], str):
            raise ValueError("Request must contain only a string tag")
        tag = request["tag"]
        create_tag = True
    else:
        raise ValueError("Unsupported release event; use a version tag or a reviewed main request")

    # Validate the version convention, not the existence of a Git tag. The
    # request path creates that tag only after the build gates have passed.
    validate_tag(root, "tag", tag)
    relative_notes = f"docs/releases/{tag}.md"
    notes = root / relative_notes
    if notes.is_symlink() or not notes.is_file() or not notes.read_text(encoding="utf-8").strip():
        raise ValueError("Release notes must be a non-empty regular file")
    return {"tag": tag, "create_tag": create_tag, "notes": relative_notes}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--event-name", required=True)
    parser.add_argument("--ref-type", required=True)
    parser.add_argument("--ref-name", required=True)
    args = parser.parse_args()
    try:
        result = resolve_release(args.root, args.event_name, args.ref_type, args.ref_name)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Release request failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
