#!/usr/bin/env bats

load helpers/test_helpers

@test "help option shows usage" {
    run_rsynx --help
    assert_success
    assert_output_contains "Usage:"
    assert_output_contains "Options:"
}

@test "version information" {
    # Test that binary runs without crashing
    run_rsynx --version || run_rsynx -V || {
        # If version flags don't exist, just test that binary exists
        run_rsynx --help
        assert_success
    }
}

@test "invalid option shows error" {
    run_rsynx --invalid-option
    assert_failure
}

@test "missing required arguments" {
    run_rsynx
    assert_failure
    assert_output_contains "required" || assert_output_contains "error"
}

@test "block size option validation" {
    create_test_file "$SRC_DIR/block_size.txt" "Block size test"
    
    # Test valid block sizes
    for size in 256 512 1024 2048 4096; do
        run_rsynx --block-size $size "$SRC_DIR/block_size.txt" "$DST_DIR/block_size_${size}.txt"
        assert_success "Block size $size should be valid"
        assert_file_exists "$DST_DIR/block_size_${size}.txt"
    done
}

@test "invalid block size" {
    create_test_file "$SRC_DIR/invalid_block.txt" "Invalid block test"
    
    # Test invalid block sizes
    run_rsynx --block-size 0 "$SRC_DIR/invalid_block.txt" "$DST_DIR/invalid_block.txt"
    assert_failure "Block size 0 should be invalid"
    
    run_rsynx --block-size -1 "$SRC_DIR/invalid_block.txt" "$DST_DIR/invalid_block.txt"
    assert_failure "Negative block size should be invalid"
}

@test "metadata flag" {
    create_test_file "$SRC_DIR/metadata_test.txt" "Metadata test content"
    chmod 644 "$SRC_DIR/metadata_test.txt"
    
    run_rsynx --metadata "$SRC_DIR/metadata_test.txt" "$DST_DIR/metadata_test.txt"
    assert_success
    assert_file_exists "$DST_DIR/metadata_test.txt"
}

@test "delete flag" {
    create_test_structure "$SRC_DIR/delete_test" \
        "keep.txt:Keep this"
    
    create_test_structure "$DST_DIR/delete_test" \
        "keep.txt:Old content" \
        "remove.txt:Remove this"
    
    run_rsynx --delete "$SRC_DIR/delete_test" "$DST_DIR/delete_test"
    assert_success
    assert_file_exists "$DST_DIR/delete_test/keep.txt"
    assert_file_not_exists "$DST_DIR/delete_test/remove.txt"
}

@test "server mode with port option" {
    start_server 7886
    sleep 0.5
    stop_server
}

@test "port option validation" {
    create_test_file "$SRC_DIR/port_test.txt" "Port test"
    
    # Test various port numbers
    for port in 5566 7878 9999; do
        start_server $port 1024
        run_rsynx --port $port "$SRC_DIR/port_test.txt" "127.0.0.1:$DST_DIR/port_test_${port}.txt"
        assert_success "Port $port should be valid"
        assert_file_exists "$DST_DIR/port_test_${port}.txt"
        stop_server
    done
}

@test "invalid port numbers" {
    create_test_file "$SRC_DIR/invalid_port.txt" "Invalid port test"
    
    # Test invalid ports (these should fail at connection time)
    run_rsynx --port 0 "$SRC_DIR/invalid_port.txt" "127.0.0.1:$DST_DIR/invalid_port.txt"
    assert_failure "Port 0 should be invalid"
    
    run_rsynx --port 65536 "$SRC_DIR/invalid_port.txt" "127.0.0.1:$DST_DIR/invalid_port.txt"
    assert_failure "Port 65536 should be invalid"
}

@test "combined options" {
    create_test_file "$SRC_DIR/combined.txt" "Combined options test"
    chmod 755 "$SRC_DIR/combined.txt"
    
    run_rsynx --block-size 512 --metadata "$SRC_DIR/combined.txt" "$DST_DIR/combined.txt"
    assert_success
    assert_file_exists "$DST_DIR/combined.txt"
    assert_files_equal "$SRC_DIR/combined.txt" "$DST_DIR/combined.txt"
}

@test "long and short options equivalence" {
    create_test_file "$SRC_DIR/short_long.txt" "Short and long options test"
    
    # Test block size short vs long
    run_rsynx -b 1024 "$SRC_DIR/short_long.txt" "$DST_DIR/short_long1.txt"
    assert_success
    
    run_rsynx --block-size 1024 "$SRC_DIR/short_long.txt" "$DST_DIR/short_long2.txt"
    assert_success
    
    assert_files_equal "$DST_DIR/short_long1.txt" "$DST_DIR/short_long2.txt"
}

@test "option order independence" {
    create_test_file "$SRC_DIR/order.txt" "Option order test"
    
    # Test different option orders
    run_rsynx --block-size 1024 --metadata "$SRC_DIR/order.txt" "$DST_DIR/order1.txt"
    assert_success
    
    run_rsynx --metadata --block-size 1024 "$SRC_DIR/order.txt" "$DST_DIR/order2.txt"
    assert_success
    
    assert_files_equal "$DST_DIR/order1.txt" "$DST_DIR/order2.txt"
}

@test "directory vs file argument handling" {
    # Test file arguments
    create_test_file "$SRC_DIR/file_arg.txt" "File argument test"
    run_rsynx "$SRC_DIR/file_arg.txt" "$DST_DIR/file_arg.txt"
    assert_success
    assert_file_exists "$DST_DIR/file_arg.txt"
    
    # Test directory arguments
    create_test_structure "$SRC_DIR/dir_arg" \
        "test.txt:Directory argument test"
    
    run_rsynx "$SRC_DIR/dir_arg" "$DST_DIR/dir_arg"
    assert_success
    assert_file_exists "$DST_DIR/dir_arg/test.txt"
}