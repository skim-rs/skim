#!/usr/bin/env bash

# Benchmark script to measure ingestion + matching rate in skim interactive mode
# This measures how fast skim can ingest items and display matched results

set -e

# Parse arguments
BINARY_PATH=${1:-"./target/release/sk"}
NUM_ITEMS=${2:-1000000}
QUERY=${3:-"test"}

echo "=== Skim Ingestion + Matching Benchmark (Interactive Mode) ==="
echo "Binary: $BINARY_PATH"
echo "Items: $NUM_ITEMS"
echo "Query: '$QUERY'"
echo ""

# Generate test data
echo "Generating $NUM_ITEMS test items..."
TMP_FILE=$(mktemp)
STATUS_FILE=$(mktemp)
SESSION_NAME="skim_bench_$$"
trap "rm -f $TMP_FILE $STATUS_FILE; tmux kill-session -t $SESSION_NAME 2>/dev/null || true" EXIT

# Generate random path-like strings with 2-10 words separated by slashes
awk -v num="$NUM_ITEMS" 'BEGIN {
    srand()
    words[1]="home"; words[2]="usr"; words[3]="etc"; words[4]="var"; words[5]="opt"
    words[6]="tmp"; words[7]="dev"; words[8]="proc"; words[9]="sys"; words[10]="lib"
    words[11]="bin"; words[12]="sbin"; words[13]="boot"; words[14]="mnt"; words[15]="media"
    words[16]="src"; words[17]="test"; words[18]="config"; words[19]="data"; words[20]="logs"
    words[21]="cache"; words[22]="backup"; words[23]="docs"; words[24]="images"; words[25]="videos"
    words[26]="audio"; words[27]="downloads"; words[28]="uploads"; words[29]="temp"; words[30]="shared"
    
    for (i = 1; i <= num; i++) {
        depth = int(rand() * 9) + 2  # 2-10 depth
        path = ""
        for (j = 1; j <= depth; j++) {
            word_idx = int(rand() * 30) + 1
            path = path words[word_idx]
            if (j < depth) path = path "/"
        }
        print path "_" i
    }
}' > "$TMP_FILE"

echo "Running skim benchmark in tmux..."
echo ""

# Create a new tmux session in the background
tmux new-session -s "$SESSION_NAME" -d

# Prepare to capture the start time as close to data ingestion as possible
# Run skim with the query already set, and measure until matcher completes
tmux send-keys -t "$SESSION_NAME" "cat $TMP_FILE | $BINARY_PATH --query '$QUERY'" Enter

# Record start time
START=$(date +%s%N)

# Wait a bit for skim to actually start
sleep 0.2

# Find skim PID for resource monitoring
SK_PID=""
for i in 1 2 3 4 5; do
    sleep 0.5
    SK_PID=$(pgrep -lf "$BINARY_PATH" | grep -E "sk|fzf" | head -1 | cut -d' ' -f1)
    if [ -n "$SK_PID" ]; then
        break
    fi
done

if [ -n "$SK_PID" ]; then
    echo "Found skim process (PID: $SK_PID) - monitoring resources..."

    # Start background monitoring of CPU and RAM
    MONITOR_LOG="/tmp/skim-monitor-$SK_PID.log"
    rm -f "$MONITOR_LOG"
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
else
    echo "Warning: Could not find skim process - resource monitoring disabled"
    MONITOR_PID=""
fi

# Monitor for matcher completion by checking status line
echo "Waiting for matching to complete..."
COMPLETED=0
MATCHED_COUNT=0
TOTAL_INGESTED=0
MAX_WAIT=60
ELAPSED=0

while [ $ELAPSED -lt $MAX_WAIT ]; do
    sleep 1
    ELAPSED=$((ELAPSED + 1))

    # Capture and check status using bench.sh's method
    tmux capture-pane -b "status-$SESSION_NAME" -t "$SESSION_NAME" 2>/dev/null || true
    tmux save-buffer -b "status-$SESSION_NAME" "$STATUS_FILE" 2>/dev/null || true

    if [ -f "$STATUS_FILE" ]; then
        # Skim status line format is typically: "  > query  matched/total"
        # We need to find the last occurrence of the pattern matched/total
        # The first number is matched items, second is total ingested items
        STATUS_LINE=$(grep -oE '[0-9]+/[0-9]+' "$STATUS_FILE" 2>/dev/null | head -1 || echo "")
        if [ -n "$STATUS_LINE" ]; then
            MATCHED_COUNT=$(echo "$STATUS_LINE" | cut -d'/' -f1)
            TOTAL_INGESTED=$(echo "$STATUS_LINE" | cut -d'/' -f2)
            
            # Print progress every few seconds
            if [ $((ELAPSED % 5)) -eq 0 ]; then
                echo "Progress: $MATCHED_COUNT matched, $TOTAL_INGESTED ingested (target: $NUM_ITEMS)"
            fi
            
            # Check if ingestion is complete
            if [ "$TOTAL_INGESTED" = "$NUM_ITEMS" ]; then
                COMPLETED=1
                echo "Ingestion complete: $TOTAL_INGESTED items"
                break
            fi
        fi
    fi
done

END=$(date +%s%N)

# Exit skim
tmux send-keys -t "$SESSION_NAME" Escape
sleep 0.1

# Wait for monitor to finish if it was started
if [ -n "$MONITOR_PID" ]; then
    wait "$MONITOR_PID" 2>/dev/null || true
fi

# Clean up
tmux kill-session -t "$SESSION_NAME" 2>/dev/null || true

ELAPSED_NS=$((END - START))
ELAPSED=$(awk "BEGIN {printf \"%.3f\", $ELAPSED_NS / 1000000000}")
RATE=$(awk "BEGIN {printf \"%.0f\", $NUM_ITEMS / $ELAPSED}")

# Extract peak CPU and RAM usage
PEAK_MEM=0
PEAK_CPU=0
if [ -n "$MONITOR_PID" ] && [ -f "$MONITOR_LOG" ]; then
    PEAK_LINE=$(grep "^PEAK:" "$MONITOR_LOG" 2>/dev/null || echo "")
    if [ -n "$PEAK_LINE" ]; then
        PEAK_MEM=$(echo "$PEAK_LINE" | cut -d: -f2)
        PEAK_CPU=$(echo "$PEAK_LINE" | cut -d: -f3)
    fi
    rm -f "$MONITOR_LOG"
fi

echo "=== Results ==="
echo "Status: $(if [ $COMPLETED -eq 1 ]; then echo 'COMPLETED'; else echo 'TIMEOUT'; fi)"
echo "Items matched: $MATCHED_COUNT / $NUM_ITEMS"
echo "Total time: ${ELAPSED}s"
echo "Items/second: ${RATE}"
if [ -n "$PEAK_MEM" ] && [ "$PEAK_MEM" -gt 0 ]; then
    echo "Peak memory usage: $((PEAK_MEM / 1024)) MB"
    echo "Peak CPU usage: ${PEAK_CPU}%"
fi
echo ""
echo "To test with different parameters:"
echo "  $0 <binary_path> <num_items> <query>"
echo "Example: $0 ./target/release/sk 500000 pattern"