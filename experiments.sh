#!/bin/bash

cargo build -p server --release
cargo build -p replayer --release

cpu_command() {
  if [ "$#" -lt 1 ]; then
    echo "Ошибка: Не указан путь к файлу CPU профиля." >&2
    return 1
  fi

  local cpu_profile_path="$1"
  shift

  local cli_args="$@"

  local ld_preload='LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libprofiler.so'
  local executable='./target/release/server'
  local config_arg='-c ./configs/server.toml'

  local cpu_profile="CPUPROFILE=$cpu_profile_path"
  local base_command="env $ld_preload $cpu_profile $executable $config_arg"

  if [ -n "$cli_args" ]; then
    echo "$base_command $cli_args"
  else
    echo "$base_command"
  fi
}

heap_command() {
  if [ "$#" -lt 1 ]; then
    echo "Ошибка: Не указан путь к файлу Heap профиля." >&2
    return 1
  fi

  local heap_profile_path="$1"
  shift

  local cli_args="$@"

  local ld_preload='LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libtcmalloc_and_profiler.so'
  local executable='./target/release/server'
  local config_arg='-c ./configs/server.toml'

  local heap_profile="HEAPPROFILE=$heap_profile_path"
  local base_command="env $ld_preload $heap_profile $executable $config_arg"

  if [ -n "$cli_args" ]; then
    echo "$base_command $cli_args"
  else
    echo "$base_command"
  fi
}

base_path="./tmp"

exp='map'
for cache in "map:0KB" "map:100KB" "map:200KB"
do
    for buffer in 0 10000 100000 200000
    do
        for (( i=0; i<10; i++ ))
        do
            dir="$base_path/$exp/$cache-$buffer/iter$i/cpu"
            cpu_cmd=$(cpu_command "$dir/server.cpu.prof" -b "$buffer" "$cache")
            bash ./run_experiment.sh "$cpu_cmd" "task replayer" "$dir"
            dir="$base_path/$exp/$cache-$buffer/iter$i/heap"
            heap_cmd=$(heap_command "$dir/server.heap" -b "$buffer" "$cache")
            bash ./run_experiment.sh "$heap_cmd" "task replayer" "$dir"
        done
    done
done

exp='list'
for cache in "list:copy" "list:non-copy"
do
    for buffer in 0 10000 100000 200000
    do
        for (( i=0; i<10; i++ ))
        do
            dir="$base_path/$exp/$cache-$buffer/iter$i/cpu"
            cpu_cmd=$(cpu_command "$dir/server.cpu.prof" -b "$buffer" "$cache")
            bash ./run_experiment.sh "$cpu_cmd" "task replayer" "$dir"
            dir="$base_path/$exp/$cache-$buffer/iter$i/heap"
            heap_cmd=$(heap_command "$dir/server.heap" -b "$buffer" "$cache")
            bash ./run_experiment.sh "$heap_cmd" "task replayer" "$dir"
        done
    done
done