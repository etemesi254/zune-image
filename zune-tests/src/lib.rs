use std::fs::{read, read_dir, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::*;

/// A random seed constant,
///
/// Has no entropy importance, just an important number
/// to me
const SEED: u64 = 37;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestEntry
{
    file: String,
    hash: u64
}

/// Calculate the hash of bytes using SEED as a seed
/// constant
fn calculate_hash(bytes: &[u8]) -> u128
{
    xxhash_rust::xxh3::xxh3_128_with_seed(bytes, SEED)
}

pub fn create_hash(file: String, bytes: &[u8]) -> TestEntry
{
    let hash = (calculate_hash(bytes) >> 64) as u64;

    TestEntry { file, hash }
}

/// Create a list of test entries from a directory by reading all files
/// in the directory and creating  a  hash from them
///
/// # Arguments
/// - main_dir: CARGO_ENV_DIR
/// - extra: Path relative to the repo
///
/// - filter: A function that when called returns true or false indicating whether the file
/// should be decoded.
/// E.g can be used to accept only some file types in a directory with mixed entries.
///  On true, the file is decoded, on false the file is skipped
///
/// - decoder: The actual decoder implementation, the decoder should override `decode` trait to get
/// and returns pixels from the image.
/// Those pixels will be hashed by using xxh3 and the 128 bits will be stored
///
/// # Returns.
/// - The hashes of all passed files in the directory.
///
/// # Panics
/// - A lot
pub fn create_tests_from_dirs<T, F>(
    main_dir: String, extra: String, filter: F, decoder: &mut T
) -> Vec<TestEntry>
where
    T: TestTrait,
    F: Fn(&PathBuf) -> bool
{
    // read directory we were passed
    let directories = read_dir(&(main_dir.clone() + &extra)).unwrap();

    let mut hashes = Vec::with_capacity(21);

    for directory in directories.flatten()
    {
        if directory.path().is_file() && filter(&directory.path())
        // ignore nested directories.
        {
            dbg!(directory.path());
            // read file
            let file_contents = read(directory.path()).unwrap();
            let pixels = decoder.decode(&file_contents);
            let file_path = directory
                .path()
                .to_str()
                .unwrap()
                .to_string()
                .replace(&main_dir, "");

            let test = create_hash(file_path, &pixels);

            hashes.push(test);
        }
    }
    hashes
}

pub trait TestTrait
{
    ///
    fn decode(&mut self, compressed_bytes: &[u8]) -> Vec<u8>;
}

#[derive(Serialize, Deserialize)]
struct Tests
{
    file: Vec<TestEntry>
}

pub fn write_to_file<P: AsRef<Path>>(file: P, tests: &[TestEntry])
{
    let t = Tests {
        file: tests.to_vec()
    };
    let string = toml::to_vec(&t).unwrap();

    OpenOptions::new()
        .write(true)
        .create(true)
        .open(file)
        .unwrap()
        .write_all(&string)
        .unwrap();
}
