from pathlib import Path
import tempfile

from scripts.jasterish_regression import Case, compare_output, discover_cases, run_compiler_case


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


def test_compare_exact_pass():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text("hello\n")
        ok, detail = compare_output("hello\n", golden, "exact")
        assert ok is True
        assert detail == ""


def test_compare_exact_fail():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text("hello\n")
        ok, detail = compare_output("world\n", golden, "exact")
        assert ok is False
        assert "mismatch" in detail.lower()


def test_compare_contains_pass():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text("BOOT\nJMK>\n")
        ok, detail = compare_output("BOOT\nJMK> prompt\n", golden, "contains")
        assert ok is True


def test_compare_regex_pass():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text(r"^JMK>\s*$")
        ok, detail = compare_output("JMK> \n", golden, "regex")
        assert ok is True


def test_run_compiler_case_skips_without_golden():
    with tempfile.TemporaryDirectory() as tmp:
        case_dir = Path(tmp) / "print-literal"
        case_dir.mkdir()
        (case_dir / "main.jstr").write_text("print 42")
        case = Case(root=case_dir, name="print-literal", kind="compiler", archs=["x86_64"], timeout=30, compare="exact")
        result = run_compiler_case(case, "x86_64", update=False)
        assert result.status == "SKIP"
