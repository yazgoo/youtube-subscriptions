docker run --volume "$PWD":/home/cross/project --volume "$PWD/db-deps:/home/cross/deb-deps" --volume "$HOME/.cargo/registry:/home/cross/.cargo/registry" ragnaroek/rust-raspberry:1.35.0 build --release
