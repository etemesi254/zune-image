## Fuzzing corpus

This files contains fuzzing files use to check decoders for resilience
against invalid input/corrupted input.

## Running in CI

CI runs are found in `.github/workflows`

They are ran in the Github CI during pull requests and periodically
on schedule.

## Running locally

There is a `./fuzz.sh` file in the directory
running it in the current directory will compile
and run decoders with the fuzzers

## Thanks to

[Shnatsel](https://github.com/Shnatsel) for providing the corpus
and time to fuzz test libraries