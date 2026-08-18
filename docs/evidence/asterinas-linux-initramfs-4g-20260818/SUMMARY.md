# 4G Asterinas vs Linux initramfs Filebench / SQLite

Not official 8G README numbers. Same initramfs job on both guests.
MEM=4G SMP=1 KVM Icelake-Server, no mlock. nfiles=10000 nthreads=10 filesize=131072 runtime=60s.

## sqlite-speedtest1 --size 1000 /ext2 (lower TOTAL wins)

```
asterinas-kvm,ok,244.649
linux-qemu-initramfs,ok,94.040
winner=linux aster=244.649 linux=94.04
```

## Filebench fileserver ext2 (higher ops/s wins)

```
asterinas-kvm,ok,8398.724,201.9
linux-qemu-initramfs,ok,11545.120,278.5
winner=linux aster=8398.724 linux=11545.12
```

Raw CSVs under `pair/`. Official 8G README numbers were not changed.
