## zune-image internals

When possible, use generics as much as possible.

Data should be passed as a slice reference
i.e `&[T]` or `&mut [T]`.

This library should not do any allocations at all

### Benchmarking

Any important function that is deemed important should
probably be benchmarked,

This needs a nightly compiler to run the benchmarks,
the command to run benchmarks is

```shell
cargo bench --features=benchmarks
```