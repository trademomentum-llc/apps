import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from scripts import generate_release_provenance as provenance
from scripts import jasterish_orchestrator as orchestrator
from scripts import jasterish_regression as regression


class CommandExecutionBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.root = Path(self.temporary_directory.name)

    def case(self) -> regression.Case:
        return regression.Case(
            root=self.root,
            name="boundary-test",
            kind="compiler",
            archs=["x86_64"],
            timeout=1,
            compare="exact",
        )

    def test_compiler_case_rejects_unapproved_architecture_before_launch(self) -> None:
        with patch.object(
            regression.subprocess,
            "run",
            side_effect=AssertionError("subprocess must not start"),
        ) as subprocess_run:
            result = regression.run_compiler_case(
                self.case(),
                "x86_64;touch-pwned",
                update=True,
            )

        self.assertEqual(result.status, "FAIL")
        self.assertIn("unsupported architecture", result.detail)
        subprocess_run.assert_not_called()

    def test_kernel_case_rejects_unapproved_architecture_before_launch(self) -> None:
        with patch.object(
            regression.subprocess,
            "run",
            side_effect=AssertionError("subprocess must not start"),
        ) as subprocess_run:
            result = regression.run_kernel_case(
                self.case(),
                "aarch64;touch-pwned",
                update=False,
                kernel_dir=self.root,
            )

        self.assertEqual(result.status, "FAIL")
        self.assertIn("unsupported architecture", result.detail)
        subprocess_run.assert_not_called()

    def test_self_host_case_rejects_unapproved_architecture_before_launch(self) -> None:
        with patch.object(
            regression.subprocess,
            "run",
            side_effect=AssertionError("subprocess must not start"),
        ) as subprocess_run:
            result = regression.run_self_host_case(
                self.case(),
                "../../bin/sh",
                update=False,
            )

        self.assertEqual(result.status, "FAIL")
        self.assertIn("unsupported architecture", result.detail)
        subprocess_run.assert_not_called()

    def test_compiler_override_rejects_non_repository_executable(self) -> None:
        with patch.dict(os.environ, {"JASTERISH_COMPILER": "/bin/sh"}):
            with self.assertRaisesRegex(ValueError, "repository-built compiler"):
                regression._resolve_compiler()

    def test_compiler_command_uses_fixed_case_local_paths(self) -> None:
        (self.root / "main.jstr").write_text("print 42\n", encoding="utf-8")
        (self.root / "expected.x86_64").write_text("42\n", encoding="utf-8")
        commands: list[list[str]] = []
        working_directories: list[Path] = []

        def fake_run(command, **kwargs):
            commands.append(command)
            working_directories.append(kwargs["cwd"])
            if "compile" in command:
                return SimpleNamespace(returncode=0, stdout="", stderr="")
            return SimpleNamespace(returncode=0, stdout="42\n", stderr="")

        with patch.object(regression, "_resolve_compiler", return_value=Path("/trusted/morphlex")):
            with patch.object(regression.subprocess, "run", side_effect=fake_run):
                result = regression.run_compiler_case(self.case(), "x86_64", update=False)

        self.assertEqual(result.status, "PASS")
        self.assertEqual(
            commands[0],
            [
                "/trusted/morphlex",
                "jstar",
                "compile",
                "--target",
                "x86_64",
                "--input",
                "main.jstr",
                "--output",
                "actual.x86_64.elf",
            ],
        )
        self.assertEqual(commands[1], ["./actual.x86_64.elf"])
        self.assertEqual(working_directories, [self.root, self.root])
        self.assertTrue(all(str(self.root) not in argument for command in commands for argument in command))

    def test_self_host_compiler_command_uses_fixed_case_local_paths(self) -> None:
        (self.root / "main.jstr").write_text("compiler source\n", encoding="utf-8")
        captured: dict[str, object] = {}

        def fake_run(command, **kwargs):
            captured["command"] = command
            captured["cwd"] = kwargs["cwd"]
            return SimpleNamespace(returncode=1, stdout="", stderr="expected stop")

        with patch.object(regression, "_resolve_compiler", return_value=Path("/trusted/morphlex")):
            with patch.object(regression.subprocess, "run", side_effect=fake_run):
                result = regression.run_self_host_case(self.case(), "x86_64", update=False)

        self.assertEqual(result.status, "FAIL")
        self.assertEqual(
            captured["command"],
            [
                "/trusted/morphlex",
                "jstar",
                "compile",
                "--target",
                "x86_64",
                "--input",
                "main.jstr",
                "--output",
                "work.x86_64/stage0.elf",
            ],
        )
        self.assertEqual(captured["cwd"], self.root)
        self.assertNotIn(str(self.root), captured["command"])

    def test_orchestrator_rejects_corpus_outside_approved_root(self) -> None:
        allowed = self.root / "allowed"
        outside = self.root / "outside"
        allowed.mkdir()
        outside.mkdir()

        with self.assertRaisesRegex(ValueError, "must be a directory within"):
            orchestrator._confined_directory(outside, allowed, "test")

    def test_orchestrator_keeps_corpus_path_out_of_command_line(self) -> None:
        project_root = self.root / "project"
        corpus_root = project_root / "tests" / "regression"
        corpus = corpus_root / "selected"
        script = project_root / "scripts" / "runner.py"
        corpus.mkdir(parents=True)
        script.parent.mkdir(parents=True)
        script.write_text("raise SystemExit(0)\n", encoding="utf-8")
        suite = orchestrator.Suite("test", project_root, script, corpus_root)
        captured: dict[str, object] = {}

        def fake_run(command, **kwargs):
            captured["command"] = command
            captured["kwargs"] = kwargs
            return SimpleNamespace(returncode=0)

        with patch.object(orchestrator.subprocess, "run", side_effect=fake_run):
            result = orchestrator.run_suite(
                suite,
                corpus,
                ["aarch64"],
                False,
                False,
                [],
            )

        command = captured["command"]
        kwargs = captured["kwargs"]
        self.assertEqual(result, 0)
        self.assertEqual(command[-2:], ["--", "."])
        self.assertNotIn(str(corpus), command)
        self.assertEqual(command[2:4], ["--arch", "aarch64"])
        self.assertEqual(kwargs["cwd"], corpus.resolve())
        self.assertIs(kwargs["shell"], False)

    def test_orchestrator_rejects_unapproved_architecture_before_launch(self) -> None:
        project_root = self.root / "project"
        corpus_root = project_root / "tests" / "regression"
        script = project_root / "scripts" / "runner.py"
        corpus_root.mkdir(parents=True)
        script.parent.mkdir(parents=True)
        script.write_text("raise SystemExit(0)\n", encoding="utf-8")
        suite = orchestrator.Suite("test", project_root, script, corpus_root)

        with patch.object(
            orchestrator.subprocess,
            "run",
            side_effect=AssertionError("subprocess must not start"),
        ) as subprocess_run:
            with self.assertRaisesRegex(ValueError, "unsupported architecture"):
                orchestrator.run_suite(
                    suite,
                    corpus_root,
                    ["$(touch pwned)"],
                    False,
                    False,
                    [],
                )

        subprocess_run.assert_not_called()

    def test_private_key_must_remain_outside_repository(self) -> None:
        repo = self.root / "repo"
        repo.mkdir()

        with self.assertRaisesRegex(SystemExit, "outside the repository"):
            provenance._private_key_path(repo.resolve(), str(repo / "release.key"))

    def test_openssl_commands_never_receive_private_key_path(self) -> None:
        private_key = self.root / "private" / "release.key"
        manifest = self.root / "manifest.sha256"
        signature = self.root / "manifest.sha256.sig"
        commands: list[list[str]] = []
        manifest.write_bytes(b"digest  artifact\n")

        def fake_run(command, **kwargs):
            commands.append(command)
            if "genpkey" in command:
                output = b"PRIVATE KEY"
            elif "-pubout" in command:
                output = b"PUBLIC KEY"
            else:
                output = b"SIGNATURE"
            return SimpleNamespace(returncode=0, stdout=output, stderr=b"")

        private_key.parent.mkdir()
        with patch.object(provenance.subprocess, "run", side_effect=fake_run):
            provenance.ensure_signing_key(private_key, create=True)
            public_key = provenance._openssl_output("public-key", private_key.read_bytes())
            provenance._sign_manifest(private_key, manifest, signature)

        self.assertEqual(public_key, b"PUBLIC KEY")
        self.assertEqual(signature.read_bytes(), b"SIGNATURE")
        self.assertEqual(private_key.stat().st_mode & 0o777, 0o600)
        self.assertTrue(
            all(
                str(private_key) not in argument
                for command in commands
                for argument in command
            )
        )

    def test_openssl_signing_boundary_with_real_executable(self) -> None:
        try:
            provenance._trusted_executable("openssl")
        except FileNotFoundError as exc:
            self.skipTest(str(exc))

        private_key = self.root / "private" / "release.key"
        manifest = self.root / "manifest.sha256"
        signature = self.root / "manifest.sha256.sig"
        private_key.parent.mkdir()
        manifest.write_bytes(b"digest  artifact\n")

        provenance.ensure_signing_key(private_key, create=True)
        public_key = provenance._openssl_output("public-key", private_key.read_bytes())
        provenance._sign_manifest(private_key, manifest, signature)

        self.assertIn(b"BEGIN PUBLIC KEY", public_key)
        self.assertGreater(len(signature.read_bytes()), 0)
        self.assertEqual(private_key.stat().st_mode & 0o777, 0o600)

    def test_git_operation_is_allowlisted_before_launch(self) -> None:
        with patch.object(
            provenance.subprocess,
            "run",
            side_effect=AssertionError("subprocess must not start"),
        ) as subprocess_run:
            with self.assertRaisesRegex(ValueError, "unsupported Git operation"):
                provenance.git(self.root, "status;touch-pwned")

        subprocess_run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
