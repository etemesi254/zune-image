import ast
import json
import os
import random
import re
import select
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

from run import HERE, checked_log, load_seal, save


def region(name):
    for architecture in ("neon", "scalar"):
        if name.startswith(f"zune_jpeg::color_convert::{architecture}"):
            return f"color_{architecture}"
        if name.startswith(f"zune_jpeg::upsampler::{architecture}::"):
            return f"upsampling_{architecture}"
        if name.startswith(f"zune_jpeg::idct::{architecture}::"):
            return f"idct_{architecture}"
    if name.startswith("zune_jpeg::bitstream::") or name.startswith("<zune_jpeg::bitstream::"):
        return "entropy"
    return "other_setup_or_unclassified"


def dispatch(results, seal):
    proof = {}
    for label in ("baseline", "candidate"):
        binary = seal["binaries"][label]["path"]
        proof[label] = {}
        for color in ("RGB", "RGBA", "BGRA"):
            for policy in ("auto", "scalar"):
                name = f"dispatch-{label}-{color}-{policy}"
                output = results / f"{name}.json"
                environment = {**os.environ, "JPEG_PROBE_BINARY": binary, "JPEG_PROBE_OUTPUT": str(output)}
                arguments = ["taskset", "-c", str(seal["hardware"]["cpu_affinity"]), "gdb", "-nx", "-q", "-batch",
                             "-ex", "file " + json.dumps(binary),
                             "-ex", "set args dispatch " + json.dumps(str(results / "corpus.tsv")) + f" {color} {policy}",
                             "-x", str(HERE / "tools/dispatch_gdb.py")]
                checked_log(arguments, results / f"{name}.log", env=environment)
                record = json.loads(output.read_text())
                counts = Counter()
                for function in record["functions"].values():
                    counts[function["group"]] += function["capped_entry_hits"]
                if policy == "auto":
                    required = ("neon_color", "neon_idct", "neon_horizontal", "neon_vertical", "neon_hv")
                else:
                    required = ("scalar_color", "scalar_idct", "scalar_horizontal", "scalar_vertical", "scalar_hv")
                    if any(value for key, value in counts.items() if key.startswith("neon_")):
                        raise ValueError(f"forced scalar actually called NEON: {name}")
                if any(counts[key] <= 0 for key in required):
                    raise ValueError(f"unverified implementation dispatch: {name}: {dict(counts)}")
                proof[label][f"{color}:{policy}"] = dict(counts)
    save(results / "dispatch.json", {"verified_native_calls": True, "capped_counts": proof,
                                     "note": "Capped entry counts establish execution, not performance or total call frequencies"})


def sample(results, seal, api, policy, seed):
    binary = seal["binaries"]["baseline"]["path"]
    name = f"profile-{api}-{policy}"
    count = 1000
    process = subprocess.Popen(
        ["taskset", "-c", str(seal["hardware"]["cpu_affinity"]), "gdb", "-q", "-nx", "--interpreter=mi2"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=0)
    pending = bytearray()
    token = 0
    stopped = None
    running = False
    with (results / f"{name}.mi.log").open("w") as log, (results / f"{name}.jsonl").open("w") as samples:
        def line():
            nonlocal pending, stopped, running
            deadline = time.monotonic() + 30
            while b"\n" not in pending:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not select.select([process.stdout], [], [], remaining)[0]:
                    raise TimeoutError("GDB MI response timeout")
                data = process.stdout.read(4096)
                if not data:
                    raise RuntimeError(f"GDB exited unexpectedly: {process.poll()}")
                pending.extend(data)
            out, _, pending = pending.partition(b"\n")
            pending = bytearray(pending)
            text = out.decode(errors="replace").strip()
            log.write(text + "\n")
            if "*stopped" in text:
                stopped = text
                running = False
            return text

        def command(text, expected="done"):
            nonlocal token, running
            token += 1
            marker = str(token) + "^"
            process.stdin.write((str(token) + text + "\n").encode())
            process.stdin.flush()
            while True:
                response = line()
                position = response.find(marker)
                if position >= 0:
                    response = response[position:]
                    if not response.startswith(marker + expected):
                        raise RuntimeError(response)
                    if expected == "running":
                        running = True
                    return response
                if '*stopped,reason="exited' in response:
                    raise RuntimeError("profiling workload exited before collection completed")

        def wait_stopped():
            nonlocal stopped
            while stopped is None:
                line()
            response, stopped = stopped, None
            if "exited" in response:
                raise RuntimeError(response)
            return response

        histogram = Counter()
        completed_jpegs = False
        try:
            command("-gdb-set pagination off")
            command("-gdb-set confirm off")
            command("-gdb-set disable-randomization off")
            command("-gdb-set debuginfod enabled off")
            command("-gdb-set mi-async on")
            command("-file-exec-and-symbols " + json.dumps(binary))
            command("-exec-arguments profile " + json.dumps(str(results / "corpus.tsv")) + f" {api} {policy}")
            command("-exec-run", "running")
            while "PROFILE_READY" not in line():
                pass
            generator = random.Random(seed)
            for index in range(count):
                time.sleep(generator.uniform(0.002, 0.006))
                command("-exec-interrupt --all")
                wait_stopped()
                response = command("-stack-list-frames 0 40")
                frames = []
                for block in re.findall(r'frame=\{((?:"(?:[^"\\]|\\.)*"|[^}])*)\}', response):
                    fields = {key: ast.literal_eval('"' + value + '"')
                              for key, value in re.findall(r'(\w+)="((?:[^"\\]|\\.)*)"', block)}
                    frames.append(fields)
                if not frames:
                    raise ValueError("empty native workload stack")
                classifications = [region(frame.get("func", "")) for frame in frames]
                category = next((kind for kind in classifications if kind != "other_setup_or_unclassified"),
                                "other_setup_or_unclassified")
                histogram[category] += 1
                samples.write(json.dumps({"sample": index, "api": api, "policy": policy,
                                          "region": category, "frames": frames}) + "\n")
                command("-exec-continue", "running")
            if not completed_jpegs:
                # PROFILE_COMPLETED may arrive while waiting for a token rather than stop.
                log.flush()
                completed_jpegs = "PROFILE_COMPLETED" in (results / f"{name}.mi.log").read_text()
            if not completed_jpegs or sum(histogram.values()) != count:
                raise ValueError("profile did not execute nonempty complete JPEG cases")
        finally:
            if process.poll() is None:
                try:
                    if running:
                        command("-exec-interrupt --all")
                        wait_stopped()
                    command("-interpreter-exec console \"kill\"")
                    command("-gdb-exit", "exit")
                except (RuntimeError, TimeoutError, BrokenPipeError) as error:
                    print(f"Explicit profiler cleanup failure: {error}", file=sys.stderr)
                    process.terminate()
                process.wait(timeout=15)
    return {"samples": count, "api": api, "policy": policy, "histogram": dict(histogram),
            "method": "own-child GDB actual-stack sampling; not hardware-cycle attribution"}


def profile(work):
    results, seal, _ = load_seal(work)
    if seal["smoke_only"] or not seal["hardware"]["native_arm64"]:
        raise ValueError("native ARM64 required for dispatch/profile evidence")
    dispatch(results, seal)
    profiles = [sample(results, seal, api, policy, 5353000 + index)
                for index, (api, policy) in enumerate((("Into", "auto"), ("Scan36", "auto"),
                                                      ("Into", "scalar"), ("Scan36", "scalar")))]
    save(results / "profiles.json", profiles)
    for record in profiles:
        print(record["api"], record["policy"], record["histogram"], flush=True)
