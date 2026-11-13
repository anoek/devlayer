# Common functions for 4* and 8* scripts

function common_init() {
    # Retry getting IP address
    MAX_ATTEMPTS=20
    ATTEMPT=1
    COVERAGE_IP=""

    while [ -z "$COVERAGE_IP" ] && [ $ATTEMPT -le $MAX_ATTEMPTS ]; do
        echo "Attempt $ATTEMPT to get IP address..."
        COVERAGE_IP=$(sudo virsh domifaddr coverage-test | grep ipv4 | awk '{print $4}' | cut -d '/' -f 1)
        
        if [ -z "$COVERAGE_IP" ]; then
            echo "IP not available yet, waiting 1 seconds..."
            sleep 1
            ATTEMPT=$((ATTEMPT + 1))
        fi
    done

    if [ -z "$COVERAGE_IP" ]; then
        echo "Failed to get IP address after $MAX_ATTEMPTS attempts"
        exit 1
    fi

    echo "Successfully got IP address: $COVERAGE_IP"
    
    # Create 4G ram block device for running the coverage tests on different file systems
    # carving out 512M of it for the ~/.devlayer directory
    ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
        if [ ! -b /dev/ram0 ]; then
            sudo modprobe brd rd_nr=1 rd_size=4194304
            sudo parted /dev/ram0 mklabel gpt
            sudo parted /dev/ram0 mkpart primary 0% 12%  # 512M 
            sudo parted /dev/ram0 mkpart primary 12% 100% # 4GB partition
        fi
EOF


    # Ensure our .devlayer and source directories exist
    ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
        mkdir -p /home/devlayer/.devlayer
        mkdir -p /home/devlayer/source
EOF

    echo_ssh_command
}

function echo_ssh_command() {
    echo ""
    echo "## SSH Command for entering manually into the VM"
    echo "ssh -o 'StrictHostKeyChecking no' -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP"
    echo ""
}   


# Create our .devlayer and source directories
function do_test() {
    location=$1

    echo "########################################################"
    echo "Testing at $location" with mount point $mount_point
    echo "########################################################"


    ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
        set -e
        . .cargo/env
        mkdir -p $location
        cd $location
        tar --strip-components=1 -zxf /home/devlayer/devlayer*.crate
        tar -zxf /home/devlayer/vendor.tar.gz
        mkdir -p .cargo
        cp /home/devlayer/cargo_config.toml .cargo/config.toml
        make test
        /home/devlayer/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata merge --output=/home/devlayer/merged-$(date +%Y-%m-%d-%H-%M-%S).profdata coverage/profraw/*.profraw
        cd /home/devlayer/
EOF

}

# Create our .devlayer and source directories
function reinitialize_fs() {
    echo "########################################################"
    echo "$mkfs test"
    echo "########################################################"


    mkfs=$1
    ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
        set -e
        sleep 0.1
        sudo umount -q --lazy /home/devlayer/source || true  
        sudo umount -q --lazy /home/devlayer/.devlayer || true
        sudo umount -q --force /home/devlayer/source || true
        sudo umount -q --force /home/devlayer/.devlayer || true
        sync
        sleep 0.1
        sudo dd if=/dev/zero of=/dev/ram0p1 bs=1M count=32
        sudo dd if=/dev/zero of=/dev/ram0p2 bs=1M count=32
        sudo $mkfs /dev/ram0p1
        sudo $mkfs /dev/ram0p2
        sudo mount /dev/ram0p1 /home/devlayer/.devlayer
        sudo mount /dev/ram0p2 /home/devlayer/source
        sudo chown devlayer:devlayer /home/devlayer/source
        sudo chown devlayer:devlayer /home/devlayer/.devlayer
EOF
}

function reinitialize_as_nfs() {
    ssh -o "StrictHostKeyChecking no" -i generated/id_devlayer_test_vm_key devlayer@$COVERAGE_IP 'bash -s' <<EOF
        set -e
        sudo umount -q --lazy /home/devlayer/source || true  
        sudo umount -q --lazy /home/devlayer/.devlayer || true
        sudo umount -q --force /home/devlayer/source || true
        sudo umount -q --force /home/devlayer/.devlayer || true
        sudo mount -t nfs localhost:/home/devlayer/nfs_source /home/devlayer/source
        sudo mount -t nfs localhost:/home/devlayer/nfs_layers /home/devlayer/.devlayer
        sudo chown devlayer:devlayer /home/devlayer/source
        sudo chown devlayer:devlayer /home/devlayer/.devlayer
EOF
}