import copy
import json
import os
import shutil
import subprocess
import sys
import unittest
import uuid
from pathlib import Path

from validate_criterion import compare


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
        for timing in (0, -1, float("nan"), float("inf"), True):
            with self.subTest(timing=timing):
                value = export("base")
                value["benchmarks"][CASE]["criterion_estimates_v1"]["mean"]["point_estimate"] = timing
                self.write(self.base, value)
                self.rejected("finite positive")

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


if __name__ == "__main__":
    unittest.main()
