# RSynx BATS Testing Framework

This directory contains a comprehensive BATS (Bash Automated Testing System) testing framework for the RSynx file synchronization tool.

## Overview

The BATS testing framework provides shell-based integration and end-to-end testing for RSynx, complementing the Rust unit tests with real-world command-line interface testing.

## Structure

```
bats-tests/
├── README.md                 # This file
├── helpers/
│   └── test_helpers.bash    # Common testing utilities and functions
├── local_sync.bats          # Local file synchronization tests
├── network_sync.bats        # Network synchronization tests
├── cli_options.bats         # Command-line interface option tests
└── edge_cases.bats          # Edge cases and error condition tests
```

## Test Categories

### 1. Local Sync Tests (`local_sync.bats`)
- Basic file synchronization
- Directory synchronization
- Metadata preservation
- Delete extraneous files
- Custom block sizes
- Large file handling
- Transfer statistics

### 2. Network Sync Tests (`network_sync.bats`)
- TCP client-server synchronization
- Different block sizes
- Binary file transfers
- Sequential file transfers
- Connection error handling
- Custom port configurations

### 3. CLI Options Tests (`cli_options.bats`)
- Help and version information
- Invalid option handling
- Block size validation
- Metadata and delete flags
- Server mode testing
- Port validation
- Combined options

### 4. Edge Cases Tests (`edge_cases.bats`)
- Large file handling
- Special character filenames
- Binary files with null bytes
- Unicode content
- Deep directory structures
- Many small files
- Minimal/maximum block sizes
- File size changes
- Permission handling
- Symlink behavior
- Concurrent operations

## Helper Functions

The `helpers/test_helpers.bash` file provides a comprehensive set of utility functions:

### Setup and Teardown
- `setup_test_env()` - Initialize test environment
- `cleanup_test_env()` - Clean up after tests
- `setup()` - Global setup for all tests
- `teardown()` - Global teardown for all tests

### File Operations
- `create_test_file(path, content)` - Create test files
- `create_test_structure(base_dir, items...)` - Create directory structures
- `file_exists(path)` - Check file existence
- `get_file_content(path)` - Read file contents
- `files_equal(file1, file2)` - Compare file contents
- `get_file_size(path)` - Get file size

### Command Execution
- `run_rsynx(args...)` - Execute RSynx commands
- `start_server(port, block_size)` - Start RSynx server
- `stop_server()` - Stop RSynx server

### Assertions
- `assert_success()` - Assert command succeeded
- `assert_failure()` - Assert command failed
- `assert_file_exists(path)` - Assert file exists
- `assert_file_not_exists(path)` - Assert file doesn't exist
- `assert_files_equal(file1, file2)` - Assert files are identical
- `assert_output_contains(text)` - Assert output contains text

### Utilities
- `random_string(length)` - Generate random strings
- `generate_content(size, char)` - Generate content of specific size
- `wait_for_port(port, timeout)` - Wait for network port
- `print_test_info()` - Print test information

## Running Tests

### Prerequisites
- BATS testing framework installed
- Rust and Cargo (for building RSynx)
- netcat (for network tests)

### Installation
```bash
# Install BATS
npm install -g bats

# Or on Ubuntu/Debian
sudo apt-get install bats

# Or on macOS
brew install bats-core
```

### Running Tests

#### Using the Test Runner Script
```bash
# Run all tests
./run_tests.sh

# Run only BATS tests
./run_tests.sh --bats-only

# Run with verbose output
./run_tests.sh --bats-only --verbose

# Run specific test file
bats bats-tests/local_sync.bats

# Run specific test
bats bats-tests/local_sync.bats --filter "sync identical files"
```

#### Manual Execution
```bash
# Run all BATS tests
bats bats-tests/

# Run specific test file
bats bats-tests/local_sync.bats

# Run with verbose output
bats bats-tests/local_sync.bats --verbose-run

# Run in parallel
bats bats-tests/ --jobs 4
```

## Test Configuration

### Environment Variables
- `BATS_TMPDIR` - Temporary directory for tests
- `TEST_DIR` - Test-specific temporary directory
- `SRC_DIR` - Source directory for test files
- `DST_DIR` - Destination directory for test files

### Test Isolation
Each test runs in its own temporary directory to ensure isolation:
- Tests create files in `$TEST_DIR/src/` and `$TEST_DIR/dst/`
- All temporary files are cleaned up after each test
- Server processes are properly terminated

## Writing New Tests

### Basic Test Structure
```bash
@test "test description" {
    # Setup
    create_test_file "$SRC_DIR/test.txt" "content"
    
    # Execute
    run_rsynx "$SRC_DIR/test.txt" "$DST_DIR/test.txt"
    
    # Assert
    assert_success
    assert_file_exists "$DST_DIR/test.txt"
    assert_files_equal "$SRC_DIR/test.txt" "$DST_DIR/test.txt"
}
```

### Network Test Structure
```bash
@test "network test description" {
    # Setup
    create_test_file "$SRC_DIR/net_test.txt" "network content"
    
    # Start server
    start_server 7890 1024
    
    # Execute
    run_rsynx --port 7890 "$SRC_DIR/net_test.txt" "127.0.0.1:$DST_DIR/net_test.txt"
    
    # Assert
    assert_success
    assert_file_exists "$DST_DIR/net_test.txt"
    
    # Cleanup
    stop_server
}
```

### Best Practices
1. Use descriptive test names
2. Always clean up resources (servers, files)
3. Use helper functions for common operations
4. Test both success and failure cases
5. Include edge cases and error conditions
6. Use unique ports for network tests
7. Verify both command success and file contents

## Debugging Tests

### Verbose Output
```bash
bats bats-tests/local_sync.bats --verbose-run
```

### Individual Test Execution
```bash
bats bats-tests/local_sync.bats --filter "sync identical files"
```

### Manual Debugging
```bash
# Set up test environment manually
export BATS_TMPDIR=/tmp
export TEST_DIR=/tmp/rsynx_test_debug
mkdir -p $TEST_DIR/src $TEST_DIR/dst

# Run commands manually
echo "test content" > $TEST_DIR/src/test.txt
cargo run -- $TEST_DIR/src/test.txt $TEST_DIR/dst/test.txt
```

## Contributing

When adding new tests:

1. Follow the existing naming conventions
2. Use the provided helper functions
3. Ensure proper cleanup in all code paths
4. Add tests for both success and failure cases
5. Update this README if adding new test categories
6. Test your changes locally before submitting

## Integration with CI/CD

The BATS tests are integrated into the CI/CD pipeline:

- GitHub Actions runs all BATS tests
- Tests run on multiple Rust versions
- Coverage reports include BATS test results
- Performance benchmarks complement the test suite

See `.github/workflows/test.yml` for the complete CI configuration.