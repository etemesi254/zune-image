use std::process::Command;

fn main()
{
    // https://stackoverflow.com/questions/51621642/how-to-specify-an-environment-variable-using-the-rustc-env-flag
    let string = String::from_utf8(
        Command::new("git")
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    println!("cargo:rustc-env=ZUNE_JPEG_GIT_HASH={}", string);
}
