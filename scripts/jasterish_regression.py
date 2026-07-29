from __future__ import annotations

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
