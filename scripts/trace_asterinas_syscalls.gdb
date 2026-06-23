set pagination off
set confirm off
target remote :1234
set $syscall_count = 0
hbreak *0xffffffff886aecc6
commands
  silent
  set $syscall_count = $syscall_count + 1
  printf "SYSCALL[%d] nr=%lu user_rip=%p arg0=%#lx arg1=%#lx arg2=%#lx\n", $syscall_count, user_ctx.user_context.general.rax, user_ctx.user_context.general.rip, user_ctx.user_context.general.rdi, user_ctx.user_context.general.rsi, user_ctx.user_context.general.rdx
  if $syscall_count >= 30
    detach
    quit
  end
  continue
end
continue
