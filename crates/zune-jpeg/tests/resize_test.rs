use std::io::Cursor;
use zune_jpeg::JpegDecoder;

#[test]
fn test_resizing_downscale_baseline() {
    let image: &[u8] = include_bytes!("images/iptc.jpeg");

    for &factor in &[1, 2, 4, 8] {
        let mut decoder = JpegDecoder::new(Cursor::new(image));
        decoder.decode_headers().unwrap();
        
        let orig_info = decoder.info().unwrap();
        let orig_width = orig_info.width as usize;
        let orig_height = orig_info.height as usize;
        
        decoder.set_downscale_factor(factor);
        let out_width = decoder.output_width();
        let out_height = decoder.output_height();
        
        assert_eq!(out_width, orig_width / factor);
        assert_eq!(out_height, orig_height / factor);
        
        let out_colorspace = decoder.options().jpeg_get_out_colorspace();
        let channels = out_colorspace.num_components();
        
        let pixels = decoder.decode().unwrap();
        assert_eq!(pixels.len(), out_width * out_height * channels, "Pixel length mismatch at factor {}", factor);
    }
}

#[test]
fn test_resizing_downscale_progressive() {
    let image: &[u8] = include_bytes!("../../../test-images/jpeg/benchmarks/speed_bench_prog.jpg");

    for &factor in &[1, 2, 4, 8] {
        let mut decoder = JpegDecoder::new(Cursor::new(image));
        decoder.decode_headers().unwrap();
        
        let orig_info = decoder.info().unwrap();
        let orig_width = orig_info.width as usize;
        let orig_height = orig_info.height as usize;
        
        decoder.set_downscale_factor(factor);
        let out_width = decoder.output_width();
        let out_height = decoder.output_height();
        
        assert_eq!(out_width, orig_width / factor);
        assert_eq!(out_height, orig_height / factor);
        
        let out_colorspace = decoder.options().jpeg_get_out_colorspace();
        let channels = out_colorspace.num_components();
        
        let pixels = decoder.decode().unwrap();
        assert_eq!(pixels.len(), out_width * out_height * channels, "Pixel length mismatch at factor {}", factor);
    }
}
