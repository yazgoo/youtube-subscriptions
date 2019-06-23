# youtube-subscriptions

terminal UI for viewing youtube subscriptions.
Especially well suited for Raspberry Pi.

<a href=https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0>
<img width=250 src="https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0.svg"/>
</a>

# requirements

- [youtube-dl](https://ytdl-org.github.io/youtube-dl/index.html) to download youtube videos
- [omxplayer](https://www.raspberrypi.org/documentation/raspbian/applications/omxplayer.md) or [vlc](https://www.videolan.org) to play videos

# setup

download your [youtube subscriptions OPML](https://www.youtube.com/subscription_manager?action_takeout=1).
and save them as the following file:
  ~/.config/youtube-subscriptions/subscription_manager


# usage

press h for help.

# cross compiling for raspberry pi

simply run:

```sh
./cross-build-raspberry.sh
```
binary will be in `target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions`
