#!/usr/bin/env bats

load helpers/test_helpers

@test "sync identical files" {
    create_test_file "$SRC_DIR/file1.txt" "Hello World"
    create_test_file "$DST_DIR/file1.txt" "Hello World"
    
    run_rsynx "$SRC_DIR/file1.txt" "$DST_DIR/file1.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_files_equal "$SRC_DIR/file1.txt" "$DST_DIR/file1.txt"
}

@test "sync different files" {
    create_test_file "$SRC_DIR/file2.txt" "New content"
    create_test_file "$DST_DIR/file2.txt" "Old content"
    
    run_rsynx "$SRC_DIR/file2.txt" "$DST_DIR/file2.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_files_equal "$SRC_DIR/file2.txt" "$DST_DIR/file2.txt"
    [[ "$(get_file_content "$DST_DIR/file2.txt")" == "New content" ]]
}

@test "sync non-existent destination" {
    create_test_file "$SRC_DIR/file3.txt" "Content to copy"
    
    run_rsynx "$SRC_DIR/file3.txt" "$DST_DIR/file3.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_file_exists "$DST_DIR/file3.txt"
    assert_files_equal "$SRC_DIR/file3.txt" "$DST_DIR/file3.txt"
}

@test "sync with custom block size" {
    create_test_file "$SRC_DIR/large.txt" "$(generate_content 2048)"
    create_test_file "$DST_DIR/large.txt" "$(generate_content 1024)"
    
    run_rsynx --block-size 512 "$SRC_DIR/large.txt" "$DST_DIR/large.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_files_equal "$SRC_DIR/large.txt" "$DST_DIR/large.txt"
}

@test "sync directory" {
    create_test_structure "$SRC_DIR/testdir" \
        "file1.txt:Content 1" \
        "file2.txt:Content 2" \
        "subdir/" \
        "subdir/file3.txt:Content 3"
    
    run_rsynx "$SRC_DIR/testdir" "$DST_DIR/testdir"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_file_exists "$DST_DIR/testdir/file1.txt"
    assert_file_exists "$DST_DIR/testdir/file2.txt"
    assert_file_exists "$DST_DIR/testdir/subdir/file3.txt"
    
    assert_files_equal "$SRC_DIR/testdir/file1.txt" "$DST_DIR/testdir/file1.txt"
    assert_files_equal "$SRC_DIR/testdir/file2.txt" "$DST_DIR/testdir/file2.txt"
    assert_files_equal "$SRC_DIR/testdir/subdir/file3.txt" "$DST_DIR/testdir/subdir/file3.txt"
}

@test "sync with metadata preservation" {
    create_test_file "$SRC_DIR/meta.txt" "Test metadata"
    chmod 755 "$SRC_DIR/meta.txt"
    
    run_rsynx --metadata "$SRC_DIR/meta.txt" "$DST_DIR/meta.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_file_exists "$DST_DIR/meta.txt"
    assert_files_equal "$SRC_DIR/meta.txt" "$DST_DIR/meta.txt"
}

@test "sync with delete extraneous files" {
    create_test_structure "$SRC_DIR/deldir" \
        "keep.txt:Keep this file"
    
    create_test_structure "$DST_DIR/deldir" \
        "keep.txt:Old content" \
        "delete.txt:Delete this file"
    
    run_rsynx --delete "$SRC_DIR/deldir" "$DST_DIR/deldir"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_file_exists "$DST_DIR/deldir/keep.txt"
    assert_file_not_exists "$DST_DIR/deldir/delete.txt"
    [[ "$(get_file_content "$DST_DIR/deldir/keep.txt")" == "Keep this file" ]]
}

@test "sync large file with partial changes" {
    local large_content="$(generate_content 4096 'A')"
    create_test_file "$SRC_DIR/large.txt" "$large_content"
    
    # Create destination with partial content
    local partial_content="$(generate_content 2048 'A')$(generate_content 2048 'B')"
    create_test_file "$DST_DIR/large.txt" "$partial_content"
    
    run_rsynx --block-size 1024 "$SRC_DIR/large.txt" "$DST_DIR/large.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_files_equal "$SRC_DIR/large.txt" "$DST_DIR/large.txt"
}

@test "sync empty file" {
    create_test_file "$SRC_DIR/empty.txt" ""
    create_test_file "$DST_DIR/empty.txt" "Some content"
    
    run_rsynx "$SRC_DIR/empty.txt" "$DST_DIR/empty.txt"
    assert_success
    assert_output_contains "Transferred:"
    
    assert_files_equal "$SRC_DIR/empty.txt" "$DST_DIR/empty.txt"
    [[ "$(get_file_size "$DST_DIR/empty.txt")" -eq 0 ]]
}

@test "sync with transfer statistics" {
    create_test_file "$SRC_DIR/stats.txt" "Transfer statistics test"
    create_test_file "$DST_DIR/stats.txt" "Old content"
    
    run_rsynx "$SRC_DIR/stats.txt" "$DST_DIR/stats.txt"
    assert_success
    assert_output_contains "Transferred:"
    assert_output_contains "bytes"
    assert_output_contains "Not transferred:"
}

@test "fail with non-existent source" {
    run_rsynx "$SRC_DIR/nonexistent.txt" "$DST_DIR/target.txt"
    assert_failure
}

@test "fail with invalid arguments" {
    run_rsynx
    assert_failure
    
    run_rsynx --invalid-flag
    assert_failure
}