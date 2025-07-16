use anyhow::Result;
use rsynx::network_sync::NetworkSyncer;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

#[test]
fn test_network_sync_file() -> Result<()> {
    let src_filename = "test_net_sync_file.txt";
    let dst_dir = "test_net_sync_dir";
    let dst_file = format!("{}/{}", dst_dir, src_filename);
    let src_content = b"Hello network sync file !";
    let dst_content = b"Hello world sync file";

    // write src file
    {
        let mut f1 = File::create(src_filename)?;
        f1.write_all(src_content)?;
    }

    let _ = fs::remove_dir_all(dst_dir);
    fs::create_dir(dst_dir)?;
    // write dst file
    {
        let mut f2 = File::create(dst_file.clone())?;
        f2.write_all(dst_content)?;
    }

    // must make sure server and client use the same block size
    let block_size = 4;

    let port = 7878;

    let server_handle = thread::spawn(move || NetworkSyncer::serve_once(port, block_size));

    thread::sleep(Duration::from_millis(100));

    println!("Syncing {} to {}", src_filename, dst_file);
    let client_syncer = NetworkSyncer::new(
        "127.0.0.1".to_string(),
        port,
        src_filename.to_string(),
        dst_file.to_string(),
    )
    .with_block_size(block_size);
    let result = client_syncer.sync()?;
    println!(
        "Client result: new_bytes: {}, reused_bytes: {}",
        result.new_bytes, result.reused_bytes
    );

    let server_result = server_handle.join().expect("Server thread panicked")?;
    println!(
        "Server result: new_bytes: {}, reused_bytes: {}",
        server_result.new_bytes, server_result.reused_bytes
    );

    let mut dst_data = Vec::new();
    let mut f = File::open(dst_file)?;
    f.read_to_end(&mut dst_data)?;
    assert_eq!(dst_data, src_content);

    fs::remove_file(src_filename)?;
    fs::remove_dir_all(dst_dir)?;
    Ok(())
}
