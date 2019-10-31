set -x
if [ $TRAVIS_OS_NAME != "linux" ]
then
  exit
fi
mkdir -p target/release
sysroot=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot/ 
docker run --entrypoint sh \
  yazgoo/rust-raspberry:latest -c \
  "set -x && \
  mkdir -p /home/cross/project/src && \
  echo '$(cat src/main.rs|base64)'|base64 -d > /home/cross/project/src/main.rs && \
  echo '$(cat Cargo.toml|base64)'|base64 -d > /home/cross/project/Cargo.toml && \
  /home/cross/bin/run.sh build --release >&2 && \
  cat /home/cross/project/target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions" > target/release/youtube-subscriptions-$TRAVIS_OS_NAME-arm
