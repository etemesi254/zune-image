use zune_inflate::errors::{DecodeErrorStatus, InflateDecodeErrors};

#[test]
fn test_error_no_trailing_newline() {
    // Test DecodeErrorStatus variants
    let errors = vec![
        DecodeErrorStatus::InsufficientData,
        DecodeErrorStatus::Generic("test error"),
        DecodeErrorStatus::GenericStr("test string error".to_string()),
        DecodeErrorStatus::CorruptData,
        DecodeErrorStatus::OutputLimitExceeded(100, 200),
        DecodeErrorStatus::MismatchedCRC(0x12345678, 0x87654321),
        DecodeErrorStatus::MismatchedAdler(0xABCDEF, 0xFEDCBA),
    ];

    for error in errors {
        let formatted = format!("{:?}", error);
        assert!(
            !formatted.ends_with('\n'),
            "Error message should not end with newline: {:?}",
            formatted
        );
    }

    // Test InflateDecodeErrors
    let inflate_error = InflateDecodeErrors::new(DecodeErrorStatus::InsufficientData, vec![]);
    let formatted = format!("{:?}", inflate_error);
    assert!(
        !formatted.ends_with('\n'),
        "InflateDecodeErrors should not end with newline: {:?}",
        formatted
    );
}
