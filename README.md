# File Synchronizer

A Rust implementation of an efficient file synchronization tool using the rsync algorithm. This tool minimizes data transfer by only sending the differences between source and destination files.

## Usage

```bash
cargo run -- <source_path> <destination_path>
```

```bash
cargo run -- --help
```

### How It Works

The synchronization process works by:

1. Calculating checksums for blocks in the destination file
2. Rolling through the source file to find matching blocks
3. Efficiently transferring only the changed portions
4. Using both weak (rolling) and strong (SHA-256) checksums for accuracy

## Performance

The tool uses a combination of techniques to optimize performance:
- Memory mapping for efficient file I/O
- Rolling checksum algorithm for quick block matching
- Block-level deduplication to minimize data transfer