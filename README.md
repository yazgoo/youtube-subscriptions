# youtube-subscriptions

terminal UI for viewing youtube subscriptions.
Especially well suited for Raspberry Pi.

<a href=https://youtu.be/WVZpqXBmB3U>
<img width=250 src="https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0.svg"/>
</a>

# requirements

- [mpv](http://mpv.io) to stream videos (if `mpv_mode` is enabled (default))
- [youtube-dl](https://ytdl-org.github.io/youtube-dl/index.html) to download youtube videos (if `mpv_mode` is disabled)
- [omxplayer](https://www.raspberrypi.org/documentation/raspbian/applications/omxplayer.md) or [vlc](https://www.videolan.org) or [mplayer](http://www.mplayerhq.hu) or [mpv](http://mpv.io) to play videos

# installing

You can download a self-contained binary from [releases page](https://github.com/yazgoo/youtube-subscriptions/releases)

# setup

Download your [youtube subscriptions OPML](https://www.youtube.com/subscription_manager?action_takeout=1).
and save it as the following file:
  ~/.config/youtube-subscriptions/subscription_manager

# usage

press h for help.

# configuration

You can optionnaly add a user configuration at

`$HOME/.config/youtube-subscriptions/config.json`

example:

```json
{
  "video_path": "__HOME/.cache/yts/videos",
  "cache_path": "__HOME/.cache/yts/yts.json",
  "youtubedl_format": "[height <=? 360][ext = mp4]",
  "video_extension": "mp4",
  "players": [
    ["/usr/bin/mplayer", "-fs"]
  ],
  "channel_ids": [],
  "mpv_mode": true,
  "mpv_path": "/usr/local/bin/mpv"
}

```

| field            | description                                                                                         | default value
| ------           | -----------                                                                                         | -------------
| video_path       | directory where videos will be stored                                                               | `/tmp`
| cache_path       | file path where video list will be stored                                                           | `/tmp/yts.json`
| players          | list of players command for videos in order of priority                                             |
| youtubedl_format | see [youtube-dl doc](https://github.com/ytdl-org/youtube-dl/blob/master/README.md#format-selection) | `[height <=? 360][ext = mp4]`
| video_extension  | youtube-dl video extension as per format                                                            | `mp4`
| channel_ids      | list of additional channel ids which will be also fetched                                           | `[]`
| mpv_mode         | try and start mpv to play the youtubee video first                                                  | `true`
| mpv_path         | path to mpv binary (will be use if mpv_mode is true)                                                | `/usr/bin/mpv`

`__HOME` will be substituted with the home path.

# cross compiling for raspberry pi

simply run:

```sh
./cross-build-raspberry.sh
```
binary will be in `target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions`
