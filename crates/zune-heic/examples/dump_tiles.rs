//! Write every coded image (tile) of a HEIF file as a raw HEVC bitstream
//! plus the planes zune-heic decoded from it, so the HEVC decoder can be
//! checked against a reference decoder:
//!
//! ```text
//! cargo run --release --features dump-tiles --example dump_tiles -- image.heic out/
//! libde265-dec265 -q -o out/1.ref.yuv out/1.hevc
//! cmp out/1.ref.yuv out/1.yuv
//! ```
//!
//! Writes `<item id>.hevc` and `<item id>.yuv` into the output directory.
use zune_core::bytestream::ZCursor;
use zune_heic::HeifDecoder;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, heic_path, out_dir] = args.as_slice() else {
        eprintln!("usage: dump_tiles <image.heic> <output directory>");
        std::process::exit(2);
    };
    let data = std::fs::read(heic_path).expect("cannot read the HEIC file");
    std::fs::create_dir_all(out_dir).expect("cannot create the output directory");

    let mut decoder = HeifDecoder::new(ZCursor::new(data.as_slice()));
    let tiles = decoder.decode_tiles_yuv().expect("zune-heic failed to decode");
    for tile in &tiles {
        let path = |ext: &str| std::path::Path::new(out_dir).join(format!("{}.{ext}", tile.item_id));
        std::fs::write(path("hevc"), &tile.hevc).expect("cannot write the bitstream");
        std::fs::write(path("yuv"), &tile.yuv).expect("cannot write the planes");
    }
    println!("{heic_path}: {} coded image(s)", tiles.len());
}
