FROM ragnaroek/rust-raspberry:1.41.1
USER root
RUN sed -i 's,/pi-tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi/arm-bcm2708hardfp-linux-gnueabi/sysroot,/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot,' /home/cross/bin/run.sh
RUN DEBIAN_FRONTEND=noninteractive apt-get install -y wget python3
RUN mkdir -p /home/cross/project
RUN mkdir -p /home/cross/deb-deps
RUN cd /home/cross/deb-deps && \
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
      wget http://ftp.debian.org/debian/pool/main/libx/libxcb/libxcb-xfixes0_1.10-3+b1_armhf.deb
RUN chown -R cross /home/cross/deb-deps
RUN chown -R cross /home/cross/project
USER cross
ENV OPENSSL_LIB_DIR=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot//usr/lib/arm-linux-gnueabihf
ENV OPENSSL_DIR=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot//etc/ssl
ENV OPENSSL_INCLUDE_DIR=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot//usr/include/openssl/
