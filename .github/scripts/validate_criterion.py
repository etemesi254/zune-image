#!/usr/bin/env python3
"""Fail closed on unusable critcmp 0.1.8 JPEG exports and comparisons."""

import argparse
import json
import math
import subprocess
from pathlib import Path


def number(value, context, *, positive=False):
    if (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not math.isfinite(value)
        or (value <= 0 if positive else value < 0)
    ):
        raise ValueError(f"{context}: expected a finite {'positive' if positive else 'nonnegative'} number")


def load_export(path, baseline):
    data = json.loads(Path(path).read_text())
    if not isinstance(data, dict) or data.get("name") != baseline:
        raise ValueError(f"{path}: expected named baseline {baseline!r}")
    cases = data.get("benchmarks")
    if not isinstance(cases, dict) or not cases:
        raise ValueError(f"{path}: empty or malformed benchmark export")
    if not any(case.startswith("jpeg:") for case in cases):
        raise ValueError(f"{path}: zero JPEG cases")
    for name, case in cases.items():
        if not name.startswith("jpeg:") or not isinstance(case, dict):
            raise ValueError(f"{path}: unexpected or malformed JPEG case {name!r}")
        if case.get("baseline") != baseline or case.get("fullname") != f"{baseline}/{name}":
            raise ValueError(f"{path}: incorrect baseline metadata for {name!r}")
        metadata = case.get("criterion_benchmark_v1")
        if (
            not isinstance(metadata, dict)
            or metadata.get("full_id") != name
            or not isinstance(metadata.get("group_id"), str)
            or not metadata["group_id"].startswith("jpeg:")
            or not isinstance(metadata.get("directory_name"), str)
        ):
            raise ValueError(f"{path}: malformed Criterion metadata for {name!r}")
        estimates = case.get("criterion_estimates_v1")
        if not isinstance(estimates, dict):
            raise ValueError(f"{path}: missing estimates for {name!r}")
        for statistic in ("mean", "median", "median_abs_dev", "std_dev", "slope"):
            estimate = estimates.get(statistic)
            if statistic == "slope" and estimate is None:
                continue
            if not isinstance(estimate, dict):
                raise ValueError(f"{path}: malformed {statistic} estimate for {name!r}")
            positive = statistic in ("mean", "median", "slope")
            number(estimate.get("point_estimate"), statistic, positive=positive)
            number(estimate.get("standard_error"), f"{statistic} standard error")
            interval = estimate.get("confidence_interval")
            if not isinstance(interval, dict):
                raise ValueError(f"{path}: missing {statistic} confidence interval")
            number(interval.get("confidence_level"), "confidence level", positive=True)
            number(interval.get("lower_bound"), "lower bound")
            number(interval.get("upper_bound"), "upper bound")
            if interval["confidence_level"] > 1 or interval["lower_bound"] > interval["upper_bound"]:
                raise ValueError(f"{path}: invalid {statistic} confidence interval")
    return set(cases)


def compare(base, head, report, critcmp="critcmp"):
    report = Path(report)
    report.mkdir(parents=True, exist_ok=True)
    result = {"status": "failed"}
    try:
        base_cases = load_export(base, "base")
        head_cases = load_export(head, "head")
        matching = base_cases & head_cases
        if not matching:
            raise ValueError("no matching JPEG cases between base and head")
        if base_cases != head_cases:
            raise ValueError(
                f"JPEG case sets differ: base-only={sorted(base_cases - head_cases)}, "
                f"head-only={sorted(head_cases - base_cases)}"
            )
        result["matching_cases"] = sorted(matching)
        # Do not let critcmp discover unrelated baselines in the caller's target.
        completed = subprocess.run(
            [
                critcmp, "--color", "never", "--target-dir", str(report / "comparison-target"),
                str(Path(base).resolve()), str(Path(head).resolve()),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        (report / "comparison.txt").write_text(completed.stdout)
        (report / "comparator.stderr.log").write_text(completed.stderr)
        result["comparator_exit_status"] = completed.returncode
        if completed.returncode:
            raise ValueError(f"critcmp failed with exit status {completed.returncode}: {completed.stderr.strip()}")
        if not completed.stdout.strip():
            raise ValueError("critcmp produced an empty comparison")
        if any(case not in completed.stdout for case in matching):
            raise ValueError("critcmp output omitted matching JPEG cases")
        result["status"] = "passed"
        return completed.stdout
    except (OSError, ValueError) as error:
        result["error"] = str(error)
        raise
    finally:
        (report / "validation.json").write_text(json.dumps(result, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", type=Path)
    parser.add_argument("head", type=Path)
    parser.add_argument("report", type=Path)
    parser.add_argument("--critcmp", default="critcmp")
    args = parser.parse_args()
    try:
        print(compare(args.base, args.head, args.report, args.critcmp), end="")
    except (OSError, ValueError) as error:
        parser.exit(1, f"JPEG benchmark validation failed: {error}\n")


if __name__ == "__main__":
    main()
