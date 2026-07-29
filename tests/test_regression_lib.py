from pathlib import Path
import tempfile

from scripts.jasterish_regression import discover_cases


def test_discover_cases_finds_test_toml():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        case_dir = root / "print-literal"
        case_dir.mkdir()
        (case_dir / "test.toml").write_text('name = "print-literal"\nkind = "compiler"\n')
        (case_dir / "main.jstr").write_text('print 42')
        cases = discover_cases(root)
        assert len(cases) == 1
        assert cases[0].name == "print-literal"
