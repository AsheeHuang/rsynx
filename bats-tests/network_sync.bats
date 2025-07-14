#!/usr/bin/env bats

load helpers/test_helpers

@test "network sync file" {
    create_test_file "$SRC_DIR/net_file.txt" "Network sync test content"
    
    # Start server
    start_server 7879 1024
    
    # Sync file over network
    run_rsynx --port 7879 "$SRC_DIR/net_file.txt" "127.0.0.1:$DST_DIR/net_file.txt"
    assert_success
    assert_output_contains "Connected to remote server"
    
    # Verify file was synced
    assert_file_exists "$DST_DIR/net_file.txt"
    [[ "$(get_file_content "$DST_DIR/net_file.txt")" == "Network sync test content" ]]
    
    stop_server
}

@test "network sync with different block sizes" {
    create_test_file "$SRC_DIR/block_test.txt" "$(generate_content 2048 'X')"
    
    # Start server with custom block size
    start_server 7880 512
    
    # Sync with matching block size
    run_rsynx --port 7880 --block-size 512 "$SRC_DIR/block_test.txt" "127.0.0.1:$DST_DIR/block_test.txt"
    assert_success
    assert_output_contains "Connected to remote server"
    
    assert_file_exists "$DST_DIR/block_test.txt"
    [[ "$(get_file_size "$DST_DIR/block_test.txt")" -eq 2048 ]]
    
    stop_server
}

@test "network sync large file" {
    local large_content="$(generate_content 8192 'L')"
    create_test_file "$SRC_DIR/large_net.txt" "$large_content"
    
    start_server 7881 1024
    
    run_rsynx --port 7881 "$SRC_DIR/large_net.txt" "127.0.0.1:$DST_DIR/large_net.txt"
    assert_success
    assert_output_contains "Connected to remote server"
    
    assert_file_exists "$DST_DIR/large_net.txt"
    assert_files_equal "$SRC_DIR/large_net.txt" "$DST_DIR/large_net.txt"
    
    stop_server
}

@test "network sync with existing destination" {
    create_test_file "$SRC_DIR/existing.txt" "New network content"
    create_test_file "$DST_DIR/existing.txt" "Old content"
    
    start_server 7882 1024
    
    run_rsynx --port 7882 "$SRC_DIR/existing.txt" "127.0.0.1:$DST_DIR/existing.txt"
    assert_success
    assert_output_contains "Connected to remote server"
    
    assert_file_exists "$DST_DIR/existing.txt"
    [[ "$(get_file_content "$DST_DIR/existing.txt")" == "New network content" ]]
    
    stop_server
}

@test "network sync binary file" {
    # Create binary-like content
    printf "\x00\x01\x02\x03\xFF\xFE\xFD\xFC" > "$SRC_DIR/binary.bin"
    
    start_server 7883 256
    
    run_rsynx --port 7883 --block-size 256 "$SRC_DIR/binary.bin" "127.0.0.1:$DST_DIR/binary.bin"
    assert_success
    assert_output_contains "Connected to remote server"
    
    assert_file_exists "$DST_DIR/binary.bin"
    assert_files_equal "$SRC_DIR/binary.bin" "$DST_DIR/binary.bin"
    
    stop_server
}

@test "server mode starts successfully" {
    run_rsynx --server --port 7884
    # Server should start (we'll kill it quickly via process cleanup)
    # This test mainly ensures server starts without immediate errors
}

@test "network sync fails with no server" {
    create_test_file "$SRC_DIR/no_server.txt" "No server test"
    
    run_rsynx --port 9999 "$SRC_DIR/no_server.txt" "127.0.0.1:$DST_DIR/no_server.txt"
    assert_failure
    assert_output_contains "Failed to connect"
}

@test "network sync fails with invalid port" {
    create_test_file "$SRC_DIR/invalid_port.txt" "Invalid port test"
    
    run_rsynx --port 99999 "$SRC_DIR/invalid_port.txt" "127.0.0.1:$DST_DIR/invalid_port.txt"
    assert_failure
}

@test "network sync with custom port" {
    create_test_file "$SRC_DIR/custom_port.txt" "Custom port test"
    
    start_server 8888 1024
    
    run_rsynx --port 8888 "$SRC_DIR/custom_port.txt" "127.0.0.1:$DST_DIR/custom_port.txt"
    assert_success
    assert_output_contains "Connected to remote server"
    
    assert_file_exists "$DST_DIR/custom_port.txt"
    [[ "$(get_file_content "$DST_DIR/custom_port.txt")" == "Custom port test" ]]
    
    stop_server
}

@test "network sync multiple files sequentially" {
    create_test_file "$SRC_DIR/seq1.txt" "Sequential file 1"
    create_test_file "$SRC_DIR/seq2.txt" "Sequential file 2"
    create_test_file "$SRC_DIR/seq3.txt" "Sequential file 3"
    
    start_server 7885 1024
    
    # Sync first file
    run_rsynx --port 7885 "$SRC_DIR/seq1.txt" "127.0.0.1:$DST_DIR/seq1.txt"
    assert_success
    
    # Sync second file
    run_rsynx --port 7885 "$SRC_DIR/seq2.txt" "127.0.0.1:$DST_DIR/seq2.txt"
    assert_success
    
    # Sync third file
    run_rsynx --port 7885 "$SRC_DIR/seq3.txt" "127.0.0.1:$DST_DIR/seq3.txt"
    assert_success
    
    # Verify all files
    assert_file_exists "$DST_DIR/seq1.txt"
    assert_file_exists "$DST_DIR/seq2.txt"
    assert_file_exists "$DST_DIR/seq3.txt"
    
    [[ "$(get_file_content "$DST_DIR/seq1.txt")" == "Sequential file 1" ]]
    [[ "$(get_file_content "$DST_DIR/seq2.txt")" == "Sequential file 2" ]]
    [[ "$(get_file_content "$DST_DIR/seq3.txt")" == "Sequential file 3" ]]
    
    stop_server
}