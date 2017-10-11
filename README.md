ZippyNFS
--------

### Requirements

You need Rust installed [from here](https://www.rust-lang.org/en-US/install.html).

You need some dependencies for Thrift. [See here](https://thrift.apache.org/docs/install/).

You also need `libfuse`. I think this already comes install on any recent version of Ubuntu. But you can find install instructions [here](https://github.com/libfuse/libfuse/).

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

For mounting tthe client you will need to install libfuse, [look here](http://fuse.sourceforge.net/).

For Ubuntu you need to do the following:
```sh
sudo apt install libfuse-dev
```

After this, you should be able to use `cargo` to build and run:

```sh
# To run client
cd client
cargo run --release <address of server>

# To run server
cd server
cargo run --release <address of server>
```
