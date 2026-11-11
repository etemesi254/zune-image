#!/usr/bin/env bash
set -Eeuo pipefail

if [[ $# != 5 ]]; then
    echo "usage: $0 BASE_CHECKOUT HEAD_CHECKOUT REPORT_DIR BASE_SHA HEAD_SHA" >&2
    exit 2
fi

base=$(realpath "$1")
head=$(realpath "$2")
mkdir -p "$3"
report=$(realpath "$3")
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
critcmp=$(command -v "${CRITCMP:-critcmp}")
base_sha=$4
head_sha=$5

finish() {
    status=$?
    trap - EXIT
    set +e
    for variant in base head; do
        if [[ -d "$report/targets/$variant/criterion" ]]; then
            mkdir -p "$report/raw/$variant"
            cp -a "$report/targets/$variant/criterion" "$report/raw/$variant/" || status=1
        fi
    done
    printf 'exit_status=%s\n' "$status" > "$report/status.txt"
    exit "$status"
}
trap finish EXIT

[[ "$base" != "$head" ]] || { echo "checkouts must be isolated" >&2; exit 1; }
for sha in "$base_sha" "$head_sha"; do
    [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || { echo "not an exact commit SHA: $sha" >&2; exit 1; }
done
for variant in base head; do
    checkout=$base
    expected=$base_sha
    [[ "$variant" == base ]] || { checkout=$head; expected=$head_sha; }
    actual=$(git -C "$checkout" rev-parse HEAD)
    [[ "$actual" == "$expected" ]] || {
        echo "$variant checkout is $actual, expected $expected" >&2
        exit 1
    }
    git -C "$checkout" diff --quiet HEAD -- || {
        echo "$variant checkout has modified tracked files" >&2
        exit 1
    }
done

export RUSTFLAGS="${JPEG_RUSTFLAGS:--C target-cpu=native}"
export CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never
# Ignore caller-provided cache wrappers and Criterion output overrides.
unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_ENCODED_RUSTFLAGS CRITERION_HOME
filter=${JPEG_BENCH_FILTER:-jpeg:}
flags=(
    "$filter" --sample-size "${JPEG_SAMPLE_SIZE:-20}"
    --warm-up-time "${JPEG_WARM_UP_TIME:-1}"
    --measurement-time "${JPEG_MEASUREMENT_TIME:-2}"
    --nresamples "${JPEG_NRESAMPLES:-10000}" --noplot
)
{
    printf 'base=%s\nhead=%s\npackage=zune-benches\ntarget=decode_jpeg\n' "$base_sha" "$head_sha"
    printf 'RUSTFLAGS=%s\nCARGO_INCREMENTAL=%s\n' "$RUSTFLAGS" "$CARGO_INCREMENTAL"
    printf 'criterion_flags='
    printf '%q ' "${flags[@]}"
    printf '\n'
    rustc -vV
    cargo -V
    "$critcmp" --version
} > "$report/measurement.txt" 2>&1

fixtures=(
    test-images/jpeg/benchmarks/speed_bench.jpg
    test-images/jpeg/benchmarks/speed_bench_horizontal_subsampling.jpg
    test-images/jpeg/benchmarks/speed_bench_vertical_subsampling.jpg
    test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg
    test-images/jpeg/benchmarks/speed_bench_prog.jpg
    test-images/jpeg/benchmarks/speed_bench_prog_h_sampling.jpg
    test-images/jpeg/benchmarks/speed_bench_prog_v_sampling.jpg
    test-images/jpeg/benchmarks/speed_bench_prog_hv_sampling.jpg
    test-images/jpeg/four_components.jpg
)

for variant in base head; do
    checkout=$base
    [[ "$variant" == base ]] || checkout=$head
    mkdir -p "$report/$variant"
    (
        cd "$checkout"
        for fixture in "${fixtures[@]}"; do
            [[ -s "$fixture" ]] || { echo "missing JPEG fixture: $fixture" >&2; exit 1; }
        done
        sha256sum "${fixtures[@]}"
    ) > "$report/$variant/fixtures.sha256" 2> "$report/$variant/fixtures.log"
    cp "$checkout/Cargo.lock" "$report/$variant/Cargo.lock"
    # A cached build is safe only within one variant; old measurements never are.
    [[ ! -e "$report/targets/$variant/criterion" ]] || {
        echo "refusing stale Criterion results for $variant" >&2
        exit 1
    }
done
diff -u "$report/base/fixtures.sha256" "$report/head/fixtures.sha256" \
    > "$report/fixture-diff.txt"

for variant in base head; do
    checkout=$base
    [[ "$variant" == base ]] || checkout=$head
    target="$report/targets/$variant"
    printf 'Measuring %s at %s with target directory %s\n' "$variant" "$checkout" "$target"
    (
        cd "$checkout"
        CARGO_TARGET_DIR="$target" cargo bench --locked -p zune-benches --bench decode_jpeg \
            -- --save-baseline "$variant" "${flags[@]}"
    ) 2>&1 | tee "$report/$variant/benchmark.log"
    "$critcmp" --target-dir "$target" --export "$variant" \
        > "$report/$variant.json" 2> "$report/$variant/export.log"
done

python3 "$script_dir/validate_criterion.py" "$report/base.json" "$report/head.json" \
    "$report" --critcmp "$critcmp" 2>&1 | tee "$report/validation.log"
