[![Discord](https://img.shields.io/badge/discord--blue?logo=discord)](https://discord.gg/F684Y8rYwZ)

# youtube-subscriptions

terminal UI for viewing youtube and/or peertube subscriptions.
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

# setup (youtube)

Create an subscription_manager file:

```
echo '<opml></opml>' > ~/.config/youtube-subscriptions/subscription_manager
```

Go to your channel page: https://www.youtube.com/feed/channels
Scroll to the bottom of the page til all your channels are loaded.
Save the source of the page in `channels.html`.

Then recover your channels list by running the following command (can take a long time if you have a lot of channels) 

```
./extract-channel-ids.sh channels.html | tee channel_ids
```

copy all those id in channel_ids list (see configuration section)

# setup (peertube)

Create a configuration file (see configuration section)
and add the channel urls you want to register to `channel_urls` list.

# backround mode

Sometimes reloading the video list can take a long time.

To avoid blocking the main app, you can run the video reload in a separate process.

Just run with `--background` flag (you can have it in a cron), and you can reload the main UI with `r`.

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
  "youtube_instance": "https://invidious.privacydev.net/",
  "video_extension": "mp4",
  "kind_symbols": {
    "Audio": "ﱘ",
    "Video": "",
    "Other": ""
  },
  "players": [
    ["/usr/bin/mplayer", "-fs"]
  ],
  "channel_ids": [],
  "channel_urls": [],
  "mpv_mode": true,
  "mpv_path": "/usr/local/bin/mpv"
}

```

| field               | description                                                                                         | default value
| ------              | -----------                                                                                         | -------------
| video_path          | directory where videos will be stored                                                               | `/tmp`
| cache_path          | file path where video list will be stored                                                           | `/tmp/yts.json`
| blockish_player     | [blockish player](https://github.com/yazgoo/blockish-player) to use (supersedes players)            | None
| players             | list of players command for videos in order of priority                                             |
| youtubedl_format    | see [youtube-dl doc](https://github.com/ytdl-org/youtube-dl/blob/master/README.md#format-selection) | `[height <=? 360][ext = mp4]`
| youtube_instance    | invidious / youtube instance to use to open videos                                                  | https://www.youtube.com/   |
| video_extension     | youtube-dl video extension as per format                                                            | `mp4`
| kind_symbols        | hash of characters to describe the media                                                            | `{ "Audio": "a", "Video": "v", "Magnet": "m", "Other": "o"  }`
| channel_ids         | list of additional channel ids which will be also fetched                                           | `[]`
| channel_urls        | list of additional channel urls which will be also fetched (can be used for peertube)               | `[]`
| mpv_mode            | try and start mpv to play the youtubee video first                                                  | `true`
| mpv_path            | path to mpv binary (will be use if mpv_mode is true)                                                | `/usr/bin/mpv`
| open_magnet         | tool to use to open magnet links (e.g. transmission-remote-cli                                      | None
| auto_thumbnail_path | file path to write thumbnails to when cursor is moved                                               | None

`__HOME` will be substituted with the home path.

# cross compiling for raspberry pi

simply run:

```sh
./cross-build-raspberry.sh
```

binary will be in `target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions`
