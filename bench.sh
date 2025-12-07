#!/bin/sh

# Performance test script for skim
# Tests CPU, memory usage, and time to completion with large input
#
# Usage: bench.sh <binary path>

set -e
export SHELL="/bin/sh"
unset HISTFILE

echo "=== Skim Performance Test ==="
echo ""

# Test configuration
TEST_SIZE=10000000
QUERY="9999"
MATCHES="27280"

SESSION_NAME="skim-test-$(date +%s)"

trap "tmux kill-session -t $SESSION_NAME" SIGINT
trap "tmux kill-session -t $SESSION_NAME" SIGTSTP
trap "tmux kill-session -t $SESSION_NAME" SIGTERM
trap "tmux kill-session -t $SESSION_NAME" SIGKILL
trap "tmux kill-session -t $SESSION_NAME" EXIT

# Function to test a version
bench() {
    local BINARY_PATH="$1"

    echo ""
    echo "======================================"
    echo "Testing $BINARY_PATH"
    echo "======================================"

    WINDOW_NAME="test-window"

    # Create tmux session
    tmux new-session -s "$SESSION_NAME" -d 2>/dev/null
    tmux new-window -d -P -F '#I' -n "$WINDOW_NAME" -t "$SESSION_NAME" >/dev/null 2>&1
    tmux set-window-option -t "$WINDOW_NAME" pane-base-index 0 >/dev/null 2>&1

    # Start timing
    START_TIME=$(date +%s.%N)

    # Start skim with input
    echo "Starting skim with seq 1 $TEST_SIZE (this will take a while)..."
    tmux send-keys -t "$WINDOW_NAME" "seq 1 $TEST_SIZE | $BINARY_PATH" Enter

    # Wait for skim to start processing and find PID
    SK_PID=""
    for i in 1 2 3 4 5; do
        sleep 0.5
        SK_PID=$(pgrep -lf "$BINARY_PATH" | grep -E "sk|fzf" | head -1 | cut -d' ' -f1)
        if [ -n "$SK_PID" ]; then
            break
        fi
    done

    if [ -z "$SK_PID" ]; then
        echo "ERROR: Could not find skim process after 2.5 seconds"
        tmux kill-session -t "$SESSION_NAME" 2>/dev/null || true
        return 1
    fi

    echo "Found skim process (PID: $SK_PID)"
    STATUS_FILE="/tmp/skim-status-$SK_PID.log"

    # Start background monitoring of resources
    MONITOR_LOG="/tmp/skim-monitor-$SK_PID.log"
    rm $MONITOR_LOG >/dev/null 2>&1 || true
    (
        PEAK_MEM=0
        PEAK_CPU=0
        while kill -0 "$SK_PID" 2>/dev/null; do
            MEM=$(ps -p "$SK_PID" -o rss= 2>/dev/null | tr -d ' ')
            CPU=$(ps -p "$SK_PID" -o %cpu= 2>/dev/null | tr -d ' ')
            if [ -n "$MEM" ] && [ "$MEM" -gt "$PEAK_MEM" ]; then
                PEAK_MEM=$MEM
            fi
            if [ -n "$CPU" ]; then
                CPU_INT=$(echo "$CPU" | cut -d. -f1)
                PEAK_CPU_INT=$(echo "$PEAK_CPU" | cut -d. -f1)
                if [ "$CPU_INT" -gt "$PEAK_CPU_INT" ]; then
                    PEAK_CPU=$CPU
                fi
            fi
            echo "$MEM $CPU" >> "$MONITOR_LOG"
            sleep 0.1
        done
        echo "PEAK:$PEAK_MEM:$PEAK_CPU" >> "$MONITOR_LOG"
    ) &
    MONITOR_PID=$!

    # Type the query character by character
    echo "Typing query: $QUERY"
    QUERY_TYPED_START_TIME=$(date +%s.%N)
    for i in $(seq 1 ${#QUERY}); do
        char=$(echo "$QUERY" | cut -c$i)
        tmux send-keys -t "$WINDOW_NAME" "$char"
        sleep 0.05
    done

    QUERY_TYPED_TIME=$(date +%s.%N)
    TYPING_DURATION=$(awk "BEGIN {printf \"%.3f\", $QUERY_TYPED_TIME - $QUERY_TYPED_START_TIME}")
    echo "Query typed in: ${TYPING_DURATION}s"
    echo "Note: With 10M items, matching may take 30+ seconds..."

    # Wait for matching to complete by checking status line
    echo "Waiting for matching to complete..."
    COMPLETED=0
    MAX_WAIT=60
    ELAPSED=0
    LAST_STATUS=""

    while [ $ELAPSED -lt $MAX_WAIT ]; do
        sleep 1
        ELAPSED=$((ELAPSED + 1))

        # Capture and check status
        tmux capture-pane -b "status-$SESSION_NAME" -t "$WINDOW_NAME.0" 2>/dev/null || true
        tmux save-buffer -b "status-$SESSION_NAME" "$STATUS_FILE" 2>/dev/null || true

        if [ -f "$STATUS_FILE" ]; then
            if grep -q "$MATCHES/$TEST_SIZE" "$STATUS_FILE"; then
              COMPLETED=1
              kill -9 "$SK_PID"
              break;
            fi
        fi
    done
    echo "Matching complete, waiting for monitor thread to finish"
    wait "$MONITOR_PID"

    END_TIME=$(date +%s.%N)
    TOTAL_DURATION=$(awk "BEGIN {printf \"%.3f\", $END_TIME - $START_TIME}")
    MATCH_DURATION=$(awk "BEGIN {printf \"%.3f\", $END_TIME - $QUERY_TYPED_TIME}")

    # Wait for monitor to finish and get peak values

    PEAK_MEM=0
    PEAK_CPU=0
    if [ -f "$MONITOR_LOG" ]; then
        PEAK_LINE=$(grep "^PEAK:" "$MONITOR_LOG")
        if [ -n "$PEAK_LINE" ]; then
            PEAK_MEM=$(echo "$PEAK_LINE" | cut -d: -f2)
            PEAK_CPU=$(echo "$PEAK_LINE" | cut -d: -f3)
        fi
    fi

    # Get final resource usage
    if [ -n "$PEAK_MEM" ] && [ $PEAK_MEM -gt 0 ]; then
        echo ""
        echo "=== Results for $BINARY_PATH ==="
        echo "Status: $(if [ $COMPLETED -eq 1 ]; then echo 'COMPLETED'; else echo 'TIMEOUT'; fi)"
        echo "Time to type query: ${TYPING_DURATION}s"
        echo "Time to complete matching: ${MATCH_DURATION}s"
        echo "Total time: ${TOTAL_DURATION}s"
        echo "Peak memory usage: $((PEAK_MEM / 1024)) MB"
        echo "Peak CPU usage: ${PEAK_CPU}%"
    else
        echo "ERROR: Failed to collect resource metrics"
    fi

    # Capture final screen
    tmux capture-pane -b "final-$SESSION_NAME" -t "$WINDOW_NAME.0" 2>/dev/null || true
    tmux save-buffer -b "final-$SESSION_NAME" "/tmp/skim-final-$SK_PID.txt" 2>/dev/null || true

    # Close skim
    tmux send-keys -t "$WINDOW_NAME" Escape 2>/dev/null || true
    sleep 0.3

    # Clean up
    rm -f "$STATUS_FILE" "$MONITOR_LOG"
}

bench "$1"