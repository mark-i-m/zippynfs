ZippyNFS
--------

A single-machine NFS implementation with a FUSE client.

## Semantics

When we say "atomic" we mean that it is not possible to observe a state in
which the operation has started and not yet completed, even in the presence of
concurrent operations and crashes.

### "Guarantees"

- Renaming a file is atomic.
- Small writes are atomic (<= 4000B).
- We guarantee nothing else about concurrent writes, including about their durability.

### Assumptions

- Renames on the underlying filesystem on the server are atomic.
- The underlying filesystem can accept writes to files even if they move during the write.

### Performance Assumptions/Usage Cases

TODO

## Design

A bit of terminology:

- By _NFS file_ or the _NFS_ we are referring to the filesystem as the client
  observes it (i.e. the logical heirarchy of the NFS filesystem).
- By _server file_ or _server filesystem_ we are referring the filesystem as
  the server actually stores it (i.e. the physical representation of the NFS).

The server stores data in the underlying filesystem on its host machine (e.g.
ext4). We tried to make minimal assumptions about the semantics we assume for
the underlying filesystem.

Each file has a unique 64-bit _File ID_ (FID), which is never reused. The root
has FID=1. Each NFS file has two correspond server files: one containing the
data and one for metadata. The server FS has the same heirarchy as the NFS, but
with the addition of all of the metadata files. Moreover, the names of server
files are based on the FID of the corresponding NFS file, so that files'
identities are not tied to a particular location. For example, the following
heirarchies represent a server FS and its corresponding NFS:

```
# Server FS

data_dir
├── 1                               // Root dir
│   ├── 4                           // Data for file "/baz.txt"
│   ├── 4.baz.txt                   // Metadata for "/baz.txt"
│   ├── 5                           // Directory "/bazee"
│   ├── 5.bazee                     // Metadata for "/bazee"
│   ├── 8                           // Directory "/foo"
│   │   ├── 2                       // Directory "/foo/bar"
│   │   │   ├── 3                   // File "/foo/bar/zee.txt"
│   │   │   └── 3.zee.txt           // Metadata for "/foo/bar/zee.txt"
│   │   └── 2.bar                   // Metadata for "/foo/bar"
│   └── 8.foo                       // Metadata for "/foo"
├── 1.root                          // Metadata for root
├── counter                         // Keeps track of the next available FID
└── tmp                             // Directory for temporary files
```

```
# NFS

/                                   // Root directory
├── foo                             // Directory
│   └── bar                         // Directory
│       └── zee.txt                 // File
├── baz.txt                         // File
└── bazee                           // Directory
```

#### Finding files

Most NFS operations come with a FID for some file. This means that it should be
possible to find a file based solely on its FID. We do this via a breadth-first
search in the server FS for a server file with the correct FID (e.g. if we are
looking for file FID=3, we look for a file named `3` in the server FS).

To avoid the performance overhead, we designed a cache for these operations
which does not overly complicate rename operations. Cache misses, creation of
files/dirs, deletion of files/dirs, and renames all update this cache
accordingly. Since NFS clients will ususally traverse paths sequentially, there
should rarely be a BFS except soon after a crash. We could further persist this
cache if we want further performance benefits, but we decided not to do that.

#### Writes

Synchronous writes are done by creating a file in the server `data_dir/tmp`
directory which is a copy of the current contents of the file. We then modify
the copy in place before atomically renaming it to replace the original file.
For large writes that are broken into smaller writes, this does a lot of
unnecessary copying and is _really_ slow.

Asynchronous writes avoid this needless copying. Clients send writes and get
ACKs immediately, but with no guarantee of persistence. The server stores the
data in memory until a client writing the file does a COMMIT. At that point all
of the writes are persisted before the server ACKs.

The server maintains an epoch number, which is incremented on startup. It sends
this epoch number to client in the ACK for a write or a commit. If the client
determines that the server has restarted since the client's last operation, it
must resend all of it's uncommitted data to the server to ensure that it is
actually committed.

#### Crash Recovery

We maintain the invariant that an existing NFS file _always_ has valid data and
metadata files co-existing in the same server directory in the server FS. In
other words, a data file or a metadata file without its partner is considered
to not exist. We maintain this invariant even in the presence of concurrent
updates and crashes.

This invariant means that restarting the server requires no extra work. We do
not need a special crash recovery procedure.

However, a crash may leave stale junk files around if it interrupts some
operations. This doesn't affect correctness at all, but it can waste space. To
mitigate this, we could build a garbage collector, but we opted to let the
garbage pile up in the intrest of implementation time.

## Development and Running

### Requirements

You need Rust installed [from here](https://www.rust-lang.org/en-US/install.html).

You need some dependencies for Thrift. [See here](https://thrift.apache.org/docs/install/).

You also need `libfuse-dev` and `pkg-config` (`sudo apt-get install libfuse-dev pkg-config`).

### Building

The first time you build, you will need to also build `Thrift`:

```sh
git submodule update --init --recursive
cd thrift
./bootstrap.sh
./configure
make
make install
```

For more info about building Thrift, [look here](https://thrift.apache.org/docs/BuildingFromSource).

After this, you should be able to use `cargo` to build and run. There are two
clients: a CLI client which allows you to send raw NFS commands for debugging,
and a FUSE client which mounts the NFS at a given mountpoint.

```sh
# To run CLI client
cd client
cargo run --release --bin client_cli -- -s <address of server> -c <COMMAND>

# To run FUSE client
cd client
cargo run --release --bin client_fuse -- -s <address of server> -m <mountpoint>

# To unmount the FUSE FS (run in another terminal)
fusermount -u <mountpoint>

# To run server
cd server
cargo run --release -- -s <address of server> -d <server data dir>

# To run server with LOGGING
RUST_LOG=thrift,server,handle cargo run --release -- -s <address of server> -d <server data dir>
```
