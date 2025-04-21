#!/bin/bash

# --- Argument Check ---
# Check if exactly three arguments were provided
if [ "$#" -ne 3 ]; then
  echo "Error: Incorrect number of arguments."
  echo "Usage: $0 \"<server_command>\" \"<tests_command>\" \"<log_directory>\""
  echo "  - Commands must be enclosed in quotes."
  echo "  - Log directory will be created if it doesn't exist."
  # Exit with an error code
  exit 1
fi

# Command line arguments:
SERVER="$1" # Command for background
TESTS="$2" # Command for foreground
LOG_DIR="$3"  # Directory for log files

# --- Directory Handling ---
# Check if the log directory exists and is a directory
if [ -e "$LOG_DIR" ] && [ ! -d "$LOG_DIR" ]; then
  echo "Error: '$LOG_DIR' exists but is not a directory."
  exit 1
fi

# If the directory doesn't exist, try to create it
if [ ! -d "$LOG_DIR" ]; then
  echo "Log directory '$LOG_DIR' not found. Attempting to create it..."
  # Try to create the directory, including parent directories (-p)
  mkdir -p "$LOG_DIR"
  if [ $? -ne 0 ]; then
    echo "Error: Failed to create log directory '$LOG_DIR'. Please check permissions."
    exit 1
  else
    echo "Log directory '$LOG_DIR' created successfully."
  fi
else
  echo "Using existing log directory: '$LOG_DIR'"
fi

# Define log file paths
# Note: These files will be overwritten on each run (> and 2>).
# Use '>>' and '2>>' respectively if you want to append to the logs.
SERVER_OUT="$LOG_DIR/server.stdout.log"
SERVER_ERR="$LOG_DIR/server.stderr.log"
TESTS_OUT="$LOG_DIR/tests.stdout.log"
TESTS_ERR="$LOG_DIR/tests.stderr.log"

# --- SCRIPT ---

echo "Starting the server in the background: $SERVER"
$SERVER > "$SERVER_OUT" 2> "$SERVER_ERR" &
# Get the PID of the last background process
PID1=$!

# Check if we got a PID and the process actually started
sleep 0.1
if [ -z "$PID1" ] || ! ps -p $PID1 > /dev/null; then
  echo "Error: Failed to start the server."
  wait $PID1 2>/dev/null # Wait for it to clean up if it was ever really there
  echo "Check logs for potential errors:"
  echo "  stdout: $SERVER_OUT"
  echo "  stderr: $SERVER_ERR"
  exit 1
fi

echo "The server started with PID: $PID1"

echo "Starting tests: $TESTS"
$TESTS > "$TESTS_OUT" 2> "$TESTS_ERR"
EXIT_CODE2=$? # Save the exit code of the second command

echo "The tests finished with exit code: $EXIT_CODE2"
echo "  Check logs for details:"
echo "    stdout: $TESTS_OUT"
echo "    stderr: $TESTS_ERR"


# Check if the process with PID1 still exists before sending the signal
# This prevents error output from kill if the process already finished on its own
if ps -p $PID1 > /dev/null; then
  echo "Sending SIGTERM signal to the server ($PID1)..."
  kill -s TERM $PID1
  KILL_EXIT_CODE=$?

  if [ $KILL_EXIT_CODE -eq 0 ]; then
    # Give the process a moment to terminate gracefully after SIGTERM
    echo "Waiting briefly for process $PID1 to terminate..."
    sleep 1

    # Check again if the process has terminated
    if ps -p $PID1 > /dev/null; then
      echo "Warning: the server ($PID1) is still active."
    else
      echo "The server successfully terminated."
    fi
  else
    if ps -p $PID1 > /dev/null; then
       echo "Error: Failed to send SIGTERM signal to process $PID1 (perhaps insufficient permissions?). Exit code from kill: $KILL_EXIT_CODE. Process $PID1 is still running."
    else
       echo "Warning: Failed to send SIGTERM signal (Exit code $KILL_EXIT_CODE), but process $PID1 seems to have terminated anyway."
    fi
  fi
else
  echo "Process with PID $PID1 no longer exists by the time the signal was to be sent (it likely finished on its own)."
  echo "  Check its logs for details:"
  echo "    stdout: $SERVER_OUT"
  echo "    stderr: $SERVER_ERR"
fi

# Return the exit code of the second command as the script's exit code
exit $EXIT_CODE2