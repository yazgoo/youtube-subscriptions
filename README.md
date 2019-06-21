# youtube-subscriptions

terminal client for your youtube subscriptions.

<a href=https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0>
<img width=250 src="https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0.svg"/>
</a>

Also, [here is a video](https://www.youtube.com/watch?v=saYmXcZNU8M&feature=youtu.be) showing what it looks like.

# requirements

- youtube-dl
- omxplayer or vlc

# setup

download your youtube subscriptions OPML:
  https://www.youtube.com/subscription_manager?action_takeout=1
and save them as the following file:
  ~/.config/youtube-subscriptions/subscription_manager

# cross compiling for raspberry pi

simply run:

```sh
./cross-build.sh
```
binary will be in `target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions`
