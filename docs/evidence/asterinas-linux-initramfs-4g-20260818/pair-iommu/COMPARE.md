# sqlite 4G: no-IOMMU vs SCHEME=iommu

First pair kept in `pair/`. This round is `pair-iommu/` (Asterinas SCHEME=iommu; Linux still no intel-iommu).
Single run each; host noise is real (Linux also moved with no flag change).

| | Asterinas TOTAL | Linux TOTAL | ratio | Asterinas VACUUM | Linux VACUUM |
|---|---|---|---|---|---|
| no IOMMU (pair/) | 244.649 | 94.04 | 2.60x | 52.15 | 5.687 |
| SCHEME=iommu (pair-iommu/) | 111.956 | 55.302 | 2.02x | 22.3 | 2.82 |

Asterinas TOTAL 244.649 -> 111.956 (2.19x faster).
Linux TOTAL 94.04 -> 55.302 (1.70x faster, same Linux flags).
VACUUM this round: 22.3s vs 2.82s (7.91x).
winner this round=linux
