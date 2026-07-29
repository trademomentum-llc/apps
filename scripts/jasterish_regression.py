from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Case:
    root: Path
    name: str
    kind: str
    archs: list[str]
    timeout: int
    compare: str

    @property
    def source_path(self) -> Path:
        return self.root / "main.jstr"


def discover_cases(root: Path) -> list[Case]:
    cases: list[Case] = []
    if not root.exists():
        return cases
    for entry in sorted(root.iterdir()):
        if not entry.is_dir():
            continue
        manifest = entry / "test.toml"
        if not manifest.exists():
            continue
        data = tomllib.loads(manifest.read_text())
        cases.append(
            Case(
                root=entry,
                name=data.get("name", entry.name),
                kind=data["kind"],
                archs=data.get("archs", ["x86_64"]),
                timeout=data.get("timeout", 30),
                compare=data.get("compare", "exact"),
            )
        )
    return cases


def _normalize_trailing_newlines(text: str) -> str:
    return text.rstrip("\n") + "\n" if text else text


def compare_output(actual: str, golden_path: Path, mode: str) -> tuple[bool, str]:
    if not golden_path.exists():
        return False, f"missing golden file: {golden_path}"

    golden = golden_path.read_text()

    if mode == "exact":
        if _normalize_trailing_newlines(actual) == _normalize_trailing_newlines(golden):
            return True, ""
        return False, f"output mismatch:\n--- expected ---\n{golden}\n--- actual ---\n{actual}"

    if mode == "contains":
        missing = [line for line in golden.splitlines() if line and line not in actual]
        if not missing:
            return True, ""
        return False, f"missing fragments: {missing}"

    if mode == "regex":
        missing = []
        for line in golden.splitlines():
            if not line:
                continue
            if not re.search(line, actual):
                missing.append(line)
        if not missing:
            return True, ""
        return False, f"unmatched regexes: {missing}"

    return False, f"unknown compare mode: {mode}"
