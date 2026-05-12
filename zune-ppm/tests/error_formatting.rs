use zune_ppm::{PPMDecodeErrors, PPMEncodeErrors};

#[test]
fn test_decode_error_no_trailing_newline() {
    let errors = vec![
        PPMDecodeErrors::GenericStatic("test error"),
        PPMDecodeErrors::InvalidHeader("bad header".to_string()),
        PPMDecodeErrors::LargeDimensions(1000, 2000),
    ];

    for error in errors {
        let formatted = format!("{:?}", error);
        assert!(
            !formatted.ends_with('\n'),
            "PPMDecodeErrors should not end with newline: {:?}",
            formatted
        );
    }
}

#[test]
fn test_encode_error_no_trailing_newline() {
    let error = PPMEncodeErrors::Static("test error");
    let formatted = format!("{:?}", error);
    assert!(
        !formatted.ends_with('\n'),
        "PPMEncodeErrors should not end with newline: {:?}",
        formatted
    );
}
