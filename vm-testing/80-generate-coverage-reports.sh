#!/usr/bin/env bash

set -e
. common-functions.sh

common_init


reinitialize_fs mkfs.btrfs
do_test /home/devlayer/source/

## Generate the coverage report
ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
    set -e
    . .cargo/env
    cd /home/devlayer/source/
    mkdir -p coverage/profdata
    cp /home/devlayer/merged-*.profdata /home/devlayer/source/coverage/profdata/
    rm -Rf /home/devlayer/coverage/profraw
    make update-coverage-report
EOF

scp -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key -rp devlayer@$COVERAGE_IP:/home/devlayer/source/coverage/html coverage_html

