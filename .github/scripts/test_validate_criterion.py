import copy
import json
import os
import shutil
import subprocess
import sys
import unittest
import uuid
from pathlib import Path

from validate_criterion import compare, load_export


CASE = "jpeg: smoke/zune-jpeg"


def export(baseline, name=CASE):
    estimate = {
        "point_estimate": 100.0,
        "standard_error": 1.0,
        "confidence_interval": {
            "confidence_level": 0.95, "lower_bound": 98.0, "upper_bound": 102.0,
        },
    }
    return {
        "name": baseline,
        "benchmarks": {
            name: {
                "baseline": baseline,
                "fullname": f"{baseline}/{name}",
                "criterion_benchmark_v1": {
                    "group_id": name.split("/")[0],
                    "function_id": "zune-jpeg",
                    "value_str": None,
                    "throughput": None,
                    "full_id": name,
                    "directory_name": name,
                },
                "criterion_estimates_v1": {
                    **{key: copy.deepcopy(estimate) for key in ("mean", "median", "median_abs_dev", "std_dev")},
                    "slope": None,
                },
            },
        },
    }


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.root = Path.cwd() / "target" / "jpeg-pipeline-tests" / uuid.uuid4().hex
        self.root.mkdir(parents=True)
        self.base = self.root / "base.json"
        self.head = self.root / "head.json"
        self.report = self.root / "report"
        self.write(self.base, export("base"))
        self.write(self.head, export("head"))
        self.comparator = self.root / "comparator"
        self.fake_comparator(f"print({CASE!r})")

    def tearDown(self):
        shutil.rmtree(self.root)

    def write(self, path, value):
        path.write_text(json.dumps(value))

    def fake_comparator(self, body):
        self.comparator.write_text(f"#!{sys.executable}\n{body}\n")
        self.comparator.chmod(0o755)

    def run_compare(self):
        return compare(self.base, self.head, self.report, str(self.comparator))

    def rejected(self, message):
        with self.assertRaisesRegex(ValueError, message):
            self.run_compare()
        self.assertEqual(json.loads((self.report / "validation.json").read_text())["status"], "failed")

    def test_positive(self):
        self.assertIn(CASE, self.run_compare())
        self.assertEqual(json.loads((self.report / "validation.json").read_text())["status"], "passed")

    @unittest.skipUnless(shutil.which("critcmp") or os.environ.get("CRITCMP"), "critcmp is not installed")
    def test_real_comparator_accepts_export_format(self):
        output = compare(self.base, self.head, self.report, os.environ.get("CRITCMP", "critcmp"))
        self.assertIn(CASE, output)

    def test_empty_file(self):
        self.base.write_text("")
        self.rejected("Expecting value")

    def test_malformed_json(self):
        self.base.write_text("{")
        self.rejected("Expecting property")

    def test_empty_benchmarks(self):
        self.write(self.base, {"name": "base", "benchmarks": {}})
        self.rejected("empty or malformed")

    def test_malformed_schema(self):
        value = export("base")
        del value["benchmarks"][CASE]["criterion_estimates_v1"]["mean"]
        self.write(self.base, value)
        self.rejected("malformed mean")

    def test_zero_jpeg_cases(self):
        self.write(self.base, export("base", "png: smoke/zune-png"))
        self.rejected("zero JPEG")

    def test_no_matching_cases(self):
        self.write(self.head, export("head", "jpeg: different/zune-jpeg"))
        self.rejected("no matching JPEG")

    def test_partial_case_mismatch(self):
        value = export("head")
        value["benchmarks"].update(export("head", "jpeg: added/zune-jpeg")["benchmarks"])
        self.write(self.head, value)
        self.rejected("case sets differ")

    def test_wrong_baseline(self):
        self.write(self.base, export("new"))
        self.rejected("expected named baseline")

    def test_zero_and_nonfinite_measurements(self):
        for timing in (0, -1, float("nan"), float("inf"), 10 ** 400, True):
            with self.subTest(timing=timing):
                value = export("base")
                value["benchmarks"][CASE]["criterion_estimates_v1"]["mean"]["point_estimate"] = timing
                self.write(self.base, value)
                self.rejected("finite positive|nonfinite JSON")

    def test_comparator_failure_preserves_diagnostics(self):
        self.fake_comparator("import sys\nprint('broken comparator', file=sys.stderr)\nsys.exit(7)")
        self.rejected("critcmp failed with exit status 7")
        self.assertIn("broken comparator", (self.report / "comparator.stderr.log").read_text())

    def test_empty_comparator_output(self):
        self.fake_comparator("pass")
        self.rejected("empty comparison")

    def test_header_only_comparison(self):
        self.fake_comparator("print('base head')")
        self.rejected("omitted matching")

    def test_missing_comparator_preserves_diagnostics(self):
        self.comparator.unlink()
        with self.assertRaises(FileNotFoundError):
            self.run_compare()
        self.assertIn("error", json.loads((self.report / "validation.json").read_text()))

    def test_comparator_failure_is_not_masked_by_tee(self):
        self.fake_comparator("import sys\nsys.exit(9)")
        script = Path(__file__).with_name("validate_criterion.py")
        completed = subprocess.run(
            [
                "bash", "-o", "pipefail", "-c", '"$@" 2>&1 | tee "$PIPELINE_TEST_LOG"', "_",
                sys.executable, str(script), str(self.base), str(self.head),
                str(self.report), "--critcmp", str(self.comparator),
            ],
            env={**os.environ, "PIPELINE_TEST_LOG": str(self.root / "pipeline.log")},
            capture_output=True, text=True, check=False,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("exit status 9", (self.root / "pipeline.log").read_text())

    @unittest.skipUnless(shutil.which("critcmp") or os.environ.get("CRITCMP"), "critcmp is not installed")
    def test_benchmark_failure_retains_raw_and_partial_export(self):
        base, head = self.root / "checkout-base", self.root / "checkout-head"
        base.mkdir()
        for suffix in (
            "", "_horizontal_subsampling", "_vertical_subsampling", "_hv_subsampling",
            "_prog", "_prog_h_sampling", "_prog_v_sampling", "_prog_hv_sampling",
        ):
            fixture = base / f"test-images/jpeg/benchmarks/speed_bench{suffix}.jpg"
            fixture.parent.mkdir(parents=True, exist_ok=True)
            fixture.write_bytes(b"fixture")
        (base / "test-images/jpeg/four_components.jpg").write_bytes(b"fixture")
        (base / "Cargo.lock").write_text("# test lockfile\n")
        subprocess.run(["git", "init", "-q", str(base)], check=True)
        subprocess.run(["git", "-C", str(base), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(base), "-c", "user.name=Test", "-c", "user.email=test@example.com",
             "commit", "-qm", "test fixture"],
            check=True,
        )
        subprocess.run(["git", "clone", "-q", "--no-hardlinks", str(base), str(head)], check=True)
        sha = subprocess.check_output(["git", "-C", str(base), "rev-parse", "HEAD"], text=True).strip()
        tools = self.root / "tools"
        tools.mkdir()
        cargo = tools / "cargo"
        cargo.write_text(
            f"#!{sys.executable}\n"
            "import json, os, pathlib, sys\n"
            "if '-V' in sys.argv:\n"
            "    print('cargo test stub'); sys.exit(0)\n"
            "variant = sys.argv[sys.argv.index('--save-baseline') + 1]\n"
            "case = next(iter(json.load(open(os.environ['JPEG_TEST_EXPORT']))['benchmarks'].values()))\n"
            "root = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / 'criterion' / 'smoke' / variant\n"
            "root.mkdir(parents=True)\n"
            "(root / 'benchmark.json').write_text(json.dumps(case['criterion_benchmark_v1']))\n"
            "(root / 'estimates.json').write_text(json.dumps(case['criterion_estimates_v1']))\n"
            "print('simulated benchmark failure after partial results')\n"
            "sys.exit(23)\n"
        )
        cargo.chmod(0o755)
        runner = Path(__file__).with_name("jpeg-benchmark.sh")
        completed = subprocess.run(
            ["bash", str(runner), str(base), str(head), str(self.report), sha, sha],
            env={
                **os.environ,
                "PATH": str(tools) + os.pathsep + os.environ["PATH"],
                "CRITCMP": os.environ.get("CRITCMP") or shutil.which("critcmp"),
                "JPEG_TEST_EXPORT": str(self.base),
            },
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(completed.returncode, 23, completed.stdout + completed.stderr)
        self.assertEqual((self.report / "status.txt").read_text(), "exit_status=23\n")
        self.assertTrue(list((self.report / "raw/base").glob("**/estimates.json")))
        self.assertEqual(load_export(self.report / "base.json", "base"), {CASE})
        self.assertIn("simulated benchmark failure", (self.report / "base/benchmark.log").read_text())


if __name__ == "__main__":
    unittest.main()
