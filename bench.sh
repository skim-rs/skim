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

# Send all non-final output to stderr. Save original stdout on fd 3 so we can
# restore it later for the final results which should go to stdout.
exec 3>&1 1>&2
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
JSON=0

# Print unified JSON result. Expects the aggregate variables to be set:
# AVG_MATCHED, MIN_MATCHED, MAX_MATCHED,
# AVG_TIME, MIN_TIME, MAX_TIME,
# AVG_RATE, MIN_RATE, MAX_RATE,
# AVG_MEM, MIN_MEM, MAX_MEM (use string "null" when not measured)
# AVG_CPU, MIN_CPU, MAX_CPU (use string "null" when not measured)
print_json() {
	printf '{'
	printf '"num_items":%s,' "$NUM_ITEMS"
	printf '"runs":%s,' "$RUNS"
	printf '"completed_runs":%s,' "$COMPLETED_COUNT"
	printf '"items_matched":{"avg":%s,"min":%s,"max":%s},' "$AVG_MATCHED" "$MIN_MATCHED" "$MAX_MATCHED"
	printf '"time_s":{"avg":%s,"min":%s,"max":%s},' "$AVG_TIME" "$MIN_TIME" "$MAX_TIME"
	printf '"items_per_second":{"avg":%s,"min":%s,"max":%s},' "$AVG_RATE" "$MIN_RATE" "$MAX_RATE"

	if [ "$AVG_MEM" = "null" ] || [ "$MIN_MEM" = "null" ] || [ "$MAX_MEM" = "null" ]; then
		printf '"peak_memory_kb":{"avg":null,"min":null,"max":null},'
	else
		printf '"peak_memory_kb":{"avg":%s,"min":%s,"max":%s},' "$AVG_MEM" "$MIN_MEM" "$MAX_MEM"
	fi

	if [ "$AVG_CPU" = "null" ] || [ "$MIN_CPU" = "null" ] || [ "$MAX_CPU" = "null" ]; then
		printf '"peak_cpu":{"avg":null,"min":null,"max":null}'
	else
		printf '"peak_cpu":{"avg":%s,"min":%s,"max":%s}' "$AVG_CPU" "$MIN_CPU" "$MAX_CPU"
	fi

	printf '}\n'
}

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
	-j | --json)
		JSON=1
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
	tmux send-keys -t "$SESSION_NAME" "unset FZF_DEFAULT_OPTS" Enter
	tmux send-keys -t "$SESSION_NAME" "unset SKIM_DEFAULT_OPTIONS" Enter
	sleep 0.1

	# Prepare to capture the start time as close to data ingestion as possible
	# Run skim with the query already set, and measure until matcher completes
	tmux send-keys -t "$SESSION_NAME" "cat $TMP_FILE | $BINARY_PATH --query '$QUERY' $EXTRA_ARGS" Enter

	# Record start time
	START=$(date +%s%N)

	# Find skim PID for resource monitoring.
	# We combine -P (parent = tmux pane shell) with -f (full cmdline contains
	# BINARY_PATH) so that transient children like direnv that run before sk
	# are ignored.  Using -P avoids the self-match problem of plain pgrep -f
	# (where the pgrep invocation's own argv would contain BINARY_PATH).
	TMUX_PANE_PID=$(tmux list-panes -t "$SESSION_NAME" -F '#{pane_pid}' 2>/dev/null || echo "")
	SK_PID=""
	MONITOR_LOG=""
	if [ -n "$TMUX_PANE_PID" ]; then
		for i in $(seq 1 50); do
			sleep 0.1
			SK_PID=$(pgrep -P "$TMUX_PANE_PID" -f "$BINARY_PATH" 2>/dev/null | head -1)
			if [ -n "$SK_PID" ]; then
				break
			fi
		done
	fi

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
				sleep 0.05
			done
			echo "PEAK:$PEAK_MEM:$PEAK_CPU" >>"$MONITOR_LOG"
		) &
		MONITOR_PID=$!
	else
		MONITOR_PID=""
	fi

	# Monitor for matcher completion by checking the tmux status line.
	# We consider matching done when:
	#   (a) total ingested == NUM_ITEMS, AND
	#   (b) the matched count has been stable for REQUIRED_STABLE_DURATION_NS.
	# We also bail out early if skim has exited (tmux pane gone / SK_PID dead).
	COMPLETED=0
	MATCHED_COUNT=0
	TOTAL_INGESTED=0
	PREV_MATCHED_COUNT=-1
	STABLE_START_TIME=0
	REQUIRED_STABLE_DURATION_NS=5000000000 # 5 seconds in nanoseconds
	MAX_WAIT_NS=$((60 * 1000000000))       # 60-second hard timeout
	CHECK_INTERVAL=0.05                    # 50 ms between checks
	END=0
	LOOP_START=$(date +%s%N)

	while true; do
		sleep $CHECK_INTERVAL

		# Hard timeout: give up after MAX_WAIT_NS regardless
		NOW=$(date +%s%N)
		if [ $((NOW - LOOP_START)) -ge $MAX_WAIT_NS ]; then
			break
		fi

		# Early-exit: if the skim process has exited, stop waiting.
		# We only break on a dead PID — the pane-child fallback is removed
		# because transient pre-sk processes (e.g. direnv) would trigger it
		# falsely before sk has even launched.
		if [ -n "$SK_PID" ] && ! kill -0 "$SK_PID" 2>/dev/null; then
			break
		fi

		# Capture and check status using bench.sh's method
		tmux capture-pane -b "status-$SESSION_NAME" -t "$SESSION_NAME" 2>/dev/null || true
		tmux save-buffer -b "status-$SESSION_NAME" "$STATUS_FILE" 2>/dev/null || true

		if [ -f "$STATUS_FILE" ]; then
			# Skim status line format is typically: "  > query  matched/total"
			# The first number is matched items, second is total ingested items
			STATUS_LINE=$(grep -oE '[0-9]+/[0-9]+' "$STATUS_FILE" 2>/dev/null | head -1 || echo "")
			if [ -n "$STATUS_LINE" ]; then
				MATCHED_COUNT=$(echo "$STATUS_LINE" | cut -d'/' -f1)
				TOTAL_INGESTED=$(echo "$STATUS_LINE" | cut -d'/' -f2)

				# Check if ingestion is complete
				if [ "$TOTAL_INGESTED" = "$NUM_ITEMS" ]; then
					if [ "$MATCHED_COUNT" != "$PREV_MATCHED_COUNT" ]; then
						# Count changed: reset stability timer and record candidate end time
						PREV_MATCHED_COUNT=$MATCHED_COUNT
						STABLE_START_TIME=$(date +%s%N)
						END=$STABLE_START_TIME
					elif [ $STABLE_START_TIME -gt 0 ]; then
						# Count unchanged: check if stable long enough
						CURRENT_TIME=$(date +%s%N)
						if [ $((CURRENT_TIME - STABLE_START_TIME)) -ge $REQUIRED_STABLE_DURATION_NS ]; then
							COMPLETED=1
							break
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

	# Extract peak CPU and RAM usage.
	# Use empty string as sentinel for "not measured" so that averaging logic
	# can skip these runs rather than treating 0 as a valid sample.
	PEAK_MEM=""
	PEAK_CPU=""
	if [ -n "$MONITOR_PID" ] && [ -n "$MONITOR_LOG" ] && [ -f "$MONITOR_LOG" ]; then
		PEAK_LINE=$(grep "^PEAK:" "$MONITOR_LOG" 2>/dev/null || echo "")
		if [ -n "$PEAK_LINE" ]; then
			PEAK_MEM=$(echo "$PEAK_LINE" | cut -d: -f2)
			PEAK_CPU=$(echo "$PEAK_LINE" | cut -d: -f3)
			# Treat 0 as "not measured" (monitor never sampled anything meaningful)
			[ "$PEAK_MEM" = "0" ] && PEAK_MEM=""
			[ "$PEAK_CPU" = "0" ] && PEAK_CPU=""
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
		if [ -n "$PEAK_MEM" ]; then
			echo "Peak memory usage: $((PEAK_MEM / 1024)) MB"
		fi
		if [ -n "$PEAK_CPU" ]; then
			echo "Peak CPU usage: ${PEAK_CPU}%"
		fi
	fi
done

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
    if (arr[i] != "" && arr[i] + 0 > 0) {
        sum += arr[i]
        count++
    }
}
if (count > 0) printf "%.0f", sum / count
else print ""
}')

AVG_CPU=$(awk -v cpus="${PEAK_CPUS[*]}" 'BEGIN {
n = split(cpus, arr, " ")
sum = 0
count = 0
for (i = 1; i <= n; i++) {
    if (arr[i] != "" && arr[i] + 0 > 0) {
        sum += arr[i]
        count++
    }
}
if (count > 0) printf "%.1f", sum / count
else print ""
}')

# Calculate min/max for several metrics so we can show them alongside averages
read -r MIN_TIME MAX_TIME <<<"$(awk -v times="${ELAPSED_TIMES[*]}" 'BEGIN { n=split(times,a," "); min=a[1]; max=a[1]; for(i=1;i<=n;i++){ if(a[i]<min) min=a[i]; if(a[i]>max) max=a[i]; } printf "%.3f %.3f", min, max }')"
read -r MIN_RATE MAX_RATE <<<"$(awk -v rates="${RATES[*]}" 'BEGIN { n=split(rates,a," "); min=a[1]; max=a[1]; for(i=1;i<=n;i++){ if(a[i]<min) min=a[i]; if(a[i]>max) max=a[i]; } printf "%.0f %.0f", min, max }')"
read -r MIN_MATCHED MAX_MATCHED <<<"$(awk -v counts="${MATCHED_COUNTS[*]}" 'BEGIN { n=split(counts,a," "); min=a[1]; max=a[1]; for(i=1;i<=n;i++){ if(a[i]<min) min=a[i]; if(a[i]>max) max=a[i]; } printf "%.0f %.0f", min, max }')"

# For memory and CPU, ignore empty/zero entries (meaning not measured).
# Output is empty string when no run was measured, so that downstream null
# checks work correctly.
read -r MIN_MEM MAX_MEM <<<"$(awk -v mems="${PEAK_MEMS[*]}" 'BEGIN { n=split(mems,a," "); min=1e18; max=0; found=0; for(i=1;i<=n;i++){ if(a[i] != "" && a[i]+0 > 0){ if(a[i]<min) min=a[i]; if(a[i]>max) max=a[i]; found=1 } } if(found) printf "%.0f %.0f", min, max; else print "" }')"
read -r MIN_CPU MAX_CPU <<<"$(awk -v cpus="${PEAK_CPUS[*]}" 'BEGIN { n=split(cpus,a," "); min=1e18; max=0; found=0; for(i=1;i<=n;i++){ if(a[i] != "" && a[i]+0 > 0){ if(a[i]<min) min=a[i]; if(a[i]>max) max=a[i]; found=1 } } if(found) printf "%.1f %.1f", min, max; else print "" }')"

# Restore stdout for final results and display them on stdout
echo ""
exec 1>&3 3>&-

# If JSON output requested, emit a single-line JSON object and exit
if [ "$JSON" -eq 1 ]; then
	# Ensure numeric defaults
	AVG_MEM=${AVG_MEM:-"null"}
	MIN_MEM=${MIN_MEM:-"null"}
	MAX_MEM=${MAX_MEM:-"null"}
	AVG_CPU=${AVG_CPU:-"null"}
	MIN_CPU=${MIN_CPU:-"null"}
	MAX_CPU=${MAX_CPU:-"null"}
	printf '{'
	printf '"num_items":%s,' "$NUM_ITEMS"
	printf '"runs":%s,' "$RUNS"
	printf '"completed_runs":%s,' "$COMPLETED_COUNT"
	printf '"items_matched":{"avg":%s,"min":%s,"max":%s},' "$AVG_MATCHED" "$MIN_MATCHED" "$MAX_MATCHED"
	printf '"time_s":{"avg":%s,"min":%s,"max":%s},' "$AVG_TIME" "$MIN_TIME" "$MAX_TIME"
	printf '"items_per_second":{"avg":%s,"min":%s,"max":%s},' "$AVG_RATE" "$MIN_RATE" "$MAX_RATE"
	printf '"peak_memory_kb":{"avg":%s,"min":%s,"max":%s},' "$AVG_MEM" "$MIN_MEM" "$MAX_MEM"
	printf '"peak_cpu":{"avg":%s,"min":%s,"max":%s}' "$AVG_CPU" "$MIN_CPU" "$MAX_CPU"
	printf '}\n'
	exit 0
else
	echo "=== Results ==="

	echo "Average items matched: $AVG_MATCHED / $NUM_ITEMS (min: $MIN_MATCHED, max: $MAX_MATCHED)"
	echo "Average time: ${AVG_TIME}s (min: ${MIN_TIME}s, max: ${MAX_TIME}s)"
	echo "Average items/second: ${AVG_RATE} (min: ${MIN_RATE}, max: ${MAX_RATE})"
	if [ -n "$AVG_MEM" ]; then
		echo "Average peak memory usage: $((AVG_MEM / 1024)) MB (min: $((MIN_MEM / 1024)) MB, max: $((MAX_MEM / 1024)) MB)"
	fi
	if [ -n "$AVG_CPU" ]; then
		echo "Average peak CPU usage: ${AVG_CPU}% (min: ${MIN_CPU}%, max: ${MAX_CPU}%)"
	fi
fi
