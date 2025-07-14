#!/usr/bin/env bash

# Test helpers for rsynx bats tests

# Global variables
RSYNX_BIN="cargo run --"
TEST_DIR="$BATS_TMPDIR/rsynx_test_$$"
SRC_DIR="$TEST_DIR/src"
DST_DIR="$TEST_DIR/dst"
SERVER_PORT=7878
SERVER_PID=""

# Store original directory
ORIGINAL_DIR="$(pwd)"

# Setup test environment
setup_test_env() {
    mkdir -p "$SRC_DIR" "$DST_DIR"
    cd "$TEST_DIR"
}

# Cleanup test environment
cleanup_test_env() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        SERVER_PID=""
    fi
    
    if [[ -d "$TEST_DIR" ]]; then
        rm -rf "$TEST_DIR"
    fi
}

# Create test file with content
create_test_file() {
    local file_path="$1"
    local content="$2"
    
    mkdir -p "$(dirname "$file_path")"
    echo -n "$content" > "$file_path"
}

# Create test directory structure
create_test_structure() {
    local base_dir="$1"
    shift
    
    mkdir -p "$base_dir"
    
    for item in "$@"; do
        if [[ "$item" == */ ]]; then
            # Directory
            mkdir -p "$base_dir/${item%/}"
        else
            # File
            if [[ "$item" == *:* ]]; then
                # File with content
                local file_name="${item%:*}"
                local content="${item#*:}"
                create_test_file "$base_dir/$file_name" "$content"
            else
                # Empty file
                create_test_file "$base_dir/$item" ""
            fi
        fi
    done
}

# Run rsynx command
run_rsynx() {
    cd "$ORIGINAL_DIR"
    run $RSYNX_BIN "$@"
}

# Start rsynx server in background
start_server() {
    local port="${1:-$SERVER_PORT}"
    local block_size="${2:-1024}"
    
    cd "$ORIGINAL_DIR"
    $RSYNX_BIN --server --port "$port" --block-size "$block_size" &
    SERVER_PID=$!
    
    # Wait for server to start
    sleep 0.5
    
    # Check if server is running
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "Failed to start server" >&2
        return 1
    fi
}

# Stop server
stop_server() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        SERVER_PID=""
    fi
}

# Check if file exists
file_exists() {
    [[ -f "$1" ]]
}

# Check if directory exists
dir_exists() {
    [[ -d "$1" ]]
}

# Get file content
get_file_content() {
    if [[ -f "$1" ]]; then
        cat "$1"
    else
        echo "File not found: $1" >&2
        return 1
    fi
}

# Compare file contents
files_equal() {
    local file1="$1"
    local file2="$2"
    
    if [[ ! -f "$file1" ]] || [[ ! -f "$file2" ]]; then
        return 1
    fi
    
    cmp -s "$file1" "$file2"
}

# Get file size
get_file_size() {
    if [[ -f "$1" ]]; then
        stat -c%s "$1" 2>/dev/null || stat -f%z "$1" 2>/dev/null || wc -c < "$1"
    else
        echo "0"
    fi
}

# Check if string contains substring
contains() {
    local string="$1"
    local substring="$2"
    [[ "$string" == *"$substring"* ]]
}

# Assert file exists
assert_file_exists() {
    local file="$1"
    local message="${2:-File should exist: $file}"
    
    if ! file_exists "$file"; then
        echo "$message" >&2
        return 1
    fi
}

# Assert file does not exist
assert_file_not_exists() {
    local file="$1"
    local message="${2:-File should not exist: $file}"
    
    if file_exists "$file"; then
        echo "$message" >&2
        return 1
    fi
}

# Assert files are equal
assert_files_equal() {
    local file1="$1"
    local file2="$2"
    local message="${3:-Files should be equal: $file1 and $file2}"
    
    if ! files_equal "$file1" "$file2"; then
        echo "$message" >&2
        echo "File1 content: $(get_file_content "$file1")" >&2
        echo "File2 content: $(get_file_content "$file2")" >&2
        return 1
    fi
}

# Assert output contains string
assert_output_contains() {
    local expected="$1"
    local message="${2:-Output should contain: $expected}"
    
    if ! contains "$output" "$expected"; then
        echo "$message" >&2
        echo "Actual output: $output" >&2
        return 1
    fi
}

# Assert command success
assert_success() {
    local message="${1:-Command should succeed}"
    
    if [[ "$status" -ne 0 ]]; then
        echo "$message" >&2
        echo "Exit status: $status" >&2
        echo "Output: $output" >&2
        return 1
    fi
}

# Assert command failure
assert_failure() {
    local message="${1:-Command should fail}"
    
    if [[ "$status" -eq 0 ]]; then
        echo "$message" >&2
        echo "Output: $output" >&2
        return 1
    fi
}

# Generate random string
random_string() {
    local length="${1:-10}"
    head /dev/urandom | tr -dc A-Za-z0-9 | head -c "$length"
}

# Generate test content with specific size
generate_content() {
    local size="$1"
    local char="${2:-A}"
    
    printf "%*s" "$size" "" | tr ' ' "$char"
}

# Wait for port to be available
wait_for_port() {
    local port="$1"
    local timeout="${2:-5}"
    local count=0
    
    while ! nc -z localhost "$port" 2>/dev/null; do
        if [[ $count -ge $timeout ]]; then
            echo "Timeout waiting for port $port" >&2
            return 1
        fi
        sleep 0.1
        ((count++))
    done
}

# Print test info
print_test_info() {
    echo "# Test: $BATS_TEST_DESCRIPTION" >&2
    echo "# Test directory: $TEST_DIR" >&2
}

# Setup function for all tests
setup() {
    setup_test_env
    print_test_info
}

# Teardown function for all tests
teardown() {
    cleanup_test_env
}