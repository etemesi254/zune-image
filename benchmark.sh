#
# Copyright (c) 2023.
#
# This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
#

#/bin/bash
echo "compiling"
cargo build --release --quiet
if ! command -v vips &> /dev/null
then
    echo "VIPS command could not be found, ensure you have installed vips in the command line"
    exit
fi

if ! command -v convert &> /dev/null
then
    echo "Imagemagick convert command could not be found, ensure you have installed imagemagick in the command line"
    exit
else
  echo "Imagemagick version"
  echo "$(convert -version)"
  echo "\n\n"
fi

function jpeg_hdr() {
    echo "Baseline jpeg to hdr"
    IN_FILE="./test-images/jpeg/benchmarks/speed_bench.jpg"
    output="$(mktemp --tmpdir result_XXXXXXXXXXXXX.hdr)"
    echo ${IN_FILE};

    hyperfine --export-markdown ./jpeg_hdr_bench.md  --warmup=5 "vips copy $IN_FILE  $output" "./target/release/zune -i  $IN_FILE -o $output" "convert $IN_FILE $output"

}
function png_jpeg() {
  echo "Baseline png to jpeg"
  IN_FILE="./test-images/png/benchmarks/speed_bench.png"
  output="$(mktemp --tmpdir result_XXXXXXXXXXXXX.jpg)"
  echo ${IN_FILE};
  hyperfine --export-markdown ./png_jpeg_bench.md  --warmup=5 "vips copy $IN_FILE  $output" "./target/release/zune -i  $IN_FILE -o $output" "convert $IN_FILE $output"
}

jpeg_hdr
png_jpeg