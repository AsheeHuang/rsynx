#!/usr/bin/env bats

load helpers/test_helpers

@test "sync very large files" {
    skip "Skipping large file test - uncomment to run manually"
    
    # Create 10MB file
    local large_content="$(generate_content 10485760 'L')"
    create_test_file "$SRC_DIR/very_large.txt" "$large_content"
    
    run_rsynx --block-size 4096 "$SRC_DIR/very_large.txt" "$DST_DIR/very_large.txt"
    assert_success
    assert_file_exists "$DST_DIR/very_large.txt"
    [[ "$(get_file_size "$DST_DIR/very_large.txt")" -eq 10485760 ]]
}

@test "sync files with special characters in names" {
    create_test_file "$SRC_DIR/special-chars_@#$.txt" "Special characters test"
    
    run_rsynx "$SRC_DIR/special-chars_@#$.txt" "$DST_DIR/special-chars_@#$.txt"
    assert_success
    assert_file_exists "$DST_DIR/special-chars_@#$.txt"
    [[ "$(get_file_content "$DST_DIR/special-chars_@#$.txt")" == "Special characters test" ]]
}

@test "sync files with spaces in names" {
    create_test_file "$SRC_DIR/file with spaces.txt" "Spaces in filename test"
    
    run_rsynx "$SRC_DIR/file with spaces.txt" "$DST_DIR/file with spaces.txt"
    assert_success
    assert_file_exists "$DST_DIR/file with spaces.txt"
    [[ "$(get_file_content "$DST_DIR/file with spaces.txt")" == "Spaces in filename test" ]]
}

@test "sync binary files with null bytes" {
    # Create file with null bytes and binary content
    printf "Binary\x00Content\x00With\x00Nulls\x00\xFF\xFE\xFD" > "$SRC_DIR/binary_nulls.bin"
    
    run_rsynx --block-size 256 "$SRC_DIR/binary_nulls.bin" "$DST_DIR/binary_nulls.bin"
    assert_success
    assert_file_exists "$DST_DIR/binary_nulls.bin"
    assert_files_equal "$SRC_DIR/binary_nulls.bin" "$DST_DIR/binary_nulls.bin"
}

@test "sync files with only newlines" {
    printf "\n\n\n\n\n" > "$SRC_DIR/only_newlines.txt"
    
    run_rsynx "$SRC_DIR/only_newlines.txt" "$DST_DIR/only_newlines.txt"
    assert_success
    assert_file_exists "$DST_DIR/only_newlines.txt"
    assert_files_equal "$SRC_DIR/only_newlines.txt" "$DST_DIR/only_newlines.txt"
}

@test "sync files with unicode content" {
    printf "Unicode test: 你好世界 🌍 émojis ñáéíóú" > "$SRC_DIR/unicode.txt"
    
    run_rsynx "$SRC_DIR/unicode.txt" "$DST_DIR/unicode.txt"
    assert_success
    assert_file_exists "$DST_DIR/unicode.txt"
    assert_files_equal "$SRC_DIR/unicode.txt" "$DST_DIR/unicode.txt"
}

@test "sync deeply nested directory structure" {
    # Create deeply nested structure
    local deep_path="$SRC_DIR/level1/level2/level3/level4/level5"
    mkdir -p "$deep_path"
    create_test_file "$deep_path/deep_file.txt" "Deep nesting test"
    
    run_rsynx "$SRC_DIR/level1" "$DST_DIR/level1"
    assert_success
    assert_file_exists "$DST_DIR/level1/level2/level3/level4/level5/deep_file.txt"
    [[ "$(get_file_content "$DST_DIR/level1/level2/level3/level4/level5/deep_file.txt")" == "Deep nesting test" ]]
}

@test "sync directory with many files" {
    # Create directory with many small files
    mkdir -p "$SRC_DIR/many_files"
    for i in {1..100}; do
        create_test_file "$SRC_DIR/many_files/file_$i.txt" "Content of file $i"
    done
    
    run_rsynx "$SRC_DIR/many_files" "$DST_DIR/many_files"
    assert_success
    
    # Check a few random files
    for i in 1 25 50 75 100; do
        assert_file_exists "$DST_DIR/many_files/file_$i.txt"
        [[ "$(get_file_content "$DST_DIR/many_files/file_$i.txt")" == "Content of file $i" ]]
    done
}

@test "sync with minimal block size" {
    create_test_file "$SRC_DIR/minimal_block.txt" "Minimal block size test content"
    
    run_rsynx --block-size 1 "$SRC_DIR/minimal_block.txt" "$DST_DIR/minimal_block.txt"
    assert_success
    assert_file_exists "$DST_DIR/minimal_block.txt"
    assert_files_equal "$SRC_DIR/minimal_block.txt" "$DST_DIR/minimal_block.txt"
}

@test "sync with maximum reasonable block size" {
    create_test_file "$SRC_DIR/max_block.txt" "$(generate_content 32768 'M')"
    
    run_rsynx --block-size 32768 "$SRC_DIR/max_block.txt" "$DST_DIR/max_block.txt"
    assert_success
    assert_file_exists "$DST_DIR/max_block.txt"
    assert_files_equal "$SRC_DIR/max_block.txt" "$DST_DIR/max_block.txt"
}

@test "sync file that shrinks" {
    create_test_file "$SRC_DIR/shrink.txt" "Short"
    create_test_file "$DST_DIR/shrink.txt" "Much longer content that will be replaced"
    
    run_rsynx "$SRC_DIR/shrink.txt" "$DST_DIR/shrink.txt"
    assert_success
    assert_files_equal "$SRC_DIR/shrink.txt" "$DST_DIR/shrink.txt"
    [[ "$(get_file_content "$DST_DIR/shrink.txt")" == "Short" ]]
}

@test "sync file that grows" {
    create_test_file "$SRC_DIR/grow.txt" "Much longer content that replaces the short one"
    create_test_file "$DST_DIR/grow.txt" "Short"
    
    run_rsynx "$SRC_DIR/grow.txt" "$DST_DIR/grow.txt"
    assert_success
    assert_files_equal "$SRC_DIR/grow.txt" "$DST_DIR/grow.txt"
    [[ "$(get_file_content "$DST_DIR/grow.txt")" == "Much longer content that replaces the short one" ]]
}

@test "sync with readonly destination directory" {
    create_test_file "$SRC_DIR/readonly_test.txt" "Readonly test content"
    mkdir -p "$DST_DIR/readonly_dir"
    
    # Make directory readonly
    chmod 555 "$DST_DIR/readonly_dir"
    
    run_rsynx "$SRC_DIR/readonly_test.txt" "$DST_DIR/readonly_dir/readonly_test.txt"
    # This should fail due to permissions
    assert_failure
    
    # Restore permissions for cleanup
    chmod 755 "$DST_DIR/readonly_dir"
}

@test "sync symlink behavior" {
    create_test_file "$SRC_DIR/target.txt" "Symlink target content"
    ln -s "$SRC_DIR/target.txt" "$SRC_DIR/symlink.txt"
    
    # Test syncing the symlink itself
    run_rsynx "$SRC_DIR/symlink.txt" "$DST_DIR/symlink.txt"
    assert_success
    assert_file_exists "$DST_DIR/symlink.txt"
    [[ "$(get_file_content "$DST_DIR/symlink.txt")" == "Symlink target content" ]]
}

@test "sync zero-byte file" {
    touch "$SRC_DIR/zero_byte.txt"
    create_test_file "$DST_DIR/zero_byte.txt" "Has content"
    
    run_rsynx "$SRC_DIR/zero_byte.txt" "$DST_DIR/zero_byte.txt"
    assert_success
    assert_file_exists "$DST_DIR/zero_byte.txt"
    [[ "$(get_file_size "$DST_DIR/zero_byte.txt")" -eq 0 ]]
}

@test "concurrent sync attempts" {
    create_test_file "$SRC_DIR/concurrent.txt" "Concurrent test"
    
    # Start multiple sync operations
    run_rsynx "$SRC_DIR/concurrent.txt" "$DST_DIR/concurrent1.txt" &
    run_rsynx "$SRC_DIR/concurrent.txt" "$DST_DIR/concurrent2.txt" &
    run_rsynx "$SRC_DIR/concurrent.txt" "$DST_DIR/concurrent3.txt" &
    
    wait
    
    # All should succeed
    assert_file_exists "$DST_DIR/concurrent1.txt"
    assert_file_exists "$DST_DIR/concurrent2.txt"
    assert_file_exists "$DST_DIR/concurrent3.txt"
}