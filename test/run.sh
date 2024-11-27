#!/usr/bin/env bash

set -euo pipefail

TEST_CLASS=test_skim.TestSkim

cd $(dirname "$0")
tests=$(sed -n 's/^\s\+def \(test_\w\+\)(self.*):\s*$/\1/p' test_skim.py | \
  sk --multi)

cargo build --release

for test in $tests; do
  python3 -m unittest $TEST_CLASS.$test
done
