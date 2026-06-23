//! Boot argument parsing skeleton for dm-verity / dm-crypt.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmBootConfig {
    pub crypt_device: Option<String>,
    pub crypt_key: Option<String>,
    pub verity_enabled: bool,
    pub verity_root_hash: Option<[u8; 32]>,
}

pub fn parse_dm_boot_config(cmdline: &str) -> DmBootConfig {
    let mut config = DmBootConfig {
        crypt_device: None,
        crypt_key: None,
        verity_enabled: false,
        verity_root_hash: None,
    };

    for item in cmdline.split_whitespace() {
        if let Some(value) = item.strip_prefix("dm_crypt.device=") {
            config.crypt_device = Some(value.to_string());
        } else if let Some(value) = item.strip_prefix("dm_crypt.key=") {
            config.crypt_key = Some(value.to_string());
        } else if item == "rootfs_verity.scheme=dm-verity" {
            config.verity_enabled = true;
        } else if let Some(value) = item.strip_prefix("rootfs_verity.hash=") {
            config.verity_root_hash = decode_hex_32(value);
        }
    }

    config
}

fn decode_hex_32(input: &str) -> Option<[u8; 32]> {
    if input.len() != 64 {
        return None;
    }

    let mut out = [0; 32];
    for idx in 0..32 {
        let start = idx * 2;
        out[idx] = u8::from_str_radix(&input[start..start + 2], 16).ok()?;
    }
    Some(out)
}

