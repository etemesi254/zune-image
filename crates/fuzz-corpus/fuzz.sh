# Move one directory up.
cd ..

echo "Running the PNG FUZZER"
cargo +nightly fuzz  run  --fuzz-dir zune-png/fuzz decode_buffer  fuzz-corpus/png -- -runs=100000


echo "Running the JPEG FUZZER"
cargo +nightly fuzz run --fuzz-dir zune-jpeg/fuzz decode_buffer -- -runs=10000 zune-jpeg/test-images/fuzz_references