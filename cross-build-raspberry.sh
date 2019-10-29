set -x
if [ $TRAVIS_OS_NAME != "linux" ]
then
  exit
fi
mkdir -p target/release
sysroot=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot/ 
docker run --user root --entrypoint sh \
  ragnaroek/rust-raspberry:1.38.0 -c \
  "set -x && \
  (
sed -i 's/bash/bash -xe/' /home/cross/bin/run.sh /home/cross/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/bin/gcc-sysroot && \
  sed -i 's/set -e/set -xe/' /home/cross/bin/run.sh && \
  sed -i 's,/pi-tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi/arm-bcm2708hardfp-linux-gnueabi/sysroot,/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot,' /home/cross/bin/run.sh && \ 
  DEBIAN_FRONTEND=noninteractive apt-get install -y wget python3 && \
  mkdir -p /home/cross/project && \
  mkdir -p /home/cross/deb-deps && \
  (cd /home/cross/deb-deps && \
  wget http://ftp.debian.org/debian/pool/main/o/openssl/libssl-dev_1.1.1d-2_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/o/openssl/openssl_1.1.1d-2_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libb/libbsd/libbsd0_0.7.0-2_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxau/libxau-dev_1.0.8-1+b2_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxau/libxau6_1.0.8-1+b2_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxdmcp/libxdmcp-dev_1.1.2-3_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxdmcp/libxdmcp6_1.1.2-3_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-shape0-dev_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-render0-dev_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-xfixes0-dev_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-xfixes0-dev_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb1-dev_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb1_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb1_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-shape0_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-render0_1.10-3+b1_armhf.deb && \
  wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-xfixes0_1.10-3+b1_armhf.deb ) && \
  chown -R cross /home/cross/deb-deps && \
  chown -R cross /home/cross/project) 1>&2 && \
  su cross -c \"\
  cd /home/cross && \
  mkdir -p /home/cross/project/src && \
  echo '$(cat src/main.rs|base64)'|base64 -d > /home/cross/project/src/main.rs && \
  echo '$(cat Cargo.toml|base64)'|base64 -d > /home/cross/project/Cargo.toml && \
  cat /home/cross/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/bin/gcc-sysroot >&2 && \
  echo UGUU43 >&2 && \
  find /home/cross/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/ -name openssl >&2 && \
  OPENSSL_LIB_DIR=$sysroot/usr/lib/arm-linux-gnueabihf \
  OPENSSL_DIR=$sysroot/etc/ssl \
  OPENSSL_INCLUDE_DIR=$sysroot/usr/include/openssl/ \
  /home/cross/bin/run.sh build --release >&2 && \
  cat /home/cross/project/target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions\"" > target/release/youtube-subscriptions-$TRAVIS_OS_NAME-arm
