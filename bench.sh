#!/usr/bin/env bash

# Benchmark script to measure ingestion + matching rate in skim interactive mode
# This measures how fast skim can ingest items and display matched results
#
# Usage: bench.sh [BINARY_PATH] [-n|--num-items NUM] [-q|--query QUERY] [-r|--runs RUNS]
#                 [-f|--file FILE] [-g|--generate-file FILE] [-- EXTRA_ARGS...]
#
# Arguments:
#   BINARY_PATH              Path to binary (default: ./target/release/sk)
#   -n, --num-items NUM      Number of items to generate (default: 1000000)
#   -q, --query QUERY        Query string to search (default: "test")
#   -r, --runs RUNS          Number of benchmark runs to average (default: 1)
#   -f, --file FILE          Use existing file as input instead of generating
#   -g, --generate-file FILE Generate test data to file and exit
#   --                       Pass remaining arguments to the binary
#
# Examples:
#   ./bench.sh                                    # Use defaults
#   ./bench.sh ./target/release/sk -n 500000 -q foo
#   ./bench.sh -n 1000000 -q test -- --no-sort --exact
#   ./bench.sh -r 5                               # Run 5 times and show average
#   ./bench.sh -f input.txt -q search             # Use existing file
#   ./bench.sh -g testdata.txt -n 2000000         # Generate file and exit

set -e
export SHELL="/bin/sh"
unset HISTFILE

# Default values
BINARY_PATH="./target/release/sk"
NUM_ITEMS=1000000
QUERY="test"
RUNS=1
INPUT_FILE=""
GENERATE_FILE=""
EXTRA_ARGS=""

# Parse arguments
ARGS=()
FOUND_SEP=0

for arg in "$@"; do
	if [ "$arg" = "--" ]; then
		FOUND_SEP=1
	elif [ $FOUND_SEP -eq 0 ]; then
		ARGS+=("$arg")
	else
		EXTRA_ARGS="$EXTRA_ARGS $arg"
	fi
done

# Parse named arguments
i=0
while [ $i -lt ${#ARGS[@]} ]; do
	arg="${ARGS[$i]}"
	case "$arg" in
	-n | --num-items)
		i=$((i + 1))
		NUM_ITEMS="${ARGS[$i]}"
		;;
	-q | --query)
		i=$((i + 1))
		QUERY="${ARGS[$i]}"
		;;
	-r | --runs)
		i=$((i + 1))
		RUNS="${ARGS[$i]}"
		;;
	-f | --file)
		i=$((i + 1))
		INPUT_FILE="${ARGS[$i]}"
		;;
	-g | --generate-file)
		i=$((i + 1))
		GENERATE_FILE="${ARGS[$i]}"
		;;
	-*)
		echo "Unknown option: $arg" >&2
		exit 1
		;;
	*)
		# First non-option argument is binary path
		BINARY_PATH="$arg"
		;;
	esac
	i=$((i + 1))
done

# Trim leading space from EXTRA_ARGS
EXTRA_ARGS=$(echo "$EXTRA_ARGS" | sed 's/^ *//')

# Validate conflicting options
if [ -n "$INPUT_FILE" ] && [ -n "$GENERATE_FILE" ]; then
	echo "Error: Cannot use both --file and --generate-file" >&2
	exit 1
fi

# Function to generate test data
generate_test_data() {
	local output_file="$1"
	local num_items="$2"

	awk -v num="$num_items" 'BEGIN {
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
    }' >"$output_file"
}

# Handle --generate-file mode
if [ -n "$GENERATE_FILE" ]; then
	echo "Generating $NUM_ITEMS items to $GENERATE_FILE..."
	generate_test_data "$GENERATE_FILE" "$NUM_ITEMS"
	echo "Generated $NUM_ITEMS items successfully"
	exit 0
fi

echo "=== Skim Ingestion + Matching Benchmark ==="
echo "Binary: $BINARY_PATH | Items: $NUM_ITEMS | Query: '$QUERY' | Runs: $RUNS"
[ -n "$INPUT_FILE" ] && echo "Input file: $INPUT_FILE"
[ -n "$EXTRA_ARGS" ] && echo "Extra args: $EXTRA_ARGS"

# Arrays to store results from multiple runs
ELAPSED_TIMES=()
RATES=()
PEAK_MEMS=()
PEAK_CPUS=()
MATCHED_COUNTS=()
COMPLETED_COUNT=0

# Prepare test data file
STATUS_FILE=$(mktemp)
CLEANUP_INPUT=0

if [ -n "$INPUT_FILE" ]; then
	# Use provided input file
	if [ ! -f "$INPUT_FILE" ]; then
		echo "Error: Input file '$INPUT_FILE' not found" >&2
		exit 1
	fi
	TMP_FILE="$INPUT_FILE"
	# Count lines in the file to determine NUM_ITEMS
	NUM_ITEMS=$(wc -l <"$INPUT_FILE")
	echo "Using input file with $NUM_ITEMS items"
else
	# Generate test data to temporary file
	TMP_FILE=$(mktemp)
	CLEANUP_INPUT=1
	echo "Generating test data..."
	generate_test_data "$TMP_FILE" "$NUM_ITEMS"
fi

trap "rm -f $STATUS_FILE; [ $CLEANUP_INPUT -eq 1 ] && rm -f $TMP_FILE || true" EXIT

# Run benchmark multiple times
for RUN in $(seq 1 $RUNS); do
	if [ $RUNS -gt 1 ]; then
		echo ""
		echo "=== Run $RUN/$RUNS ==="
	fi

	SESSION_NAME="skim_bench_$$_$RUN"

	# Create a new tmux session in the background
	tmux new-session -s "$SESSION_NAME" -d

	# Unset HISTFILE in the tmux session to prevent command from appearing in shell history
	tmux send-keys -t "$SESSION_NAME" "unset HISTFILE" Enter
	sleep 0.1

	# Prepare to capture the start time as close to data ingestion as possible
	# Run skim with the query already set, and measure until matcher completes
	tmux send-keys -t "$SESSION_NAME" "cat $TMP_FILE | $BINARY_PATH --query '$QUERY' $EXTRA_ARGS" Enter

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
				echo "$MEM $CPU" >>"$MONITOR_LOG"
				sleep 0.1
			done
			echo "PEAK:$PEAK_MEM:$PEAK_CPU" >>"$MONITOR_LOG"
		) &
		MONITOR_PID=$!
	else
		MONITOR_PID=""
	fi

	# Monitor for matcher completion by checking status line
	# Wait for matched count to stabilize for 10 seconds to ensure matching is truly complete
	COMPLETED=0
	MATCHED_COUNT=0
	TOTAL_INGESTED=0
	PREV_MATCHED_COUNT=-1
	STABLE_START_TIME=0
	REQUIRED_STABLE_DURATION_NS=10000000000 # 10 seconds in nanoseconds
	MAX_WAIT=60
	CHECK_INTERVAL=0.05 # 50ms for <0.05s precision
	ELAPSED_CHECKS=0
	MAX_CHECKS=$((MAX_WAIT * 20)) # 60 seconds * 20 checks per second = 1200 checks
	END=0

	while [ $ELAPSED_CHECKS -lt $MAX_CHECKS ]; do
		sleep $CHECK_INTERVAL
		ELAPSED_CHECKS=$((ELAPSED_CHECKS + 1))

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

				# Check if ingestion is complete
				if [ "$TOTAL_INGESTED" = "$NUM_ITEMS" ]; then
					# Check if matched count has changed
					if [ "$MATCHED_COUNT" != "$PREV_MATCHED_COUNT" ]; then
						# Count changed, reset stability timer and mark end time
						PREV_MATCHED_COUNT=$MATCHED_COUNT
						STABLE_START_TIME=$(date +%s%N)
						END=$STABLE_START_TIME
					else
						# Count is same as before, check if we've been stable long enough
						if [ $STABLE_START_TIME -gt 0 ]; then
							CURRENT_TIME=$(date +%s%N)
							STABLE_NS=$((CURRENT_TIME - STABLE_START_TIME))

							if [ $STABLE_NS -ge $REQUIRED_STABLE_DURATION_NS ]; then
								COMPLETED=1
								break
							fi
						fi
					fi
				fi
			fi
		fi
	done

	# If we didn't capture an end time, set it now
	if [ $END -eq 0 ]; then
		END=$(date +%s%N)
	fi

	# Exit skim
	tmux send-keys -t "$SESSION_NAME" Escape
	sleep 0.1

	# Wait for monitor to finish if it was started
	if [ -n "$MONITOR_PID" ]; then
		wait "$MONITOR_PID" 2>/dev/null || true
	fi

	# Clean up session
	tmux kill-session -t "$SESSION_NAME" 2>/dev/null || true

	ELAPSED_NS=$((END - START))
	ELAPSED_SEC=$(awk "BEGIN {printf \"%.3f\", $ELAPSED_NS / 1000000000}")
	RATE=$(awk "BEGIN {printf \"%.0f\", $NUM_ITEMS / $ELAPSED_SEC}")

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

	# Store results
	ELAPSED_TIMES+=("$ELAPSED_SEC")
	RATES+=("$RATE")
	MATCHED_COUNTS+=("$MATCHED_COUNT")
	PEAK_MEMS+=("$PEAK_MEM")
	PEAK_CPUS+=("$PEAK_CPU")

	if [ $COMPLETED -eq 1 ]; then
		COMPLETED_COUNT=$((COMPLETED_COUNT + 1))
	fi

	# Print individual run results
	if [ $RUNS -gt 1 ]; then
		echo "Status: $(if [ $COMPLETED -eq 1 ]; then echo 'COMPLETED'; else echo 'TIMEOUT'; fi)"
		echo "Items matched: $MATCHED_COUNT / $NUM_ITEMS"
		echo "Total time: ${ELAPSED_SEC}s"
		echo "Items/second: ${RATE}"
		if [ -n "$PEAK_MEM" ] && [ "$PEAK_MEM" -gt 0 ]; then
			echo "Peak memory usage: $((PEAK_MEM / 1024)) MB"
			echo "Peak CPU usage: ${PEAK_CPU}%"
		fi
	fi
done

# Calculate and display average results
echo ""
echo "=== Results ==="

if [ $RUNS -gt 1 ]; then
	echo "Completed runs: $COMPLETED_COUNT / $RUNS"

	# Calculate averages
	AVG_TIME=$(awk -v times="${ELAPSED_TIMES[*]}" 'BEGIN {
        n = split(times, arr, " ")
        sum = 0
        for (i = 1; i <= n; i++) sum += arr[i]
        printf "%.3f", sum / n
    }')

	AVG_RATE=$(awk -v rates="${RATES[*]}" 'BEGIN {
        n = split(rates, arr, " ")
        sum = 0
        for (i = 1; i <= n; i++) sum += arr[i]
        printf "%.0f", sum / n
    }')

	AVG_MATCHED=$(awk -v counts="${MATCHED_COUNTS[*]}" 'BEGIN {
        n = split(counts, arr, " ")
        sum = 0
        for (i = 1; i <= n; i++) sum += arr[i]
        printf "%.0f", sum / n
    }')

	AVG_MEM=$(awk -v mems="${PEAK_MEMS[*]}" 'BEGIN {
        n = split(mems, arr, " ")
        sum = 0
        count = 0
        for (i = 1; i <= n; i++) {
            if (arr[i] > 0) {
                sum += arr[i]
                count++
            }
        }
        if (count > 0) printf "%.0f", sum / count
        else print "0"
    }')

	AVG_CPU=$(awk -v cpus="${PEAK_CPUS[*]}" 'BEGIN {
        n = split(cpus, arr, " ")
        sum = 0
        count = 0
        for (i = 1; i <= n; i++) {
            if (arr[i] > 0) {
                sum += arr[i]
                count++
            }
        }
        if (count > 0) printf "%.1f", sum / count
        else print "0"
    }')

	echo "Average items matched: $AVG_MATCHED / $NUM_ITEMS"
	echo "Average time: ${AVG_TIME}s"
	echo "Average items/second: ${AVG_RATE}"
	if [ "$AVG_MEM" != "0" ] && [ -n "$AVG_MEM" ]; then
		echo "Average peak memory usage: $((AVG_MEM / 1024)) MB"
		echo "Average peak CPU usage: ${AVG_CPU}%"
	fi
else
	# Single run - display results
	echo "Status: $(if [ $COMPLETED_COUNT -eq 1 ]; then echo 'COMPLETED'; else echo 'TIMEOUT'; fi)"
	echo "Items matched: ${MATCHED_COUNTS[0]} / $NUM_ITEMS"
	echo "Total time: ${ELAPSED_TIMES[0]}s"
	echo "Items/second: ${RATES[0]}"
	if [ "${PEAK_MEMS[0]}" != "0" ] && [ -n "${PEAK_MEMS[0]}" ]; then
		echo "Peak memory usage: $((${PEAK_MEMS[0]} / 1024)) MB"
		echo "Peak CPU usage: ${PEAK_CPUS[0]}%"
	fi
fi
