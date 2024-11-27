#!/bin/sh

OUT_DIR="/tmp/sk-test-mock"

mkdir -p $OUT_DIR

echo $@ > $OUT_DIR/stdout 2>$OUT_DIR/stderr
