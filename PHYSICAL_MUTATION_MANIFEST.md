# Physical Mutation Manifest

Baseline image: `target/verity-rootfs.img`; block size: 4096; original root hash: `b1a6dcd4e458855146a62f4f01ee9d628d809b6245e99285d0822bb0ab427d17`.

| ID | Region | Physical block | Byte offset | Original block SHA-256 | Corrupt block SHA-256 |
|---|---|---:|---:|---|---|
| V03 | first data | 0 | 17 | `d6b378de2781b946233e5e434f56cbeae6be6aed87605aee9edb37117c36f27d` | `b31dc2a37cc0c3c164617ca51af9cef0e49101f8e7a9f79c721db1a2dba2c2ef` |
| V04 | middle data | 32768 | 134217745 | `05725f10112c3e92b99465d39f44e1844acd25a6fc696d45b01de40a541899bb` | `1e12a90f8a205a6a50f321bcbfd96ea4c4036aecc33a786d5c21f9346ca5056e` |
| V05 | last data | 65535 | 268431377 | `ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7` | `dbeaf0b712cd66aa6890897d48822a833d3de2df287adf7e1ac8f57528854dc0` |
| V06 | first leaf hash | 65536 | 268435473 | `04efd25795f03305a8c39354f74a71848f5e158d6a96faa195381c3144fbb7ce` | `5026ae43d7fd0385cc29fbcc0b31af35017a5371c5b657fd47266acc9243efdb` |
| V07 | first intermediate hash | 66048 | 270532625 | `c0ab5f9f0d9c5b530c293b3c36a7063be2cd590577199b4bd6636673836c85f4` | `557feb6640edc47dce22eb28e19c447871730350a85e77c2faaba4f5f112e921` |

每项只翻转目标块内偏移 17 的一个字节，测试始终使用原始正确 root hash。
