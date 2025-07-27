use rsynx::local_sync::LocalSyncer;
use std::{
    fs::{self, File},
    io::{Read, Write},
};

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

#[test]
fn test_compression_sync() {
    let content = b"This is a test file with some repeated content. ".repeat(100);
    let (src_path, dst_path) = setup_test_files("compression", &content, b"");

    // Sync with compression enabled
    let syncer = LocalSyncer::new(src_path.clone(), dst_path.clone())
        .with_block_size(1024)
        .with_compression(true);
    let result = syncer.sync().unwrap();

    // Verify the files are identical
    let mut src_content = Vec::new();
    let mut dst_content = Vec::new();
    File::open(&src_path)
        .unwrap()
        .read_to_end(&mut src_content)
        .unwrap();
    File::open(&dst_path)
        .unwrap()
        .read_to_end(&mut dst_content)
        .unwrap();

    assert_eq!(src_content, dst_content);
    assert!(result.new_bytes > 0);

    cleanup_test_files(&src_path, &dst_path);
}

#[test]
fn test_compression_vs_no_compression() {
    let content = b"This is a test file with some repeated content. ".repeat(50);
    let (src_path1, dst_path1) = setup_test_files("no_compression", &content, b"");
    let (src_path2, dst_path2) = setup_test_files("with_compression", &content, b"");

    // Sync without compression
    let syncer1 = LocalSyncer::new(src_path1.clone(), dst_path1.clone())
        .with_block_size(1024)
        .with_compression(false);
    let result1 = syncer1.sync().unwrap();

    // Sync with compression
    let syncer2 = LocalSyncer::new(src_path2.clone(), dst_path2.clone())
        .with_block_size(1024)
        .with_compression(true);
    let result2 = syncer2.sync().unwrap();

    // Both should produce identical results
    let mut dst_content1 = Vec::new();
    let mut dst_content2 = Vec::new();
    File::open(&dst_path1)
        .unwrap()
        .read_to_end(&mut dst_content1)
        .unwrap();
    File::open(&dst_path2)
        .unwrap()
        .read_to_end(&mut dst_content2)
        .unwrap();

    assert_eq!(dst_content1, dst_content2);
    assert_eq!(result1.new_bytes, result2.new_bytes);

    cleanup_test_files(&src_path1, &dst_path1);
    cleanup_test_files(&src_path2, &dst_path2);
}

#[test]
fn test_compression_small_file() {
    let content = b"Small file";
    let (src_path, dst_path) = setup_test_files("small_compression", content, b"");

    let syncer = LocalSyncer::new(src_path.clone(), dst_path.clone()).with_compression(true);
    let result = syncer.sync().unwrap();

    // Verify content is correct
    let mut dst_content = Vec::new();
    File::open(&dst_path)
        .unwrap()
        .read_to_end(&mut dst_content)
        .unwrap();
    assert_eq!(content, dst_content.as_slice());
    assert_eq!(result.new_bytes, content.len());

    cleanup_test_files(&src_path, &dst_path);
}
