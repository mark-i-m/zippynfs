#!/bin/bash

# Exit on certain errors
set -u

# Global constants
readonly DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

trap 'cleanup; exit 1' SIGINT

# Clean up files, etc.
cleanup() {
    if [[ -f ./pid ]]; then
        kill "$(cat ./pid)"
        rm ./pid
    fi
    echo 'Done'
}

main() {
    cd "$DIR"

    # Check arguments
    if (($# != 2)); then
        >&2 echo 'Usage: ./server.sh <SERVER> <DATA_DIR>'
        exit 1
    fi

    # Sanity checks
    if [[ ! -d "$2" ]]; then
        >&2 echo "$2 is not a directory, exiting"
        exit 1
    fi

    # Run server until the first COMMIT message
    echo 'Starting server...'
    cd ../server/
    (RUST_LOG=info cargo run --release --color=never -- -s "$1" -d "$2" 2>&1 > /dev/null & echo $! >&3) 3> ./pid | grep -m1 'Handling COMMMIT ZipCommitArgs' > /dev/null

    echo 'Killing server...'
    kill "$(cat ./pid)"
    rm ./pid

    # Run server again
    echo 'Starting server...'
    cargo run --release --color=never -- -s "$1" -d "$2" 2>&1 > /dev/null
}

main "$@"
