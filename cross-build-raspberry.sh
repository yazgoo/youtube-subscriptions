set -x
if [ "$1" = "noregistry" ] 
then
  mkdir -p target/release
  docker run  --entrypoint sh \
    ragnaroek/rust-raspberry:1.35.0 -c \
      "mkdir -p /home/cross/project && \
      /home/cross/bin/run.sh build --release > /dev/null && \
      cat target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions " > target/release/youtube-subscriptions-$TRAVIS_OS_NAME-arm
else
docker run --volume "$PWD":/home/cross/project \
  --volume $HOME/.cargo/registry:/home/cross/.cargo/registry \
  --volume "$PWD/db-deps:/home/cross/deb-deps" $additional_argument \
  ragnaroek/rust-raspberry:1.35.0 build --release
fi
