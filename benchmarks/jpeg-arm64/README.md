# Native ARM64 JPEG benchmark CI

This opt-in, artifact-only workflow benchmarks **standalone zune-jpeg on native
Linux ARM64**. It does not post PR comments, change SIMD defaults, or establish
Skia/Blink/browser, Mac M2/M4, Windows ARM64, or RSS results.

Add the `arm64-benchmark` label to a PR to run it, including on drafts. The
baseline is the immutable PR base SHA, not a later-moving `dev`. Once installed
on the repository's default branch, `workflow_dispatch` also accepts a pinned
control SHA or ordinary branch/tag name. Use a control containing prerequisite
optimizations: do not credit unrelated base/head differences to a new experiment.

Before the new workflow is on the default branch, the existing **Benchmark
Regression** dispatcher can invoke it from the draft ref without merging or
changing repository settings:

```sh
gh workflow run criterion_benchmarks.yml --ref CI_DRAFT_BRANCH \
  -f native_arm64=true -f baseline_ref=PINNED_CONTROL_SHA
```

That explicit native-only mode skips the Criterion job and its PR comparison.
Ordinary Criterion invocations retain their current behavior.

## What the workflow verifies

* `ubuntu-24.04-arm`, ARM64 host identity, native AArch64 ELF binaries and pinned
  Rust 1.89.0; no emulation or cross-compilation claimed as runtime validation.
* The same harness, flags, corpus, allocation boundaries and CPU affinity for
  control/candidate. No competing benchmark, build or profile jobs inside the
  runner during timings; no frequency/power/sysctl tuning.
* Six capped named-entry probes per build: RGB/RGBA/BGRA with normal dispatch
  and forced scalar reference. These are actual implementation calls, not
  function-pointer address comparisons. Counts are capped at 32 per function
  and must not be interpreted as total frequencies or timing gains.
* Own-child GDB profiles: 1,000 actual stack samples for each whole-image/36-row
  API and auto/scalar mode. Costs are sampled active-stack fractions, not
  hardware cycles or the primary case weights. Empty/failed profiles block timing.
* Exact control/candidate pixels plus scalar parity, nonempty JPEGs, all rows
  and final completion. Existing native debug/release/scalar/no-std checks must
  also pass. A native baseline bug is a real failure, not skipped to obtain timings.
* Separate counting-allocator results; instrumentation is never in timed builds.

The standalone package avoids the existing multi-codec Criterion suite's native
mozjpeg/VIPS/HEIC dependencies. Only repository zune-jpeg and standard-library
dependencies are used. Existing image assets are selected by geometry/encoding,
not timing; no new image binaries are added.

## Frozen protocol

[protocol.json](./protocol.json) and [corpus.tsv](./corpus.tsv) define the inputs
before any kernel changes or first comparison. Do not switch metrics, modify
corpus weights, tune the candidate after results, or retry until significant.
Another kernel experiment requires a fresh, predeclared protocol.

Primary: complete-image **36-row source-output scanline decoding**, including
construction/headers, allocations, every row/retry, finish and destruction.
Source is RGB for YCbCr/native RGB, Luma for grayscale, CMYK for CMYK. Its 32
cases have equal image and resident/growing-input weights. This corresponds to
the crate API used by Skia M5's ARM source-output route, **not** timing of C++
swizzling or an unselected direct RGBA/BGRA application path.

Every one of the 416 individual cases is protected: whole-image RGB/RGBA/BGRA,
source auto/scalar whole and scanline APIs, one-row callers, raw/YCbCr,
grayscale/native RGB/CMYK, baseline/progressive/restart/multi-scan and
small/large/odd geometries covering all four common chroma sampling modes.
Reduced output is not applicable to controls lacking a scaling API; this
workflow does not add one. Growing input reveals preloaded bytes on suspension,
not network I/O or compressed-buffer allocation time.

Each session runs 30 independent identical-control A/A process pairs and 30
counterbalanced A/B pairs. Two sessions run serially on the same runner, separated
by at least 300 seconds and with fresh A/A calibration. Inner iterations are
fixed per image and never treated as independent samples.

Process T = geometric mean primary ns/image; paired gain = `1 - T_head/T_control`;
summary = median of process-pair gains. N = linear-interpolated 95th percentile
of absolute labeled A/A differences. Confidence intervals resample process pairs
10,000 times, preserving case correlations. Each protected case has its own N.

KEEP requires primary lower 95% gain CI > N and every protected upper 95%
slowdown CI <= its own N **in both sessions**, plus correctness/dispatch.
Forced-kernel-only wins do not pass. Hardware identity, power-policy availability,
compiler versions, hashes, run/attempt identifiers and raw samples are artifacts.

**Green CI means valid collection, not a performance win.** The step summary and
`summary.json` report KEEP/DROP/INCONCLUSIVE independently of command success.
Unchanged decoder source (such as a CI-only draft) is never reported KEEP.
Broad measured noise is not a tight zero-regression guarantee.

## Reproduction

Use two clean source checkouts and the candidate's committed harness:

```sh
tools=candidate/benchmarks/jpeg-arm64/tools
python3 "$tools/run.py" prepare --work /tmp/jpeg-arm64 \
  --baseline "$(pwd)/baseline" --candidate "$(pwd)/candidate"
python3 "$tools/run.py" validate --work /tmp/jpeg-arm64
# Run the native correctness commands from the workflow, recording success.
python3 "$tools/run.py" profile --work /tmp/jpeg-arm64
python3 "$tools/run.py" allocations --work /tmp/jpeg-arm64
python3 "$tools/run.py" measure --work /tmp/jpeg-arm64
```

Local non-ARM development may explicitly use `prepare --local-smoke`; it runs
two pairs/one session/one iteration and is always **non-performance evidence**.
It must never be substituted for the native CI job.

Populated timing phases refuse overwrite/rerun. Tool, decoder, malformed/empty
case, missing-output and too-short-batch failures propagate. Artifacts upload even
on failure; no successful build or empty benchmark is treated as speed evidence.

Only retain/publish a kernel optimization after its gate passes and approval is
given, including manual safety/wide-coefficient-fallback and bounded-memory
review. CI's scoped performance verdict is not publication approval.
This infrastructure PR itself makes no optimization claim.
