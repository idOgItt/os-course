#!/bin/bash

set -ex

export TMP=$(mktemp -d)

JOBSERVER=../$(ls --sort=time $(find ../target -name jobserver) | head --lines=1)

(cd env_test && $JOBSERVER 1)

if [[ ! -f $TMP/done ]]; then
    echo "environment is broken"
    exit 1
fi

for i in 1 2 10; do
    export TMP=$(mktemp -d)

    (cd simple_test && $JOBSERVER $i)

    if [[ ! -f $TMP/a ]] || [[ ! -f $TMP/b ]]; then
        echo "some of commands didn't run"
        exit 1
    fi
done

(cd pstree_test && $JOBSERVER 10)

export TMP=$(mktemp -d)

(cd two_dirs_test && $JOBSERVER 20)

if [[ $(uniq $TMP/out | wc -l) == 2 ]]; then
    echo "two recursive make commands should run in parallel"
    exit 1
fi

export TMP=$(mktemp -d)

(cd limit_test && $JOBSERVER 2)

if [[ -f $TMP/limit ]]; then
    echo "concurrent command limit exceeded"
    exit 1
fi

export TMP=$(mktemp -d)

(cd no_repeat_test && $JOBSERVER 5)

a=$(cat $TMP/a)
if [[ $(cat $TMP/a | wc -c) -ne 3 || $a != *"a"* || $a != *"b"* || $a != *"c"* ]]; then
    echo "repeating execs detected"
    exit 1
fi

b=$(cat $TMP/b)
if [[ $(cat $TMP/b | wc -c) -ne 3 || $b != *"a"* || $b != *"b"* || $b != *"c"* ]]; then
    echo "repeating execs detected"
    exit 1
fi

