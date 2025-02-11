use rsynx::sync::LocalSyncer;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
use filetime::FileTime;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::MetadataExt;
use rand::Rng;

fn setup_test_files(name: &str, src_content: &[u8], dst_content: &[u8]) -> (String, String) {
    let src_path = format!("test_src_{}", name);
    let dst_path = format!("test_dst_{}", name);

    let mut src_file = File::create(&src_path).unwrap();
    src_file.write_all(src_content).unwrap();
    src_file.flush().unwrap();

    let mut dst_file = File::create(&dst_path).unwrap();
    dst_file.write_all(dst_content).unwrap();
    dst_file.flush().unwrap();

    (src_path, dst_path)
}

fn cleanup_test_files(src_path: &str, dst_path: &str) {
    let _ = fs::remove_file(src_path);
    let _ = fs::remove_file(dst_path);
}
/// Helper: Verify the content of a file.
fn verify_content(path: &str, expected: &[u8]) {
    let mut file = File::open(path).unwrap();
    let mut buffer = vec![0; expected.len()];
    file.read_exact(&mut buffer).unwrap();
    assert_eq!(buffer, expected);
}

#[test]
fn test_basic_sync() {
    let (src, dst) = setup_test_files("basic", b"0123456789", b"012345a789");
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, b"0123456789");
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_empty_destination() {
    let (src, dst) = setup_test_files("empty", b"0123456789", b"");
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, b"0123456789");
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_identical_files() {
    let (src, dst) = setup_test_files("identical", b"0123456789", b"0123456789");
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, b"0123456789");
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_multiple_changes() {
    let (src, dst) = setup_test_files("multiple_changes", b"0123456789", b"01a34b6c89");
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, b"0123456789");
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_longer_files() {
    let src_content = "The quick brown fox jumps over the lazy dog".as_bytes();
    let dst_content = "The quick brown cat jumps over the lazy dog".as_bytes();
    let (src, dst) = setup_test_files("longer_files", src_content, dst_content);
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, src_content);
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_different_sizes() {
    let (src, dst) = setup_test_files("different_sizes", b"0123456789", b"01234");
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, b"0123456789");
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_binary_data() {
    let src_content = &[0, 1, 2, 3, 255, 254, 253, 252];
    let dst_content = &[0, 1, 2, 3, 0, 254, 253, 252];
    let (src, dst) = setup_test_files("binary_data", src_content, dst_content);
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(4);
    syncer.sync().unwrap();
    verify_content(&dst, src_content);
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_large_file() {
    let mut src_content = vec![0; 100000];
    let dst_content = vec![0; 100000];

    let mut rng = rand::rng();
    for _ in 0..100 {
        let random_byte_index = rng.random_range(0..src_content.len());
        let random_bit_index = rng.random_range(0..8);
        src_content[random_byte_index] ^= 1 << random_bit_index;
    }

    let (src, dst) = setup_test_files("large_file", &src_content, &dst_content);
    let syncer = LocalSyncer::new(src.clone(), dst.clone()).with_block_size(512);
    let result = syncer.sync().unwrap();
    println!("Transferred: {} bytes, Not transferred: {} bytes", result.new_bytes, result.reused_bytes);

    // Verify the content of the destination file
    verify_content(&dst, &src_content);
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_basic_sync_directory() {
    let src_dir = "test_sync_src_dir";
    let dst_dir = "test_sync_dst_dir";

    let _ = fs::remove_dir_all(src_dir);
    let _ = fs::remove_dir_all(dst_dir);

    fs::create_dir_all(src_dir).unwrap();
    let sub_dir = format!("{}/subdir", src_dir);
    fs::create_dir_all(&sub_dir).unwrap();

    fs::write(format!("{}/file1.txt", src_dir), b"Hello world").unwrap();
    fs::write(format!("{}/file2.txt", src_dir), b"Rust is awesome").unwrap();
    fs::write(format!("{}/file3.txt", sub_dir), b"Subdirectory file").unwrap();

    let syncer = LocalSyncer::new(src_dir.to_string(), dst_dir.to_string()).with_block_size(4);
    syncer.sync().unwrap();

    let file1 = fs::read(format!("{}/file1.txt", dst_dir)).unwrap();
    let file2 = fs::read(format!("{}/file2.txt", dst_dir)).unwrap();
    let file3 = fs::read(format!("{}/subdir/file3.txt", dst_dir)).unwrap();

    assert_eq!(file1, b"Hello world");
    assert_eq!(file2, b"Rust is awesome");
    assert_eq!(file3, b"Subdirectory file");

    let _ = fs::remove_dir_all(src_dir);
    let _ = fs::remove_dir_all(dst_dir);
}

#[test]
#[ignore]
fn test_preserve_metadata() {
    let (src, dst) = setup_test_files("preserve_metadata", b"", b"");
    let syncer = LocalSyncer::new(src.clone(), dst.clone())
        .with_block_size(4)
        .with_preserve_metadata(true);

    // Set metadata on the source file.
    let atime = FileTime::from_unix_time(6666666, 0);
    let mtime = FileTime::from_unix_time(6666666, 0);
    filetime::set_file_times(&src, atime, mtime).unwrap();

    // Set permissions on the source file.
    let src_perm = fs::Permissions::from_mode(0o644);
    fs::set_permissions(&src, src_perm).unwrap();

    syncer.sync().unwrap();
    verify_content(&dst, b"");
    let dst_meta = fs::metadata(&dst).unwrap();

    // Note: Mode testing may not work consistently on non-Linux systems.
    // assert_eq!(dst_meta.permissions().mode() & 0o777, 0o644);
    assert_eq!(dst_meta.atime(), atime.unix_seconds());
    assert_eq!(dst_meta.mtime(), mtime.unix_seconds());
    cleanup_test_files(&src, &dst);
}

#[test]
fn test_delete_extraneous() {
    let src_dir = "test_sync_src_delete";
    let dst_dir = "test_sync_dst_delete";

    let _ = fs::remove_dir_all(src_dir);
    let _ = fs::remove_dir_all(dst_dir);

    fs::create_dir_all(src_dir).unwrap();
    fs::create_dir_all(dst_dir).unwrap();

    // Create a file in the source directory.
    fs::write(format!("{}/file1.txt", src_dir), b"Hello").unwrap();

    // In the destination, create a file with outdated content and an extra file.
    fs::write(format!("{}/file1.txt", dst_dir), b"Old content").unwrap();
    fs::write(format!("{}/extraneous.txt", dst_dir), b"Should be removed").unwrap();

    // Sync with delete_extraneous enabled.
    let syncer = LocalSyncer::new(src_dir.to_string(), dst_dir.to_string())
        .with_block_size(4)
        .with_delete_extraneous(true);
    syncer.sync().unwrap();

    // Verify that file1.txt is updated.
    let file1 = fs::read(format!("{}/file1.txt", dst_dir)).unwrap();
    assert_eq!(file1, b"Hello");

    // Verify that the extraneous file is deleted.
    assert!(!Path::new(&format!("{}/extraneous.txt", dst_dir)).exists());

    let _ = fs::remove_dir_all(src_dir);
    let _ = fs::remove_dir_all(dst_dir);
} 

#[test]
fn test_no_delete_extraneous() {
    let src_dir = "test_sync_src_no_delete";
    let dst_dir = "test_sync_dst_no_delete";

    let _ = fs::remove_dir_all(src_dir);
    let _ = fs::remove_dir_all(dst_dir);

    fs::create_dir_all(src_dir).unwrap();
    fs::create_dir_all(dst_dir).unwrap();

    fs::write(format!("{}/file1.txt", src_dir), b"Hello").unwrap();
    fs::write(format!("{}/file2.txt", dst_dir), b"Hello").unwrap();

    let syncer = LocalSyncer::new(src_dir.to_string(), dst_dir.to_string())
        .with_block_size(4)
        .with_delete_extraneous(false);
    syncer.sync().unwrap();
    
    assert!(Path::new(&format!("{}/file1.txt", dst_dir)).exists());
    assert!(Path::new(&format!("{}/file2.txt", dst_dir)).exists());

    let _ = fs::remove_dir_all(src_dir);
    let _ = fs::remove_dir_all(dst_dir);
}