export RUST_TEST_THREADS=1
cargo b --bin server & bg
cargo t --features="hcor_client"
