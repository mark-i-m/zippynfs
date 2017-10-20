#!/bin/bash

# Exit on certain errors
set -u

# Global constants
readonly DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

trap 'cleanup; exit 1' SIGINT

# Clean up files, etc.
cleanup() {
    fusermount -u "$MOUNT"
    rm -rf "$MOUNT"
    echo 'Done'
}

main() {
    cd "$DIR"

    # Check arguments
    if (($# != 2)); then
        >&2 echo 'Usage: ./client.sh <SERVER> <MOUNT>'
        exit 1
    fi

    # Create a global variable, to help with cleanup
    readonly MOUNT="$2"

    # Sanity checks
    if [[ -e "$MOUNT" ]]; then
        >&2 echo "$MOUNT already exists, exiting"
        exit 1
    fi

    echo "Creating NFS mountpoint..."
    mkdir "$2"

    # Run client
    cd ../client/
    echo 'Starting client...'
    cargo run --bin client_fuse -- -s "$1" -m "$2"

    # We shouldn't reach this point
    cleanup
}

main "$@"
