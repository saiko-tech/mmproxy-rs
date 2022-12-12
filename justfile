run:
    cargo build --release
    sudo ./target/release/mmproxy -m 123 -l "0.0.0.0:25577" -4 "127.0.0.1:1122"
