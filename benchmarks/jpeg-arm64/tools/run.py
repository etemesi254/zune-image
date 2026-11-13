import argparse
import csv
import hashlib
import io
import json
import math
import os
import platform
import random
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
RUSTFLAGS = "-C target-cpu=generic -C force-frame-pointers=yes"


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def command(arguments, **kwargs):
    return subprocess.check_output([str(value) for value in arguments], text=True, **kwargs).strip()


def checked_log(arguments, path, **kwargs):
    with Path(path).open("w") as output:
        subprocess.run([str(value) for value in arguments], stdout=output,
                       stderr=subprocess.STDOUT, check=True, **kwargs)


def validate_ref(value):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,199}", value) or ".." in value:
        raise ValueError("baseline must be a commit SHA or ordinary branch/tag name")
    return value


def parse_expected(text):
    rows = list(csv.DictReader(io.StringIO(text)))
    result = {}
    for row in rows:
        if row["case"] in result or row["primary"] not in ("0", "1"):
            raise ValueError("duplicate case or invalid primary flag")
        record = {field: int(row[field]) for field in ("primary", "iterations", "rows")}
        if record["iterations"] <= 0 or record["rows"] <= 0:
            raise ValueError("empty JPEG case")
        result[row["case"]] = record
    if not result:
        raise ValueError("no JPEG cases")
    return result


def parse_measurements(text, expected, smoke=False):
    if not expected:
        raise ValueError("empty expected workload matrix")
    rows = list(csv.DictReader(io.StringIO(text)))
    result = {}
    for row in rows:
        key = row["case"]
        if key not in expected or key in result:
            raise ValueError(f"unexpected/duplicate case {key}")
        value = float(row["ns_per_image"])
        batch = int(row["batch_ns"])
        iterations = int(row["iterations"])
        if not math.isfinite(value) or value <= 0 or batch <= 0:
            raise ValueError(f"invalid elapsed time: {key}")
        if not smoke and batch < 100000:
            raise ValueError(f"batch below frozen duration floor: {key}")
        if iterations != (1 if smoke else expected[key]["iterations"]):
            raise ValueError(f"iteration count changed: {key}")
        if int(row["rows"]) != expected[key]["rows"] or int(row["calls"]) <= 0:
            raise ValueError(f"incomplete/empty JPEG output: {key}")
        if int(row["primary"]) != expected[key]["primary"]:
            raise ValueError(f"primary selection changed: {key}")
        if abs(value - batch / iterations) > 0.0011:
            raise ValueError(f"inconsistent batch timing: {key}")
        result[key] = value
    if set(result) != set(expected):
        raise ValueError("missing JPEG measurements")
    return result


def quantile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    lower = int(position)
    return values[lower] + (values[min(lower + 1, len(values) - 1)] - values[lower]) * (position - lower)


def pair_values(pairs, expected):
    primary = [key for key, value in expected.items() if value["primary"]]
    if not primary:
        raise ValueError("empty primary metric")
    values = {key: [1 - b[key] / a[key] for a, b in pairs] for key in expected}
    times = [(math.exp(statistics.mean(math.log(a[key]) for key in primary)),
              math.exp(statistics.mean(math.log(b[key]) for key in primary))) for a, b in pairs]
    values["PRIMARY"] = [1 - b / a for a, b in times]
    return values, times


def confidence(data, indices):
    medians = [statistics.median(data[index] for index in sample) for sample in indices]
    return [quantile(medians, 0.025), quantile(medians, 0.975)]


def source_info(path):
    return {
        "path": str(path),
        "commit": command(["git", "-C", path, "rev-parse", "HEAD"]),
        "decoder_trees": {name: command(["git", "-C", path, "rev-parse", f"HEAD:crates/{name}"])
                          for name in ("zune-jpeg", "zune-core")},
    }


def prepare(work, baseline, candidate, smoke):
    if work.exists():
        raise ValueError("refusing to overwrite an existing experiment")
    architecture = platform.machine().lower()
    if not smoke and architecture not in ("aarch64", "arm64"):
        raise ValueError(f"native ARM64 required, found {architecture}")
    work.mkdir(parents=True)
    results = work / "artifacts"
    results.mkdir()
    entries = list(csv.DictReader((HERE / "corpus.tsv").open(), delimiter="\t"))
    protocol = json.loads((HERE / "protocol.json").read_text())
    if len(entries) != protocol["images"] or len({row["name"] for row in entries}) != len(entries):
        raise ValueError("empty/duplicate/incomplete corpus")
    prepared = io.StringIO()
    writer = csv.DictWriter(prepared, fieldnames=list(entries[0]), delimiter="\t", lineterminator="\n")
    writer.writeheader()
    for entry in entries:
        path = (candidate / "test-images/jpeg" / entry["path"]).resolve()
        if not path.is_relative_to(candidate / "test-images/jpeg") or digest(path) != entry["sha256"]:
            raise ValueError(f"corpus content/path changed: {entry['name']}")
        entry["path"] = str(path)
        writer.writerow(entry)
    corpus = results / "corpus.tsv"
    corpus.write_text(prepared.getvalue())
    shutil.copy2(HERE / "protocol.json", results / "protocol.json")
    cpu = min(os.sched_getaffinity(0))
    environment = {**os.environ, "RUSTFLAGS": RUSTFLAGS, "CARGO_INCREMENTAL": "0"}
    sources = {"baseline": source_info(baseline), "candidate": source_info(candidate)}
    binaries = {}
    for label, source in sources.items():
        if not smoke and command(["git", "-C", source["path"], "status", "--porcelain"]):
            raise ValueError(f"{label} source is not clean")
        package = work / label
        package.mkdir()
        dependency = Path(source["path"]) / "crates/zune-jpeg"
        manifest = f"""[package]
name = "jpeg-arm64-benchmark"
version = "0.1.0"
edition = "2021"
[workspace]
[features]
allocation-count = []
[[bin]]
name = "jpeg-arm64-benchmark"
path = {json.dumps(str(HERE / "src/main.rs"))}
[dependencies]
zune-jpeg = {{ path = {json.dumps(str(dependency))}, features = ["arith"] }}
[profile.release]
opt-level = 3
debug = 2
codegen-units = 16
lto = false
"""
        (package / "Cargo.toml").write_text(manifest)
        target = work / f"target-{label}"
        checked_log(["cargo", "+1.89.0", "build", "--release", "--offline",
                     "--manifest-path", package / "Cargo.toml"], results / f"build-{label}.log",
                    env={**environment, "CARGO_TARGET_DIR": str(target)})
        binary = target / "release/jpeg-arm64-benchmark"
        header = binary.read_bytes()[:20]
        if not smoke and (header[:4] != b"\x7fELF" or int.from_bytes(header[18:20], "little") != 183):
            raise ValueError("benchmark executable is not native AArch64 ELF")
        binaries[label] = {"path": str(binary), "sha256": digest(binary)}
    expected = parse_expected(command([binaries["baseline"]["path"], "list", corpus]))
    if len(expected) != protocol["cases"] or sum(row["primary"] for row in expected.values()) != protocol["primary_cases"]:
        raise ValueError("prepared workload matrix differs from frozen protocol")
    save(results / "cases.json", expected)
    governors = list(Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_governor"))
    host = {
        "architecture": architecture, "native_arm64": architecture in ("aarch64", "arm64"),
        "cpu_affinity": cpu, "allowed_cpus": sorted(os.sched_getaffinity(0)),
        "lscpu": command(["lscpu"]), "kernel": platform.release(),
        "rustc": command(["rustc", "+1.89.0", "-Vv"]),
        "cargo": command(["cargo", "+1.89.0", "-V"]),
        "frequency_power_policy": {str(path): path.read_text().strip() for path in governors}
            if governors else {"status": "unavailable from runner; no settings changed"},
        "clock_resolution_seconds": time.clock_getres(time.CLOCK_MONOTONIC),
        "runner_arch": os.environ.get("RUNNER_ARCH"),
        "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        "github_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
    }
    save(results / "seal.json", {
        "smoke_only": smoke, "sources": sources, "binaries": binaries, "hardware": host,
        "unchanged_decoder_source": sources["baseline"]["decoder_trees"] == sources["candidate"]["decoder_trees"],
        "protocol_sha256": digest(HERE / "protocol.json"), "corpus_sha256": digest(corpus),
        "harness_sha256": {str(path.relative_to(HERE)): digest(path)
                           for path in [*(HERE / "src").glob("*.rs"), *(HERE / "tools").glob("*.py"), HERE / "corpus.tsv"]},
        "created_unix": time.time(),
    })
    print(f"Prepared {len(expected)} nonempty JPEG cases; native={host['native_arm64']}; smoke={smoke}")


def load_seal(work):
    results = work / "artifacts"
    seal = json.loads((results / "seal.json").read_text())
    for record in seal["binaries"].values():
        if digest(record["path"]) != record["sha256"]:
            raise ValueError("timed binary changed")
    if digest(results / "corpus.tsv") != seal["corpus_sha256"] or digest(HERE / "protocol.json") != seal["protocol_sha256"]:
        raise ValueError("frozen inputs/protocol changed")
    for name, expected in seal["harness_sha256"].items():
        if digest(HERE / name) != expected:
            raise ValueError("shared harness changed after build")
    return results, seal, json.loads((results / "cases.json").read_text())


def validate(work):
    results, seal, expected = load_seal(work)
    checked_log([seal["binaries"]["baseline"]["path"], "validate", results / "corpus.tsv"],
                results / "scalar-parity.log")
    processes = []
    errors = []
    payload_bytes = 0
    count = 0
    try:
        for label in ("baseline", "candidate"):
            error = (results / f"pixel-parity-{label}.stderr").open("w")
            errors.append(error)
            processes.append(subprocess.Popen([seal["binaries"][label]["path"], "emit",
                                              str(results / "corpus.tsv")], stdout=subprocess.PIPE, stderr=error))
        seen = set()
        aggregate = hashlib.sha256()
        while True:
            headers = [process.stdout.readline() for process in processes]
            if not headers[0] or headers[0] != headers[1]:
                raise ValueError(f"pixel stream metadata/EOF mismatch: {headers}")
            fields = headers[0].decode().rstrip().split("\t")
            if fields[0] == "END":
                if fields != ["END", str(len(expected)), "0"]:
                    raise ValueError("incomplete pixel stream")
                break
            key, rows, length = fields
            if key not in expected or key in seen or int(rows) != expected[key]["rows"] or int(length) <= 0:
                raise ValueError("empty/incomplete/duplicate pixel case")
            seen.add(key)
            left = int(length)
            aggregate.update(headers[0])
            while left:
                size = min(left, 65536)
                chunks = [process.stdout.read(size) for process in processes]
                if len(chunks[0]) != size or chunks[0] != chunks[1]:
                    raise ValueError(f"exact pixel mismatch: {key}")
                aggregate.update(chunks[0])
                left -= size
            payload_bytes += int(length)
            count += 1
        if seen != set(expected) or any(process.wait() != 0 for process in processes):
            raise ValueError("pixel producer failed/incomplete")
        save(results / "validation.json", {
            "cases": count, "exact_pixel_bytes_compared": payload_bytes,
            "aggregate_sha256": aggregate.hexdigest(), "scalar_and_pair_parity": True,
        })
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
                process.wait()
        for error in errors:
            error.close()
    print(f"Validated {count} cases and {payload_bytes} exact pixel bytes")


def allocations(work):
    results, seal, expected = load_seal(work)
    tables = {}
    for label in ("baseline", "candidate"):
        target = work / f"allocation-target-{label}"
        checked_log(["cargo", "+1.89.0", "build", "--release", "--offline", "--features", "allocation-count",
                     "--manifest-path", work / label / "Cargo.toml"], results / f"allocation-build-{label}.log",
                    env={**os.environ, "RUSTFLAGS": RUSTFLAGS, "CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(target)})
        text = command([target / "release/jpeg-arm64-benchmark", "allocations", results / "corpus.tsv"])
        (results / f"allocations-{label}.csv").write_text(text + "\n")
        rows = list(csv.DictReader(io.StringIO(text)))
        table = {row["case"]: row for row in rows}
        if set(table) != set(expected) or len(table) != len(rows) or any(int(row["remaining_live_bytes"]) for row in rows):
            raise ValueError("invalid/leaking/empty allocation results")
        tables[label] = table
    delta = {key: {field: int(tables["candidate"][key][field]) - int(tables["baseline"][key][field])
                   for field in ("allocations", "requested_bytes", "peak_live_bytes")} for key in expected}
    save(results / "allocation-deltas.json", delta)


def host_state():
    return {"time_unix": time.time(), "loadavg": Path("/proc/loadavg").read_text().strip()}


def phase(work, session, name, results, seal, expected):
    smoke = seal["smoke_only"]
    n = 2 if smoke else 30
    directory = results / f"session-{session}" / name
    directory.mkdir(parents=True, exist_ok=True)
    if list(directory.iterdir()):
        raise ValueError("refusing to rerun/overwrite a populated phase")
    order = ["AB"] * (n // 2) + ["BA"] * (n // 2)
    random.Random(530000 + session).shuffle(order)
    binary_a = seal["binaries"]["baseline"]["path"]
    binary_b = binary_a if name == "aa" else seal["binaries"]["candidate"]["path"]
    pairs = []
    save(directory / "metadata.json", {
        "session": session, "phase": name, "pairs": n, "orders": order,
        "binaries": {"A": digest(binary_a), "B": digest(binary_b)},
        "protocol_sha256": seal["protocol_sha256"], "started_unix": time.time(),
        "independence_unit": "process pair", "smoke_only": smoke,
    })
    for index, sequence in enumerate(order):
        values = {}
        for label in sequence:
            binary = binary_a if label == "A" else binary_b
            arguments = ["taskset", "-c", str(seal["hardware"]["cpu_affinity"]), binary, "measure",
                         str(results / "corpus.tsv"), str(5300000 + session * 1000 + index)]
            if smoke:
                arguments.append("smoke")
            before = host_state()
            process = subprocess.run(arguments, capture_output=True, text=True)
            path = directory / f"pair-{index:02d}-{label}.csv"
            path.write_text(process.stdout)
            path.with_suffix(".stderr").write_text(process.stderr)
            with (directory / "processes.jsonl").open("a") as out:
                out.write(json.dumps({"pair": index, "label": label, "command": arguments,
                                      "returncode": process.returncode, "before": before, "after": host_state()}) + "\n")
            if process.returncode:
                raise RuntimeError(f"benchmark process failed ({process.returncode}): {path.with_suffix('.stderr')}")
            values[label] = parse_measurements(process.stdout, expected, smoke)
        pairs.append((values["A"], values["B"]))
        print(f"session={session} phase={name} pair={index + 1}/{n}", flush=True)
    save(directory / "completed.json", {"processes": 2 * n, "cases_per_process": len(expected), "ended_unix": time.time()})
    if smoke:
        return {}
    values, times = pair_values(pairs, expected)
    if name == "aa":
        report = {key: {"N": quantile([abs(value) for value in data], 0.95), "pair_gains": data}
                  for key, data in values.items()}
        save(directory / "noise.json", report)
    else:
        noise = json.loads((results / f"session-{session}/aa/noise.json").read_text())
        generator = random.Random(535300 + session)
        indices = [[generator.randrange(n) for _ in range(n)] for _ in range(10000)]
        report = {}
        for key, data in values.items():
            low, high = confidence(data, indices)
            margin = noise[key]["N"]
            report[key] = {
                "median_gain": statistics.median(data), "gain_CI95": [low, high], "N": margin,
                "pair_gains": data, "passes": low > margin if key == "PRIMARY" else -low <= margin,
            }
            if key != "PRIMARY":
                report[key]["slowdown_CI95"] = [-high, -low]
        report["PRIMARY"]["control_process_T_ns"] = [a for a, _ in times]
        report["PRIMARY"]["candidate_process_T_ns"] = [b for _, b in times]
        save(directory / "confidence.json", report)
        with (directory / "protected.csv").open("w") as out:
            writer = csv.writer(out)
            writer.writerow(["case", "median_gain", "gain_CI_low", "gain_CI_high", "noise_N", "slowdown_CI_upper", "passes"])
            for key in expected:
                row = report[key]
                writer.writerow([key, row["median_gain"], *row["gain_CI95"], row["N"], row["slowdown_CI95"][1], row["passes"]])
    return report


def measure(work):
    results, seal, expected = load_seal(work)
    if not (results / "validation.json").is_file():
        raise ValueError("exact correctness validation is required before timing")
    if not seal["smoke_only"]:
        dispatch = json.loads((results / "dispatch.json").read_text())
        profiles = json.loads((results / "profiles.json").read_text())
        if not dispatch["verified_native_calls"] or len(profiles) != 4 or any(row["samples"] != 1000 for row in profiles):
            raise ValueError("complete actual native dispatch/profiling is required before timing")
        if not (results / "allocation-deltas.json").is_file() or not (results / "native-correctness-complete").is_file():
            raise ValueError("native correctness and separate allocation checks must finish before timing")
    sessions = 1 if seal["smoke_only"] else 2
    reports = []
    for session in range(1, sessions + 1):
        if session > 1:
            last = json.loads((results / f"session-{session - 1}/ab/completed.json").read_text())["ended_unix"]
            time.sleep(max(0, last + 300 - time.time()))
        load_seal(work)
        phase(work, session, "aa", results, seal, expected)
        reports.append(phase(work, session, "ab", results, seal, expected))
    if seal["smoke_only"]:
        summary = {
            "verdict": "INCONCLUSIVE", "reason": "local smoke is not ARM64/performance evidence",
            "valid_processes": 8, "cases_per_process": len(expected), "smoke_only": True,
            "native_arm64": seal["hardware"]["native_arm64"], "N": None, "gain": None, "CI95": None,
        }
        save(results / "summary.json", summary)
        text = (f"# Local smoke ({seal['hardware']['architecture']})\n\n"
                f"Validated eight nonempty processes with {len(expected)} cases each. "
                "No ARM64 performance result, noise calibration or gain confidence interval is claimed.\n")
        (results / "summary.md").write_text(text)
        print(text)
        return
    all_pass = all(all(row["passes"] for row in report.values()) for report in reports)
    failed = [[key for key, row in report.items() if key != "PRIMARY" and not row["passes"]] for report in reports]
    if seal["unchanged_decoder_source"]:
        verdict = "INCONCLUSIVE"
        reason = "unchanged decoder source; infrastructure validation, not an optimization"
    elif all_pass:
        verdict, reason = "KEEP", "both sessions satisfy primary and every protected bound"
    elif any(failed):
        verdict, reason = "DROP", "protected non-regression bound(s) failed"
    else:
        verdict, reason = "INCONCLUSIVE", "primary benefit did not clear both calibrated noise floors"
    summary = {"verdict": verdict, "reason": reason, "primary": [report["PRIMARY"] for report in reports],
               "failed_protected_cases": failed, "valid_processes": sessions * (8 if seal["smoke_only"] else 120),
               "cases_per_process": len(expected), "native_arm64": seal["hardware"]["native_arm64"],
               "smoke_only": seal["smoke_only"], "unchanged_decoder_source": seal["unchanged_decoder_source"]}
    save(results / "summary.json", summary)
    lines = ["# Native ARM64 JPEG benchmark", "", f"**{verdict}**: {reason}.",
             "", "A green workflow means valid collection, not a demonstrated optimization.",
             "", "| Session | Primary N | Median gain | Paired 95% gain CI | Failed protected |",
             "|---|---:|---:|---|---:|"]
    for index, report in enumerate(reports):
        row = report["PRIMARY"]
        lines.append(f"| {index + 1} | {row['N']:.6%} | {row['median_gain']:.6%} | "
                     f"[{row['gain_CI95'][0]:.6%}, {row['gain_CI95'][1]:.6%}] | {len(failed[index])} |")
    lines += ["", "Standalone Linux ARM64 only; no Mac M2/M4, Windows ARM64, Skia/Blink/browser or RSS claim.",
              "See uploaded raw process pairs, per-case protected bounds, native dispatch, profiles, and allocation artifacts."]
    text = "\n".join(lines) + "\n"
    (results / "summary.md").write_text(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as out:
            out.write(text)
    print(text)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("check-ref", "prepare", "validate", "profile", "allocations", "measure"))
    parser.add_argument("--work", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--local-smoke", action="store_true")
    parser.add_argument("--ref")
    args = parser.parse_args()
    if args.mode == "check-ref":
        validate_ref(args.ref)
        return
    work = args.work.resolve()
    if args.mode == "prepare":
        prepare(work, args.baseline.resolve(), args.candidate.resolve(), args.local_smoke)
    elif args.mode == "validate":
        validate(work)
    elif args.mode == "allocations":
        allocations(work)
    elif args.mode == "measure":
        measure(work)
    else:
        from profile_gdb import profile
        profile(work)


if __name__ == "__main__":
    main()
