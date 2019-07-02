# youtube-subscriptions

terminal UI for viewing youtube subscriptions.
Especially well suited for Raspberry Pi.

<a href=https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0>
<img width=250 src="https://asciinema.org/a/6pXhdC6yCrAU7LrtpeUMPhMA0.svg"/>
</a>

# requirements

- [youtube-dl](https://ytdl-org.github.io/youtube-dl/index.html) to download youtube videos
- [omxplayer](https://www.raspberrypi.org/documentation/raspbian/applications/omxplayer.md) or [vlc](https://www.videolan.org) to play videos

# installing

You can download a self-contained binary from [releases page](https://github.com/yazgoo/youtube-subscriptions/releases)

# setup

Download your [youtube subscriptions OPML](https://www.youtube.com/subscription_manager?action_takeout=1).
and save it as the following file:
  ~/.config/youtube-subscriptions/subscription_manager

# usage

press h for help.

# configuration


```json
{
  "video_path": "__HOME/.cache/yts/videos",
  "cache_path": "__HOME/.cache/yts/yts.json",
  "players": [
    ["/usr/bin/mplayer", "-fs"]
  ]
}

```

| field      | description
| ------     | ------------
| video_path | directory where videos will be stored
| cache_path | file path where video list will be stored
| players    | list of players command for videos in order of priority

# download mode

You can update the subscriptions and download the last N videos by running.
Here with N = 5:

```sh
$ youtube-subscriptions 5
```

This is very usefull to download your subscriptions in a cron.

Don't forget to put the path were youtube-dl is installed.

Example crontab:

```cron
PATH=/home/pi/.local/bin/:/usr/local/sbin:/usr/local/bin:/sbin:/bin:/usr/sbin:/usr/bin
50 * * * * /home/pi/youtube-subscriptions 5 > /home/pi/youtube-subscriptions.log 2>&1
```

# cross compiling for raspberry pi

simply run:

```sh
./cross-build-raspberry.sh
```
binary will be in `target/arm-unknown-linux-gnueabihf/release/youtube-subscriptions`
