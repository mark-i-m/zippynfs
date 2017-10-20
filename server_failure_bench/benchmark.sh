#!/bin/bash

# Exit on certain errors
set -u

# Global constants
readonly DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

main() {
    cd "$DIR"

    # Check arguments
    if (($# != 1)); then
        >&2 echo 'Usage: ./benchmark.sh <MOUNT>'
        exit 1
    fi

    # Sanity checks
    if [[ ! -d "$1" ]]; then
        >&2 echo "$1 is not a directory, exiting"
        exit 1
    fi
    mountpoint "$1" > /dev/null 2>&1
    if (($? != 0)); then
        >&2 echo "$1 is not a mountpoint, exiting"
        exit 1
    fi

    echo 'Generating random file...'
    dd if=/dev/urandom of=/tmp/large bs=1M count=10

    echo 'Changing to NFS mountpoint...'
    cd "$1"

    echo 'Copying random file...'
    time cp /tmp/large .

    echo 'Cleaning up...'
    rm large
    rm /tmp/large

    echo 'Done. Kill the server script to run again.'
}

main "$@"
