// SPDX-License-Identifier: MPL-2.0

//! Small AES-NI primitives for kernel users that provide an FPU-state guard.
//!
//! Keeping the vector instructions behind this scalar API avoids enabling
//! SIMD code generation for an entire freestanding kernel crate.

use core::arch::{asm, x86_64::__cpuid};

/// Returns whether the current CPU exposes the AES instruction-set extension.
pub fn is_available() -> bool {
    // SAFETY: CPUID leaf 1 is available on every supported x86_64 CPU.  Newer
    // toolchains expose this intrinsic as safe, so tolerate an unused block.
    #[allow(unused_unsafe)]
    unsafe {
        __cpuid(1).ecx & (1 << 25) != 0
    }
}

/// Expands an AES-256 key into fifteen encryption round keys.
///
/// The caller must verify [`is_available`] and preserve the current FPU/XMM
/// context while calling this function.
pub fn expand_key_256(key: &[u8; 32], round_keys: &mut [[u8; 16]; 15]) {
    // SAFETY: All memory operands point to arrays of the exact required size.
    // The caller contract guarantees AES-NI and exclusive temporary XMM use.
    unsafe {
        asm!(
            "movdqu xmm1, [{key}]",
            "movdqu xmm3, [{key} + 16]",
            "movdqu [{out}], xmm1",
            "movdqu [{out} + 16], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x01",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 32], xmm1",
            "aeskeygenassist xmm2, xmm1, 0x00", "pshufd xmm2, xmm2, 0xaa",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "pxor xmm3, xmm2", "movdqu [{out} + 48], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x02",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 64], xmm1",
            "aeskeygenassist xmm2, xmm1, 0x00", "pshufd xmm2, xmm2, 0xaa",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "pxor xmm3, xmm2", "movdqu [{out} + 80], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x04",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 96], xmm1",
            "aeskeygenassist xmm2, xmm1, 0x00", "pshufd xmm2, xmm2, 0xaa",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "pxor xmm3, xmm2", "movdqu [{out} + 112], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x08",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 128], xmm1",
            "aeskeygenassist xmm2, xmm1, 0x00", "pshufd xmm2, xmm2, 0xaa",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "pxor xmm3, xmm2", "movdqu [{out} + 144], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x10",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 160], xmm1",
            "aeskeygenassist xmm2, xmm1, 0x00", "pshufd xmm2, xmm2, 0xaa",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "pxor xmm3, xmm2", "movdqu [{out} + 176], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x20",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 192], xmm1",
            "aeskeygenassist xmm2, xmm1, 0x00", "pshufd xmm2, xmm2, 0xaa",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "movdqa xmm4, xmm3", "pslldq xmm4, 4", "pxor xmm3, xmm4",
            "pxor xmm3, xmm2", "movdqu [{out} + 208], xmm3",

            "aeskeygenassist xmm2, xmm3, 0x40",
            "pshufd xmm2, xmm2, 0xff",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "movdqa xmm4, xmm1", "pslldq xmm4, 4", "pxor xmm1, xmm4",
            "pxor xmm1, xmm2", "movdqu [{out} + 224], xmm1",
            key = in(reg) key.as_ptr(),
            out = in(reg) round_keys.as_mut_ptr(),
            out("xmm1") _, out("xmm2") _, out("xmm3") _, out("xmm4") _,
            options(nostack),
        );
    }
}

/// Encrypts one block with expanded AES-256 encryption round keys.
pub fn encrypt_block_256(block: &mut [u8; 16], keys: &[[u8; 16]; 15]) {
    // SAFETY: The fixed offsets remain within `block` and `keys`; see module contract.
    unsafe {
        asm!(
            "movdqu xmm0, [{block}]",
            "movdqu xmm1, [{keys}]", "pxor xmm0, xmm1",
            "movdqu xmm1, [{keys} + 16]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 32]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 48]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 64]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 80]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 96]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 112]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 128]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 144]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 160]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 176]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 192]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 208]", "aesenc xmm0, xmm1",
            "movdqu xmm1, [{keys} + 224]", "aesenclast xmm0, xmm1",
            "movdqu [{block}], xmm0",
            block = in(reg) block.as_mut_ptr(), keys = in(reg) keys.as_ptr(),
            out("xmm0") _, out("xmm1") _, options(nostack),
        );
    }
}

/// Decrypts one block with expanded AES-256 encryption round keys.
pub fn decrypt_block_256(block: &mut [u8; 16], keys: &[[u8; 16]; 15]) {
    // SAFETY: The fixed offsets remain within `block` and `keys`; see module contract.
    unsafe {
        asm!(
            "movdqu xmm0, [{block}]",
            "movdqu xmm1, [{keys} + 224]", "pxor xmm0, xmm1",
            "movdqu xmm1, [{keys} + 208]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 192]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 176]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 160]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 144]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 128]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 112]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 96]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 80]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 64]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 48]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 32]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys} + 16]", "aesimc xmm1, xmm1", "aesdec xmm0, xmm1",
            "movdqu xmm1, [{keys}]", "aesdeclast xmm0, xmm1",
            "movdqu [{block}], xmm0",
            block = in(reg) block.as_mut_ptr(), keys = in(reg) keys.as_ptr(),
            out("xmm0") _, out("xmm1") _, options(nostack),
        );
    }
}
