#!/bin/bash

# Check if an argument was provided
if [ -z "$1" ]; then
  echo "Error: Benchmark name must be provided as an argument."
  echo "Usage: $0 <benchmark_name>"
  exit 1
fi

# Assign the first argument to the BENCH variable
BENCH="$1"

# Print information about the benchmark being run
echo "Running benchmark: $BENCH"

# Create the profiles directory if it doesn't exist
mkdir -p "./profiles/${BENCH}"
echo "Profile directory: ./profiles/${BENCH}"

# Set the path for the CPU profile
CPU_PROFILE_PATH="./profiles/${BENCH}/bench.cpu.prof"
echo "CPU profile file: ${CPU_PROFILE_PATH}"

# Specify the path to the libprofiler library (may differ on your system)
# Make sure this path is correct for your environment!
PROFILER_LIB="/usr/lib/x86_64-linux-gnu/libprofiler.so"
# Or possibly: /usr/lib/libprofiler.so

# Check if the libprofiler library exists
if [ ! -f "$PROFILER_LIB" ]; then
    echo "Error: libprofiler library not found at ${PROFILER_LIB}"
    echo "CPU profiling might not work. Install 'google-perftools' or 'libgoogle-perftools-dev'."
    exit 1
fi

# Run cargo bench with environment variables for profiling
echo "Running cargo bench..."
echo "env CPUPROFILE=\"${CPU_PROFILE_PATH}\" LD_PRELOAD=\"${PROFILER_LIB}\" cargo bench -p server --bench \"${BENCH}\""
env CPUPROFILE="${CPU_PROFILE_PATH}" LD_PRELOAD="${PROFILER_LIB}" cargo bench -p server --bench "${BENCH}"

# Check the exit code of cargo bench
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ]; then
  echo "Benchmark '${BENCH}' completed successfully."
else
  echo "Error during benchmark '${BENCH}' execution (exit code: $EXIT_CODE)."
  exit $EXIT_CODE
fi

if [ -n "$PROFILER_LIB" ] && [ -f "$PROFILER_LIB" ]; then
    echo "Profiling data saved to ${CPU_PROFILE_PATH}"
fi