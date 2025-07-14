#!/bin/bash

# RSynx Test Runner Script
# Comprehensive test suite runner for RSynx project

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATS_DIR="$SCRIPT_DIR/bats-tests"
COVERAGE_DIR="$SCRIPT_DIR/coverage"
TEST_LOG="$SCRIPT_DIR/test_results.log"

# Default options
RUN_UNIT_TESTS=true
RUN_INTEGRATION_TESTS=true
RUN_BATS_TESTS=true
VERBOSE=false
PARALLEL=false
BAIL_ON_FAIL=false
GENERATE_COVERAGE=false
CLEAN_BEFORE=false

# Help function
show_help() {
    cat << EOF
RSynx Test Runner

Usage: $0 [OPTIONS]

Options:
    -h, --help              Show this help message
    -v, --verbose           Verbose output
    -p, --parallel          Run tests in parallel
    -b, --bail             Stop on first failure
    -c, --coverage         Generate coverage report
    --clean                Clean before running tests
    --unit-only            Run only unit tests
    --integration-only     Run only integration tests
    --bats-only            Run only bats tests
    --no-unit              Skip unit tests
    --no-integration       Skip integration tests
    --no-bats              Skip bats tests

Examples:
    $0                      Run all tests
    $0 --unit-only          Run only Rust unit tests
    $0 --bats-only          Run only bats shell tests
    $0 -v -c               Run all tests with verbose output and coverage
    $0 --parallel --bail    Run tests in parallel, stop on first failure

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -p|--parallel)
            PARALLEL=true
            shift
            ;;
        -b|--bail)
            BAIL_ON_FAIL=true
            shift
            ;;
        -c|--coverage)
            GENERATE_COVERAGE=true
            shift
            ;;
        --clean)
            CLEAN_BEFORE=true
            shift
            ;;
        --unit-only)
            RUN_UNIT_TESTS=true
            RUN_INTEGRATION_TESTS=false
            RUN_BATS_TESTS=false
            shift
            ;;
        --integration-only)
            RUN_UNIT_TESTS=false
            RUN_INTEGRATION_TESTS=true
            RUN_BATS_TESTS=false
            shift
            ;;
        --bats-only)
            RUN_UNIT_TESTS=false
            RUN_INTEGRATION_TESTS=false
            RUN_BATS_TESTS=true
            shift
            ;;
        --no-unit)
            RUN_UNIT_TESTS=false
            shift
            ;;
        --no-integration)
            RUN_INTEGRATION_TESTS=false
            shift
            ;;
        --no-bats)
            RUN_BATS_TESTS=false
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            show_help
            exit 1
            ;;
    esac
done

# Logging function
log() {
    local level="$1"
    local message="$2"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    case "$level" in
        "INFO")
            echo -e "${GREEN}[INFO]${NC} $message"
            ;;
        "WARN")
            echo -e "${YELLOW}[WARN]${NC} $message"
            ;;
        "ERROR")
            echo -e "${RED}[ERROR]${NC} $message"
            ;;
        "DEBUG")
            if [[ "$VERBOSE" == "true" ]]; then
                echo -e "${BLUE}[DEBUG]${NC} $message"
            fi
            ;;
    esac
    
    echo "[$timestamp] [$level] $message" >> "$TEST_LOG"
}

# Check prerequisites
check_prerequisites() {
    log "INFO" "Checking prerequisites..."
    
    # Check if cargo is available
    if ! command -v cargo &> /dev/null; then
        log "ERROR" "cargo is not installed or not in PATH"
        exit 1
    fi
    
    # Check if bats is available (only if we're running bats tests)
    if [[ "$RUN_BATS_TESTS" == "true" ]]; then
        if ! command -v bats &> /dev/null; then
            log "ERROR" "bats is not installed or not in PATH"
            log "INFO" "Install bats: npm install -g bats"
            exit 1
        fi
        log "DEBUG" "bats version: $(bats --version)"
    fi
    
    # Check if nc is available (for network tests)
    if ! command -v nc &> /dev/null; then
        log "WARN" "nc (netcat) is not available - some network tests may fail"
    fi
    
    log "INFO" "Prerequisites check passed"
}

# Clean function
clean_artifacts() {
    log "INFO" "Cleaning test artifacts..."
    
    # Clean Rust artifacts
    if [[ -d "target" ]]; then
        cargo clean
    fi
    
    # Clean coverage artifacts
    if [[ -d "$COVERAGE_DIR" ]]; then
        rm -rf "$COVERAGE_DIR"
    fi
    
    # Clean test logs
    if [[ -f "$TEST_LOG" ]]; then
        rm -f "$TEST_LOG"
    fi
    
    # Clean temporary bats files
    find /tmp -name "rsynx_test_*" -type d -exec rm -rf {} + 2>/dev/null || true
    
    log "INFO" "Clean completed"
}

# Build the project
build_project() {
    log "INFO" "Building project..."
    
    local build_args=""
    if [[ "$VERBOSE" == "true" ]]; then
        build_args="--verbose"
    fi
    
    if ! cargo build $build_args; then
        log "ERROR" "Build failed"
        exit 1
    fi
    
    log "INFO" "Build completed"
}

# Run Rust unit tests
run_unit_tests() {
    log "INFO" "Running Rust unit tests..."
    
    local test_args=""
    if [[ "$VERBOSE" == "true" ]]; then
        test_args="--verbose"
    fi
    
    if [[ "$PARALLEL" == "true" ]]; then
        test_args="$test_args --jobs $(nproc)"
    fi
    
    if [[ "$BAIL_ON_FAIL" == "true" ]]; then
        test_args="$test_args --no-fail-fast"
    fi
    
    if ! cargo test --lib $test_args; then
        log "ERROR" "Unit tests failed"
        if [[ "$BAIL_ON_FAIL" == "true" ]]; then
            exit 1
        fi
        return 1
    fi
    
    log "INFO" "Unit tests passed"
    return 0
}

# Run Rust integration tests
run_integration_tests() {
    log "INFO" "Running Rust integration tests..."
    
    local test_args=""
    if [[ "$VERBOSE" == "true" ]]; then
        test_args="--verbose"
    fi
    
    if [[ "$PARALLEL" == "true" ]]; then
        test_args="$test_args --jobs $(nproc)"
    fi
    
    if ! cargo test --test '*' $test_args; then
        log "ERROR" "Integration tests failed"
        if [[ "$BAIL_ON_FAIL" == "true" ]]; then
            exit 1
        fi
        return 1
    fi
    
    log "INFO" "Integration tests passed"
    return 0
}

# Run bats tests
run_bats_tests() {
    log "INFO" "Running bats shell tests..."
    
    if [[ ! -d "$BATS_DIR" ]]; then
        log "ERROR" "Bats test directory not found: $BATS_DIR"
        return 1
    fi
    
    local bats_args=""
    if [[ "$VERBOSE" == "true" ]]; then
        bats_args="--verbose-run"
    fi
    
    if [[ "$PARALLEL" == "true" ]]; then
        bats_args="$bats_args --jobs $(nproc)"
    fi
    
    local failed_tests=0
    
    # Run each bats test file
    for test_file in "$BATS_DIR"/*.bats; do
        if [[ -f "$test_file" ]]; then
            local test_name=$(basename "$test_file" .bats)
            log "DEBUG" "Running bats test: $test_name"
            
            if ! bats $bats_args "$test_file"; then
                log "ERROR" "Bats test failed: $test_name"
                failed_tests=$((failed_tests + 1))
                
                if [[ "$BAIL_ON_FAIL" == "true" ]]; then
                    exit 1
                fi
            else
                log "DEBUG" "Bats test passed: $test_name"
            fi
        fi
    done
    
    if [[ $failed_tests -gt 0 ]]; then
        log "ERROR" "Some bats tests failed ($failed_tests failures)"
        return 1
    fi
    
    log "INFO" "All bats tests passed"
    return 0
}

# Generate coverage report
generate_coverage() {
    log "INFO" "Generating coverage report..."
    
    # Check if tarpaulin is available
    if ! command -v cargo-tarpaulin &> /dev/null; then
        log "WARN" "cargo-tarpaulin not found, installing..."
        if ! cargo install cargo-tarpaulin; then
            log "ERROR" "Failed to install cargo-tarpaulin"
            return 1
        fi
    fi
    
    mkdir -p "$COVERAGE_DIR"
    
    if ! cargo tarpaulin --out Html --output-dir "$COVERAGE_DIR"; then
        log "ERROR" "Coverage generation failed"
        return 1
    fi
    
    log "INFO" "Coverage report generated in: $COVERAGE_DIR"
    return 0
}

# Main execution
main() {
    echo -e "${BLUE}RSynx Test Runner${NC}"
    echo "===================="
    
    # Initialize log file
    echo "Test run started at $(date)" > "$TEST_LOG"
    
    # Clean if requested
    if [[ "$CLEAN_BEFORE" == "true" ]]; then
        clean_artifacts
    fi
    
    # Check prerequisites
    check_prerequisites
    
    # Build project
    build_project
    
    # Track test results
    local total_tests=0
    local passed_tests=0
    local failed_tests=0
    
    # Run unit tests
    if [[ "$RUN_UNIT_TESTS" == "true" ]]; then
        total_tests=$((total_tests + 1))
        if run_unit_tests; then
            passed_tests=$((passed_tests + 1))
        else
            failed_tests=$((failed_tests + 1))
        fi
    fi
    
    # Run integration tests
    if [[ "$RUN_INTEGRATION_TESTS" == "true" ]]; then
        total_tests=$((total_tests + 1))
        if run_integration_tests; then
            passed_tests=$((passed_tests + 1))
        else
            failed_tests=$((failed_tests + 1))
        fi
    fi
    
    # Run bats tests
    if [[ "$RUN_BATS_TESTS" == "true" ]]; then
        total_tests=$((total_tests + 1))
        if run_bats_tests; then
            passed_tests=$((passed_tests + 1))
        else
            failed_tests=$((failed_tests + 1))
        fi
    fi
    
    # Generate coverage if requested
    if [[ "$GENERATE_COVERAGE" == "true" ]]; then
        generate_coverage
    fi
    
    # Print summary
    echo
    echo "===================="
    echo -e "${BLUE}Test Summary${NC}"
    echo "===================="
    echo "Total test suites: $total_tests"
    echo -e "Passed: ${GREEN}$passed_tests${NC}"
    echo -e "Failed: ${RED}$failed_tests${NC}"
    
    # Exit with appropriate code
    if [[ $failed_tests -gt 0 ]]; then
        log "ERROR" "Some tests failed"
        exit 1
    else
        log "INFO" "All tests passed!"
        exit 0
    fi
}

# Run main function
main "$@"