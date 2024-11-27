#!/usr/bin/env bash

set -euo pipefail

TEST_CLASS=test_skim.TestSkim

cd $(dirname "$0")
tests=$(sed -n 's/^\s\+def \(test_\w\+\)(self.*):\s*$/\1/p' test_skim.py | \
  sk --multi --bind 'ctrl-a:select-all' | xargs -I% echo "$TEST_CLASS.%")

cargo build --release

python3 -m unittest $tests
