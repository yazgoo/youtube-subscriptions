set -x
[ -z "$TRAVIS_OS_NAME" ] || additional_argument="--volume "$HOME/.cargo/registry:/home/cross/.cargo/registry"
docker run --volume "$PWD":/home/cross/project \
  --volume "$PWD/db-deps:/home/cross/deb-deps" $additional_argument \
  ragnaroek/rust-raspberry:1.35.0 build --release
