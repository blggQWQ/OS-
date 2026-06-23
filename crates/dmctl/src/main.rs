use dm::{
    BlockDevice, DmCryptDevice, DmCryptTable, DmError, DmVerityDevice, FileBlockDevice,
    MemoryBlockDevice, Result,
};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::str::FromStr;
use std::time::Instant;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        print_help();
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let Some(cmd) = args.get(1).map(String::as_str) else {
        print_help();
        return Ok(());
    };

    match cmd {
        "init-image" => {
            require_len(&args, 5)?;
            let image = &args[2];
            let blocks = parse_u64(&args[3])?;
            let block_size = parse_usize(&args[4])?;
            FileBlockDevice::create(image, blocks, block_size)?;
            println!("created image={image} blocks={blocks} block_size={block_size}");
        }
        "crypt-write" => {
            require_len(&args, 7)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let key = args[4].as_bytes();
            let block_id = parse_u64(&args[5])?;
            let data = padded_block(args[6].as_bytes(), block_size);
            let lower = FileBlockDevice::open(image, block_size, true)?;
            let crypt = DmCryptDevice::new(lower, key)?;
            crypt.write_block(block_id, &data)?;
            println!("encrypted write ok: image={image} block={block_id}");
        }
        "crypt-read" => {
            require_len(&args, 6)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let key = args[4].as_bytes();
            let block_id = parse_u64(&args[5])?;
            let lower = FileBlockDevice::open(image, block_size, false)?;
            let crypt = DmCryptDevice::new(lower, key)?;
            let mut out = vec![0; block_size];
            crypt.read_block(block_id, &mut out)?;
            println!("{}", printable_block(&out));
        }
        "raw-read-hex" => {
            require_len(&args, 5)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let block_id = parse_u64(&args[4])?;
            let lower = FileBlockDevice::open(image, block_size, false)?;
            let raw = lower.snapshot_block(block_id)?;
            println!("{}", hex_encode(&raw));
        }
        "crypt-table-fill" => {
            require_len(&args, 8)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let table = DmCryptTable::from_str(&args[4])?;
            let block_id = parse_u64(&args[5])?;
            let byte = parse_hex_byte(&args[6])?;
            let count = parse_usize(&args[7])?;
            let lower = FileBlockDevice::open(image, block_size, true)?;
            let crypt = DmCryptDevice::from_table(lower, &table)?;
            let block = vec![byte; block_size];
            for index in 0..count {
                crypt.write_block(block_id + index as u64, &block)?;
            }
            println!(
                "aes-xts write ok: image={image} first_block={block_id} blocks={count} byte=0x{byte:02x}"
            );
        }
        "verity-root" => {
            require_len(&args, 4)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let lower = FileBlockDevice::open(image, block_size, false)?;
            let verity = DmVerityDevice::build(lower)?;
            println!("{}", hex_encode(&verity.root_hash()));
        }
        "verity-verify" => {
            require_len(&args, 5)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let root_hash = decode_hex_32(&args[4])?;
            let lower = FileBlockDevice::open(image, block_size, false)?;
            let base = DmVerityDevice::build(lower)?;
            let lower = FileBlockDevice::open(image, block_size, false)?;
            let verity = DmVerityDevice::open(lower, base.tree().clone(), root_hash);
            let mut block = vec![0; block_size];
            for block_id in 0..verity.num_blocks() {
                verity.read_block(block_id, &mut block)?;
            }
            println!("verity verification ok: image={image}");
        }
        "verity-format-dm" => {
            require_len(&args, 4)?;
            let image = &args[2];
            let data_blocks = parse_usize(&args[3])?;
            let root_hash = format_dm_verity_image(image, data_blocks)?;
            println!("rootfs_verity.scheme=dm-verity");
            println!("rootfs_verity.data_blocks={data_blocks}");
            println!("rootfs_verity.hash_start={data_blocks}");
            println!("rootfs_verity.hash={}", hex_encode(&root_hash));
        }
        "tamper" => {
            require_len(&args, 7)?;
            let image = &args[2];
            let block_size = parse_usize(&args[3])?;
            let block_id = parse_u64(&args[4])?;
            let offset = parse_usize(&args[5])?;
            let byte = parse_hex_byte(&args[6])?;
            if offset >= block_size {
                return Err(DmError::InvalidBlockSize {
                    expected: block_size,
                    actual: offset,
                });
            }

            let lower = FileBlockDevice::open(image, block_size, true)?;
            let mut block = vec![0; block_size];
            lower.read_block(block_id, &mut block)?;
            block[offset] = byte;
            lower.write_block(block_id, &block)?;
            println!("tampered image={image} block={block_id} offset={offset} byte=0x{byte:02x}");
        }
        "demo-crypt" => demo_crypt()?,
        "demo-verity" => demo_verity()?,
        "benchmark" => {
            let blocks = args.get(2).map(|v| parse_u64(v)).transpose()?.unwrap_or(4096);
            let iterations = args.get(3).map(|v| parse_usize(v)).transpose()?.unwrap_or(3);
            benchmark(blocks, iterations)?;
        }
        _ => print_help(),
    }

    Ok(())
}

fn demo_crypt() -> Result<()> {
    fs::create_dir_all("target")?;
    let image = "target/dmctl-crypt-demo.img";
    FileBlockDevice::create(image, 8, 64)?;
    let lower = FileBlockDevice::open(image, 64, true)?;
    let crypt = DmCryptDevice::new(lower, b"national-final-demo-key")?;
    let plain = padded_block(b"CSCC dm-crypt demo: plaintext visible only above mapper", 64);
    crypt.write_block(2, &plain)?;

    let raw = FileBlockDevice::open(image, 64, false)?.snapshot_block(2)?;
    let mut recovered = vec![0; 64];
    crypt.read_block(2, &mut recovered)?;

    println!("image: {image}");
    println!("plain: {}", printable_block(&plain));
    println!("raw-hex: {}", hex_encode(&raw));
    println!("read: {}", printable_block(&recovered));
    println!("ciphertext_differs: {}", raw != plain);
    Ok(())
}

fn demo_verity() -> Result<()> {
    fs::create_dir_all("target")?;
    let image = "target/dmctl-verity-demo.img";
    FileBlockDevice::create(image, 4, 32)?;
    let lower = FileBlockDevice::open(image, 32, true)?;
    lower.write_block(0, &padded_block(b"verity block zero", 32))?;
    lower.write_block(1, &padded_block(b"verity block one", 32))?;
    lower.write_block(2, &padded_block(b"verity block two", 32))?;
    lower.write_block(3, &padded_block(b"verity block three", 32))?;

    let lower = FileBlockDevice::open(image, 32, false)?;
    let verity = DmVerityDevice::build(lower)?;
    let root = verity.root_hash();
    println!("root-hash: {}", hex_encode(&root));

    let lower = FileBlockDevice::open(image, 32, true)?;
    let mut tampered = lower.snapshot_block(1)?;
    tampered[0] ^= 0xff;
    lower.write_block(1, &tampered)?;

    let lower = FileBlockDevice::open(image, 32, false)?;
    let protected = DmVerityDevice::open(lower, verity.tree().clone(), root);
    let mut out = vec![0; 32];
    match protected.read_block(1, &mut out) {
        Ok(()) => println!("tamper_detected: false"),
        Err(DmError::IntegrityViolation { block_id }) => {
            println!("tamper_detected: true block={block_id}");
        }
        Err(err) => return Err(err),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Usage:
  dmctl init-image <image> <blocks> <block-size>
  dmctl crypt-write <image> <block-size> <key> <block-id> <text>
  dmctl crypt-read <image> <block-size> <key> <block-id>
  dmctl raw-read-hex <image> <block-size> <block-id>
  dmctl crypt-table-fill <image> <block-size> <quoted-table> <block-id> <byte-hex> <count>
  dmctl verity-root <image> <block-size>
  dmctl verity-verify <image> <block-size> <root-hash-hex>
  dmctl verity-format-dm <image> <data-blocks>
  dmctl tamper <image> <block-size> <block-id> <offset> <byte-hex>
  dmctl demo-crypt
  dmctl demo-verity
  dmctl benchmark [blocks] [iterations]"
    );
}

fn benchmark(blocks: u64, iterations: usize) -> Result<()> {
    const BLOCK_SIZE: usize = 4096;
    if blocks == 0 || iterations == 0 {
        return Err(DmError::Io("blocks and iterations must be non-zero".into()));
    }
    let plain = MemoryBlockDevice::new(blocks, BLOCK_SIZE)?;
    let key_hex: String = (0u8..64).map(|byte| format!("{byte:02x}")).collect();
    let table = DmCryptTable::from_str(&format!(
        "aes-xts-plain64 {key_hex} 0 memory 0"
    ))?;
    let crypt = DmCryptDevice::from_table(plain.clone(), &table)?;
    let block = vec![0x5a; BLOCK_SIZE];
    let mut out = vec![0; BLOCK_SIZE];

    println!("implementation,operation,block_size,bytes,seconds,mib_per_second,checksum");
    let (elapsed, checksum) = timed_io(blocks, iterations, || {
        for block_id in 0..blocks {
            plain.write_block(block_id, &block)?;
        }
        Ok(0)
    })?;
    print_bench("rust-plain", "write", blocks, iterations, elapsed, checksum);

    let (elapsed, checksum) = timed_io(blocks, iterations, || {
        let mut sum = 0u64;
        for block_id in 0..blocks {
            plain.read_block(block_id, &mut out)?;
            sum = sum.wrapping_add(out[0] as u64);
        }
        Ok(sum)
    })?;
    print_bench("rust-plain", "read", blocks, iterations, elapsed, checksum);

    let (elapsed, checksum) = timed_io(blocks, iterations, || {
        for block_id in 0..blocks {
            crypt.write_block(block_id, &block)?;
        }
        Ok(0)
    })?;
    print_bench("rust-aes-xts", "write", blocks, iterations, elapsed, checksum);

    let (elapsed, checksum) = timed_io(blocks, iterations, || {
        let mut sum = 0u64;
        for block_id in 0..blocks {
            crypt.read_block(block_id, &mut out)?;
            sum = sum.wrapping_add(out[0] as u64);
        }
        Ok(sum)
    })?;
    print_bench("rust-aes-xts", "read", blocks, iterations, elapsed, checksum);

    for block_id in 0..blocks {
        plain.write_block(block_id, &block)?;
    }
    let verity = DmVerityDevice::build(plain)?;
    let (elapsed, checksum) = timed_io(blocks, iterations, || {
        let mut sum = 0u64;
        for block_id in 0..blocks {
            verity.read_block(block_id, &mut out)?;
            sum = sum.wrapping_add(out[0] as u64);
        }
        Ok(sum)
    })?;
    print_bench("rust-verity", "read", blocks, iterations, elapsed, checksum);
    Ok(())
}

fn timed_io<F>(blocks: u64, iterations: usize, mut operation: F) -> Result<(f64, u64)>
where
    F: FnMut() -> Result<u64>,
{
    let start = Instant::now();
    let mut checksum = 0u64;
    for _ in 0..iterations {
        checksum = checksum.wrapping_add(operation()?);
    }
    let elapsed = start.elapsed().as_secs_f64();
    if elapsed == 0.0 || blocks == 0 {
        return Err(DmError::Io("benchmark timer resolution too low".into()));
    }
    Ok((elapsed, checksum))
}

fn print_bench(
    implementation: &str,
    operation: &str,
    blocks: u64,
    iterations: usize,
    seconds: f64,
    checksum: u64,
) {
    const BLOCK_SIZE: usize = 4096;
    let bytes = blocks as f64 * iterations as f64 * BLOCK_SIZE as f64;
    let mib_per_second = bytes / (1024.0 * 1024.0) / seconds;
    println!(
        "{implementation},{operation},{BLOCK_SIZE},{},{seconds:.6},{mib_per_second:.2},{checksum}",
        bytes as u64
    );
}

fn require_len(args: &[String], len: usize) -> Result<()> {
    if args.len() != len {
        return Err(DmError::Io(format!("expected {len} arguments, got {}", args.len())));
    }
    Ok(())
}

fn parse_u64(value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| DmError::Io(format!("invalid integer: {value}")))
}

fn parse_usize(value: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| DmError::Io(format!("invalid integer: {value}")))
}

fn padded_block(input: &[u8], block_size: usize) -> Vec<u8> {
    let mut out = vec![0; block_size];
    let len = input.len().min(block_size);
    out[..len].copy_from_slice(&input[..len]);
    out
}

fn printable_block(input: &[u8]) -> String {
    let end = input.iter().position(|byte| *byte == 0).unwrap_or(input.len());
    String::from_utf8_lossy(&input[..end]).into_owned()
}

fn hex_encode(input: &[u8]) -> String {
    input.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex_32(input: &str) -> Result<[u8; 32]> {
    let bytes = decode_hex(input)?;
    if bytes.len() != 32 {
        return Err(DmError::Io(format!(
            "root hash must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut out = [0; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_hex(input: &str) -> Result<Vec<u8>> {
    if input.len() % 2 != 0 {
        return Err(DmError::Io("hex string must have even length".to_string()));
    }

    let mut out = Vec::with_capacity(input.len() / 2);
    for idx in (0..input.len()).step_by(2) {
        let byte = u8::from_str_radix(&input[idx..idx + 2], 16)
            .map_err(|_| DmError::Io(format!("invalid hex byte: {}", &input[idx..idx + 2])))?;
        out.push(byte);
    }
    Ok(out)
}

fn parse_hex_byte(input: &str) -> Result<u8> {
    let value = input.strip_prefix("0x").unwrap_or(input);
    u8::from_str_radix(value, 16).map_err(|_| DmError::Io(format!("invalid hex byte: {input}")))
}

fn format_dm_verity_image(path: &str, data_blocks: usize) -> Result<[u8; 32]> {
    const BLOCK_SIZE: usize = 4096;
    const HASH_SIZE: usize = 32;
    const HASHES_PER_BLOCK: usize = BLOCK_SIZE / HASH_SIZE;

    if data_blocks == 0 {
        return Err(DmError::EmptyDevice);
    }

    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let data_size = data_blocks
        .checked_mul(BLOCK_SIZE)
        .ok_or_else(|| DmError::Io("verity data size overflow".to_string()))?;
    if file.metadata()?.len() < data_size as u64 {
        return Err(DmError::InvalidImageSize {
            image_size: file.metadata()?.len(),
            block_size: data_size,
        });
    }

    let mut digests = Vec::with_capacity(data_blocks);
    let mut block = vec![0u8; BLOCK_SIZE];
    for block_id in 0..data_blocks {
        file.seek(SeekFrom::Start((block_id * BLOCK_SIZE) as u64))?;
        file.read_exact(&mut block)?;
        digests.push(dm::sha256::sha256(&block));
    }

    let mut output_block_id = data_blocks;
    loop {
        let mut next_level = Vec::with_capacity(digests.len().div_ceil(HASHES_PER_BLOCK));
        for digest_group in digests.chunks(HASHES_PER_BLOCK) {
            block.fill(0);
            for (slot, digest) in digest_group.iter().enumerate() {
                let offset = slot * HASH_SIZE;
                block[offset..offset + HASH_SIZE].copy_from_slice(digest);
            }
            file.seek(SeekFrom::Start((output_block_id * BLOCK_SIZE) as u64))?;
            file.write_all(&block)?;
            output_block_id += 1;
            next_level.push(dm::sha256::sha256(&block));
        }
        if next_level.len() == 1 {
            file.set_len((output_block_id * BLOCK_SIZE) as u64)?;
            file.flush()?;
            return Ok(next_level[0]);
        }
        digests = next_level;
    }
}
