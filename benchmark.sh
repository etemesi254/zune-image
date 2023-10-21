#
# Copyright (c) 2023.
#
# This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
#

#/bin/bash
echo "compiling zune-image"
cargo build --release --quiet
if ! command -v vips &> /dev/null
then
    echo "VIPS command could not be found, ensure you have installed vips in the command line"
    exit
else
  echo "VIPS present"
fi

if ! command -v convert &> /dev/null
then
    echo "Imagemagick convert command could not be found, ensure you have installed imagemagick in the command line"
    exit
else
  echo "Imagemagick present"

fi
if !command -v hyperfine &> /dev/null
then
  echo "Hyperfine command not found see https://github.com/sharkdp/hyperfine for installation method"
else
  echo "hyperfine command present"
fi

function jpeg_hdr() {
    echo "Baseline jpeg to hdr"
    IN_FILE="./test-images/jpeg/benchmarks/speed_bench.jpg"
    output="$(mktemp --tmpdir result_XXXXXXXXXXXXX.hdr)"
    echo ${IN_FILE};

    hyperfine  --warmup=5 "vips copy $IN_FILE  $output" "./target/release/zune -i  $IN_FILE -o $output" "convert $IN_FILE $output"

}
function png_jpeg() {
  echo "Baseline png to jpeg"
  IN_FILE="./test-images/png/benchmarks/speed_bench.png"
  output="$(mktemp --tmpdir result_XXXXXXXXXXXXX.jpg)"
  echo ${IN_FILE};
  hyperfine --warmup=5 "vips copy $IN_FILE  $output" "./target/release/zune -i  $IN_FILE -o $output" "convert $IN_FILE $output"
}

function jpeg_jxl_lossless(){
  echo "Baseline JPEG to JXL lossless"
  # vips takes too long on this
  #IN_FILE="./test-images/jpeg/benchmarks/speed_bench.jpg"
  IN_FILE="./test-images/png/benchmarks/speed_bench.png"

  output="$(mktemp --tmpdir result_XXXXXXXXXXXXX.jxl)"
  echo ${IN_FILE};

  hyperfine  --warmup=5 "vips jxlsave $IN_FILE  $output --lossless=true" "./target/release/zune -i  $IN_FILE -o $output" "convert $IN_FILE $output"


}

jpeg_hdr
png_jpeg
jpeg_jxl_lossless
