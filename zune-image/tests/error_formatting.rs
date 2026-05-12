use zune_image::errors::{ImgErrors, ImgOperationsErrors, ImgEncodeErrors};
use zune_image::channel::ChannelErrors;

#[test]
fn test_error_no_trailing_newline() {
    // Test ImgErrors variants
    let img_error = ImgErrors::GenericStr("test error");
    let formatted = format!("{:?}", img_error);
    assert!(
        !formatted.ends_with('\n'),
        "ImgErrors should not end with newline: {:?}",
        formatted
    );

    // Test ImgOperationsErrors
    let ops_error = ImgOperationsErrors::Generic("test error");
    let formatted = format!("{:?}", ops_error);
    assert!(
        !formatted.ends_with('\n'),
        "ImgOperationsErrors should not end with newline: {:?}",
        formatted
    );

    // Test ImgEncodeErrors
    let encode_error = ImgEncodeErrors::GenericStatic("test error");
    let formatted = format!("{:?}", encode_error);
    assert!(
        !formatted.ends_with('\n'),
        "ImgEncodeErrors should not end with newline: {:?}",
        formatted
    );

    // Test ChannelErrors
    let channel_error = ChannelErrors::UnalignedPointer(8, 4);
    let formatted = format!("{:?}", channel_error);
    assert!(
        !formatted.ends_with('\n'),
        "ChannelErrors should not end with newline: {:?}",
        formatted
    );
}
