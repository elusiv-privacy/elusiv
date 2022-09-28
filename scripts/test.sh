#!/bin/bash

if [ "$1" = "--integration" ]; then
make test TEST_MANIFEST=elusiv TEST_METHOD=test-bpf TEST_KIND=integration
else
make test TEST_MANIFEST=elusiv TEST_METHOD=test TEST_KIND=unit
fi