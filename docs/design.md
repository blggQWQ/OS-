# Design

## Goal

Provide a compact Rust Device Mapper model that can be moved into Asterinas after the block-device path is available.

## Core Interface

`BlockDevice` is the only required abstraction:

```rust
fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
```

Every mapper wraps a lower device and implements the same trait:

```text
application / filesystem
        -> DmCryptDevice or DmVerityDevice
        -> BlockDevice implementation
        -> virtio-blk or disk image
```

## Modules

- `block_device`: trait plus `MemoryBlockDevice` for tests.
- `device_mapper`: passthrough wrapper used to validate the forwarding path.
- `dm_crypt`: transparent block encryption wrapper.
- `dm_verity`: read-only integrity wrapper.
- `hash_tree`: Merkle tree construction and verification.
- `sha256`: dependency-free SHA-256 implementation for hashing and demo keystream generation.

## dm-crypt Flow

Write:

```text
plaintext block -> per-block keystream -> ciphertext -> lower device
```

Read:

```text
ciphertext block -> same per-block keystream -> plaintext -> caller
```

The demo cipher is intentionally simple and dependency-free. For a competition submission integrated into Asterinas, replace it with AES-XTS or AES-CBC-ESSIV from a vetted crypto provider.

## dm-verity Flow

1. Read all data blocks from the lower read-only image.
2. Build leaf hashes with SHA-256.
3. Build parent levels by hashing adjacent child hashes.
4. Store the root hash as the trusted value.
5. On read, verify the block against the tree and root hash before returning it.

## Asterinas Wiring

The OS-specific work should be isolated to one adapter:

```rust
struct VirtioBlkAdapter {
    inner: AsterinasVirtioBlockHandle,
}

impl BlockDevice for VirtioBlkAdapter {
    // forward to the real Asterinas block I/O API
}
```

Once this adapter works, the mapper code can wrap it without changing the crypto or verity logic.

