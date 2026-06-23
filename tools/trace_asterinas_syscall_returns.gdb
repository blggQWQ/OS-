set pagination off
set confirm off
set breakpoint pending on
set $calls = 0

target remote :1234
hbreak *0xffffffff886aeccb
commands
  silent
  set $calls = $calls + 1
  printf "ENTER %u nr=%llu a0=%#llx a1=%#llx a2=%#llx a3=%#llx a4=%#llx a5=%#llx\n", $calls, *(unsigned long long *)($rsp+0x40), *(unsigned long long *)($rsp+0x48), *(unsigned long long *)($rsp+0x50), *(unsigned long long *)($rsp+0x58), *(unsigned long long *)($rsp+0x60), *(unsigned long long *)($rsp+0x68), *(unsigned long long *)($rsp+0x70)
  continue
end

hbreak *0xffffffff886aed3a
commands
  silent
  printf "RETURN %u result_tag=%#llx value=%#llx error=%#llx\n", $calls, *(unsigned long long *)($rsp+0x88), *(unsigned long long *)($rsp+0x78), *(unsigned long long *)($rsp+0x80)
  if $calls >= 80
    detach
    quit
  end
  continue
end

continue
