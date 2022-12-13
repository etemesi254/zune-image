use zune_jpeg::JpegDecoder;

#[test]
#[ignore = "TODO: fix this"]
fn eof()
{
    let mut decoder = JpegDecoder::new(&[0xff, 0xd8, 0xa4]);

    let err = decoder.decode().unwrap_err();

    assert!(matches!(err, zune_jpeg::errors::DecodeErrors::Format(_)));
}

#[test]
fn bad_ff_marker_size()
{
    let mut decoder = JpegDecoder::new(&[0xff, 0xd8, 0xff, 0x00, 0x00, 0x00]);

    let err = decoder.decode().unwrap_err();
    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::Format(x) if x == "Found a marker with invalid length : 0")
    );
}

#[test]
fn bad_number_of_scans()
{
    let mut decoder = JpegDecoder::new(&[255, 216, 255, 218, 232, 197, 255]);

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::SosError(x) if x == "Bad SOS length 59589,corrupt jpeg")
    );
}

#[test]
fn huffman_length_subtraction_overflow()
{
    let mut decoder = JpegDecoder::new(&[255, 216, 255, 196, 0, 0]);

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::FormatStatic(x) if x == "Invalid Huffman length in image")
    );
}

#[test]
#[ignore = "TODO: fix this"]
fn index_oob()
{
    let mut decoder = JpegDecoder::new(&[255, 216, 255, 218, 0, 8, 1, 0, 8, 1]);

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::HuffmanDecode(x) if x == "Invalid Huffman length in image")
    );
}

#[test]
fn mul_with_overflow()
{
    let mut decoder = JpegDecoder::new(&[255, 216, 255, 192, 255, 1, 8, 9, 119, 48, 255, 192]);

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::SofError(x) if x == "Length of start of frame differs from expected 584,value is 65281")
    );
}
