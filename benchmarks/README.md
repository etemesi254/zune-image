# Zune benchmarks

### Benchmark primitives.

This crate exposes the ability to compare `zune-` family of crates to other crates that
exists which do the same functionality

It does not contain any library specific code, most of the code exists
in the `benches` directory

Benchmarking is done via `criterion` 

## Running on MacOs

Tested on mac-os solana


1. Install brew
2. Install libvips
```shell
brew install libvips
```
3. Create a symlink to allow the system linker to find libraries

```shell
sudo ln -s /opt/homebrew/lib /usr/local/lib
```

4. Run cargo bench

```shell
cargo bench --workspace
```

If you fail to symlink, the libraries