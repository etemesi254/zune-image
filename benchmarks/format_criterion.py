#!/usr/bin/env python3
"""Turn Criterion's JSON output into compact Markdown benchmark tables."""

from __future__ import annotations

import argparse
import json
import platform
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Result:
    group: str
    function: str
    nanoseconds: float
    throughput: str | None


def human_time(nanoseconds: float) -> str:
    units = ((1e9, "s"), (1e6, "ms"), (1e3, "µs"), (1, "ns"))
    for scale, suffix in units:
        if nanoseconds >= scale or suffix == "ns":
            value = nanoseconds / scale
            decimals = 2 if value < 100 else 1
            return f"{value:.{decimals}f} {suffix}"
    raise AssertionError("unreachable")


def human_rate(amount: float, kind: str) -> str:
    if kind == "Bytes":
        units = (("GiB/s", 1024**3), ("MiB/s", 1024**2), ("KiB/s", 1024), ("B/s", 1))
    else:
        units = (("Gelem/s", 1e9), ("Melem/s", 1e6), ("Kelem/s", 1e3), ("elem/s", 1))
    for suffix, scale in units:
        if amount >= scale or scale == 1:
            value = amount / scale
            decimals = 2 if value < 100 else 1
            return f"{value:.{decimals}f} {suffix}"
    raise AssertionError("unreachable")


def slowdown(value: float, winner: float) -> str:
    ratio = value / winner
    decimals = 3 if ratio < 1.1 else 2
    return f"{ratio:.{decimals}f}× slower"


def read_results(root: Path) -> list[Result]:
    results: list[Result] = []
    for benchmark_file in sorted(root.glob("**/new/benchmark.json")):
        estimates_file = benchmark_file.with_name("estimates.json")
        if not estimates_file.is_file():
            continue
        try:
            benchmark = json.loads(benchmark_file.read_text())
            estimates = json.loads(estimates_file.read_text())
            nanoseconds = float(estimates["mean"]["point_estimate"])
        except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
            raise ValueError(f"cannot read {benchmark_file}: {error}") from error

        throughput = None
        throughput_data = benchmark.get("throughput")
        if throughput_data:
            kind, amount = next(iter(throughput_data.items()))
            per_second = float(amount) * 1e9 / nanoseconds
            throughput = human_rate(per_second, kind)
        results.append(Result(
            group=str(benchmark["group_id"]),
            function=str(benchmark.get("function_id") or benchmark.get("value_str") or "benchmark"),
            nanoseconds=nanoseconds,
            throughput=throughput,
        ))
    return results


def escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def display_group(group: str) -> tuple[str, str]:
    if ":" in group:
        section, name = group.split(":", 1)
        return section.strip().replace("_", " ").title(), name.strip()
    return "Benchmarks", group


def render(runs: list[tuple[str, list[Result]]]) -> str:
    labels = [label for label, _ in runs]
    indexed: dict[tuple[str, str], dict[str, Result]] = defaultdict(dict)
    for label, results in runs:
        for result in results:
            indexed[(result.group, result.function)][label] = result

    sections: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for group, function in indexed:
        section, _ = display_group(group)
        sections[section].append((group, function))

    winners: dict[tuple[str, str], float] = {}
    for group, _function in indexed:
        for label in labels:
            times = [
                results[label].nanoseconds
                for (candidate_group, _), results in indexed.items()
                if candidate_group == group and label in results
            ]
            if times:
                winners[(group, label)] = min(times)

    lines = ["# Criterion benchmark results", ""]
    for section in sorted(sections, key=str.casefold):
        lines.extend((f"## {escape(section)}", ""))
        headers = ["Benchmark", "Library"]
        for label in labels:
            headers.extend((f"{label} Time", f"{label} Throughput", f"{label} Comparison"))
        lines.append("| " + " | ".join(headers) + " |")
        lines.append("|:" + "|:".join("-" * max(3, len(h) - 1) for h in headers) + "|")

        keys = sorted(sections[section], key=lambda item: (item[0].casefold(), item[1].casefold()))
        previous_group = None
        for group, function in keys:
            _, benchmark_name = display_group(group)
            benchmark_cell = f"**{escape(benchmark_name)}**" if group != previous_group else ""
            cells = [benchmark_cell, escape(function)]
            for label in labels:
                result = indexed[(group, function)].get(label)
                if result:
                    winner_time = winners[(group, label)]
                    comparison = (
                        "**Winner**"
                        if result.nanoseconds == winner_time
                        else slowdown(result.nanoseconds, winner_time)
                    )
                    cells.extend((human_time(result.nanoseconds), result.throughput or "—", comparison))
                else:
                    cells.extend(("—", "—", "—"))
            lines.append("| " + " | ".join(cells) + " |")
            previous_group = group
        lines.append("")
    return "\n".join(lines)


def parse_run(value: str) -> tuple[str, Path]:
    if "=" in value:
        label, path = value.split("=", 1)
        if not label:
            raise argparse.ArgumentTypeError("run label cannot be empty")
        return label, Path(path)
    return platform.system() or "Current", Path(value)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Format Criterion JSON results as README-style Markdown tables.",
        epilog="Compare runs with: --run macOS=results/mac --run Linux=results/linux",
    )
    parser.add_argument("--run", action="append", type=parse_run, metavar="[LABEL=]DIR",
                        help="Criterion directory (repeat for cross-platform columns)")
    parser.add_argument("--output", "-o", type=Path, help="write Markdown to this file")
    args = parser.parse_args(argv)

    run_specs = args.run or [(platform.system() or "Current", Path("target/criterion"))]
    runs: list[tuple[str, list[Result]]] = []
    for label, path in run_specs:
        if not path.is_dir():
            parser.error(f"Criterion directory does not exist: {path}")
        results = read_results(path)
        if not results:
            parser.error(f"no **/new/benchmark.json results found in: {path}")
        runs.append((label, results))

    markdown = render(runs) + "\n"
    if args.output:
        args.output.write_text(markdown)
    else:
        sys.stdout.write(markdown)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
