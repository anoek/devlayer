#!/usr/bin/env bash

set -e
. common-functions.sh

common_init

reinitialize_fs mkfs.ext4
do_test /home/devlayer/source/utf8/こんにちは

reinitialize_fs mkfs.ext4
do_test /home/devlayer/source/utf8/😀

reinitialize_fs mkfs.ext4
do_test /home/devlayer/source/one_dir_deep

# We test /home/devlayer/source in the filesystem test a lot, no need to test it here


### At this point coverage tests should be at 100% so lets bail early if they aren't
# ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
#     set -e
#     . .cargo/env
#     cd /home/devlayer/source/one_dir_deep
#     mkdir -p coverage/profdata
#     cp /home/devlayer/merged-*.profdata /home/devlayer/source/coverage/profdata/
#     make coverage-check
# EOF

