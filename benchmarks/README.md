# Zune benchmarks

### Benchmark primitives.

This crate exposes the ability to compare `zune-` family of crates to other crates that
exists which do the same functionality

It does not contain any library specific code, most of the code exists
in the `benches` directory

Benchmarking is done via `criterion` 

## Formatting Criterion results

Criterion's JSON results can be converted into grouped Markdown tables like the
ones below. After running the benchmarks, use:

```shell
python3 benchmarks/format_criterion.py > benchmark-results.md
```

The default input is `target/criterion`. To compare saved runs from two systems:

```shell
python3 benchmarks/format_criterion.py \
  --run macOS=results/macos/criterion \
  --run Linux=results/linux/criterion \
  --output benchmark-results.md
```

The formatter uses Criterion's mean estimate, automatically selects readable
time units, calculates throughput from Criterion's byte or element metadata,
marks the fastest library in each benchmark as the winner, reports how many
times slower each alternative is, and uses `—` where a run has no matching
result.

## Running on MacOs

Tested on mac-os solana


1. Install brew
2. Install libvips
```shell
brew install libvips,nasm
```
1. Create a symlink to allow the system linker to find libraries

```shell
sudo ln -s /opt/homebrew/lib /usr/local/lib
```

1. Run cargo bench

```shell
cargo bench --workspace
```

If you fail to symlink, the libraries

## On linux
This is a short one on ubuntu 

1. Install clang, libvips, libvips-dev , nasm (for mozjpeg)

```shell
sudo apt install clang libvips libvips-dev nasm
```

1. Run the benchmark
```shell
cargo bench --workspace
```

Benchmarks

## On linux
System Specs
```text
Architecture:             x86_64
  CPU op-mode(s):         32-bit, 64-bit
  Address sizes:          40 bits physical, 48 bits virtual
  Byte Order:             Little Endian
CPU(s):                   4
  On-line CPU(s) list:    0-3
Vendor ID:                AuthenticAMD
  Model name:             AMD EPYC-Rome Processor
    CPU family:           23
    Model:                49
    Thread(s) per core:   1
    Core(s) per socket:   4
    Socket(s):            1
    Stepping:             0
    BogoMIPS:             4890.80
    Flags:                fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht syscall nx mmxext fxsr_opt pdpe1gb rdtscp lm rep_good nopl cpuid extd_apicid tsc_known
                          _freq pni pclmulqdq ssse3 fma cx16 sse4_1 sse4_2 x2apic movbe popcnt aes xsave avx f16c rdrand hypervisor lahf_lm cmp_legacy cr8_legacy abm sse4a misalignsse 3dnowprefetch osvw topoext pe
                          rfctr_core ssbd ibrs ibpb stibp vmmcall fsgsbase bmi1 avx2 smep bmi2 rdseed adx smap clflushopt clwb sha_ni xsaveopt xsavec xgetbv1 clzero xsaveerptr wbnoinvd arat umip rdpid
Virtualization features:  
  Hypervisor vendor:      KVM
  Virtualization type:    full
Caches (sum of all):      
  L1d:                    128 KiB (4 instances)
  L1i:                    128 KiB (4 instances)
  L2:                     2 MiB (4 instances)
  L3:                     16 MiB (1 instance)

```

Benchmark results found in [Benchmark Results](./benchmark-results.md)