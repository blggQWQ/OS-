# Optional webserver-style read-mostly workload.

set $dir=/mnt/cscc-fb
set $nfiles=1000
set $meandirwidth=20
set $filesize=cvar(type=cvar-gamma,parameters=mean:16384;gamma:1.5)
set $nthreads=100
set $iosize=1m
set $runtime=60

define fileset name=bigfileset,path=$dir,size=$filesize,entries=$nfiles,dirwidth=$meandirwidth,prealloc=100
define fileset name=logfiles,path=$dir,size=$filesize,entries=1,dirwidth=$meandirwidth,prealloc=100

define process name=filereader,instances=1
{
  thread name=filereaderthread,memsize=10m,instances=$nthreads
  {
    flowop openfile name=openfile1,filesetname=bigfileset,fd=1
    flowop readwholefile name=readfile1,fd=1,iosize=$iosize
    flowop closefile name=closefile1,fd=1
    flowop openfile name=openfile2,filesetname=bigfileset,fd=1
    flowop readwholefile name=readfile2,fd=1,iosize=$iosize
    flowop closefile name=closefile2,fd=1
    flowop openfile name=openfile3,filesetname=logfiles,fd=1
    flowop appendfilerand name=appendlog,iosize=16k,fd=1
    flowop closefile name=closefile3,fd=1
  }
}

echo "CSCC webserver workload starting"
run $runtime
