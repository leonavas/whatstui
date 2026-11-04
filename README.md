# WhatsTUI

A lightweight WhatsApp client for the terminal. It links to your phone as
another device (like WhatsApp Web) and shows your chats, groups, pictures,
voice notes and contact details in a terminal UI.

The WhatsApp side is a small Go helper built on
[whatsmeow](https://github.com/tulir/whatsmeow), carried inside the
binary; the interface is Rust ([ratatui](https://ratatui.rs)).

The interface is in English by default and can be switched to Brazilian
Portuguese in **Settings** (account menu, Alt+S), stored as
`language = pt-BR` in `~/.config/whatstui/config`.

> **Unofficial.** whatstui is not affiliated with, endorsed by or
> connected to WhatsApp or Meta. "WhatsApp" is a trademark of its owner, used
> here only to say what this connects to. Use it at your own risk.

## Requirements

- Linux (x86_64 or aarch64).
- A terminal with an image protocol is recommended for pictures and avatars:
  kitty, ghostty, WezTerm, foot or any sixel terminal. Others fall back to
  half-block images.
- A [Nerd Font](https://www.nerdfonts.com/) for the icons.
- Optional, at run time: `mpv` for voice notes, audio and videos, `ffmpeg`
  for sharper video previews, `wl-paste` or
  `xclip` to paste pictures, `zenity`, `yad` or `kdialog` to pick files.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/leonavas/whatstui/master/install.sh | sh
```

The script ([install.sh](install.sh)) downloads the latest
[release](https://github.com/leonavas/whatstui/releases) for your
architecture (built on Ubuntu 24.04, so glibc 2.39 or newer), checks its
SHA-256 and puts `whatstui` in `~/.local/bin`, without sudo, plus its
desktop entry and icon in `~/.local/share`, so it shows in your app launcher.
It warns when that folder is not on your `PATH`. `WHATSTUI_INSTALL_DIR`
changes the folder and `WHATSTUI_VERSION=v0.1.0` picks a version.
Installed this way, whatstui updates itself (see [Auto-update](#auto-update));
running the script again also updates it.

### Upgrading from whatsapp-tui

The app was renamed whatstui. On first start it moves
`~/.local/share/whatsapp-tui`, `~/.cache/whatsapp-tui` and
`~/.config/whatsapp-tui` to the `whatstui` names, linked session included, so
there's no QR code to scan again. The old binary
(`~/.local/bin/whatsapp-tui`), desktop entry
(`~/.local/share/applications/whatsapp-tui.desktop`) and icons
(`whatsapp-tui.*`) can be removed. The window class is now
`org.omarchy.whatstui`.

### From source

You need Rust (stable) and Go, with CGO and a C compiler (the helper uses
SQLite through cgo). The repository pins Go with [mise](https://mise.jdx.dev):
`mise install`, and the build finds it through `mise exec` when `go` isn't on
your `PATH`.

Then either:

```sh
cargo install --locked --git https://github.com/leonavas/whatstui
```

or, from a clone:

```sh
cargo build --release
./target/release/whatstui
```

`assets/` has the icon and a desktop entry (`whatstui.desktop`, which
opens it in a terminal with the window class `org.omarchy.whatstui`).

## Link your phone

The first start shows a QR code: scan it in WhatsApp on your phone, under
**Linked devices**. The session is kept in `~/.local/share/whatstui/`.

## Keys

| key | does |
| --- | --- |
| `↑` `↓` / `k` `j`, `g` `G` | move through the chats |
| `Enter` / `→` / `l` | open the chat |
| `Ctrl+N` / `Ctrl+P` | open the next or previous chat |
| `/`, `Ctrl+K` | search chats, contacts, messages and phone numbers (`Ctrl+K` from anywhere, the message box too) |
| `1` `2` `3` `4` | all chats, unread, favorites, groups |
| `m` | the chat's menu: archive, pin, mark read or unread, mute, favorite, clear, delete |
| `Esc` / `←` | out of the archived chats |
| `Tab` | between the chat list and the message box |
| `i` | the contact's or group's details |
| `Space` | play or pause the open chat's last audio |
| `v` | open the open chat's last video in the viewer |
| `Space` / `Enter`, `b`, `p`, `o`, `Esc` / `q` | in the viewer: play or stop the video, save it to your downloads folder, save it and show it in the file manager, open it outside the app, close |
| `PgUp` `PgDn` | scroll the conversation |
| `Enter` / `Shift+Enter` | send / new line |
| `↑` in an empty message box | edit your last message (within 15 minutes; `Esc` gives up) |
| `Alt+R` | reply to the chat's last message; again, to the one before (`Esc` gives up) |
| `Ctrl+B` / `Ctrl+I` | bold / italic (`Alt+B` / `Alt+I` where the terminal keeps them) |
| `Ctrl+Shift+X` `M` `C` | strikethrough, monospace, code (`Alt+X` `Alt+M` `Alt+C` too) |
| `Ctrl+Shift+9` | make the line a quote, or not |
| `:` | emoji by name |
| `@` in a group | mention someone, or **todos** for everyone (`↑` `↓` pick, `Enter` or `Tab` take it) |
| `Alt+E` / `Alt+A` | the emoji panel / attach files |
| `Ctrl+V` | paste a picture or files |
| `Alt+S` / `,` | your account: pause notifications, profile, settings, shortcuts, disconnect |
| `Ctrl+/` / `?` | the keyboard shortcuts (`?` from the chat list) |
| `Ctrl+C` with messages picked or text selected | copy them (one message, its text; more, a line each with time and author) |
| `q`, `Ctrl+C` | quit |

All of them are also in your name › **Atalhos de teclado**, which follows
whether Enter sends.

Your name › **Perfil** changes your name (`n`), your recado (`r`) and your
photo (`f`, through the file chooser; cropped square from the middle) or
removes it (`x`), as WhatsApp Web's profile does; the phone follows.

The mouse works too: click chats, links, audio players, photos and videos (open
them in the viewer; a click outside the picture closes it), a business message's link buttons, the clip
and the smiley in the message box, and your name at the foot of the chat
list; right-click a chat (or click the chevron by its time) for its menu.

Hovering a message shows a chevron in its corner (a right-click does the
same) with its menu: **Responder**, **Copiar**, **Editar** (yours, for 15
minutes), **Reagir**, **Abrir no Maps** (a shared place), **Dados da mensagem**
(yours: when it was delivered, read and played; in a group, by whom, and how
many it hasn't reached yet) and **Apagar**. Responder shows the message over the
message box, with ✕ to give up; what's sent next goes as its reply. Apagar puts a checkbox by each message to pick
more of them (`Esc` leaves, `Enter` or the trash asks, `Ctrl+C` or the copy button copies them); then **Apagar para
mim**, or **Apagar para todos** when every one picked is yours, about two and
a half days old at most, and not in the chat with yourself.

## Photos and videos

Photos and videos show inside their bubbles, as on WhatsApp Web: the
message's thumbnail at once (blurred, as the Web's, till the file is here), the photo itself once it's downloaded (photos on
screen download by themselves; for older ones the phone isn't asked). A click
opens one in the viewer, as WhatsApp Web's: big over the whole window, with
who sent it, when, and its caption. A video (GIFs too, looped and muted)
plays there through `mpv --vo=kitty` in terminals that speak kitty's graphics
protocol (kitty, ghostty); the rest of the screen waits while it plays, and
when it ends its picture stays, to play again. `o` opens it outside the app.
Elsewhere, or with **Tocar vídeos no app** off in the settings, a video opens
in an mpv window, and without mpv in whatever `xdg-open` picks; photos open
in your image viewer in terminals without pictures. With `ffmpeg` installed, a downloaded video's bubble gets a
sharper frame than the thumbnail. Messages from businesses (and from
WhatsApp itself) show their header picture, title, footer and buttons; link
buttons open in the browser.

A shared place shows the small map the phone sent, its name and address; a
click on the map or the name opens it in Google Maps.

Downloads are kept in `~/.cache/whatstui/media/`: what isn't seen in 7
days goes, and over 900 MB the media seen longest ago goes first (it
downloads again when it's wanted).

## Chats

As on WhatsApp Web, pinned chats come first (the last pinned on top, three at
most), and a chat's row shows when it's muted or pinned. Archived chats leave
the list for **Arquivadas** at its top, with how many of them are unread; new
messages keep them there unless the phone is set to unarchive them. The chat
menu archives, pins, marks read or unread, mutes (8 hours, a week or always),
adds to Favorites (the **Favoritas** chip), clears and deletes, and the
change goes to WhatsApp, so your phone and other devices follow; changes made
there show here too. A favorite added here reaches the phone once WhatsApp
has sent this device its favorites (that is, once you have at least one
favorite set on the phone); until then it's kept here only.

A chat read on the phone stops being unread here, with read receipts on or
off, and edits made on the phone change their message here too. Business
chats you haven't saved go by their verified name, as on WhatsApp Web.

Calls show in their chat, centered, and as the chat's last message in the
list: voice or video, and how they went (missed, declined, answered on
another device, or how long they lasted). They're heard live and taken from
the phone's call log, older ones included; only a missed call counts as
unread. This device never rings or answers one.

## Events and scripting

whatstui runs once per session (running it again brings the open one to
the front) and tells other programs what is unread: a snapshot in
`$XDG_RUNTIME_DIR/whatstui/state.json`, an event stream
(`whatstui events`), hooks in `~/.config/whatstui/hooks/`, and
commands such as `whatstui open <chat>` and `whatstui pause 1h`. See
[docs/events.md](docs/events.md).

It shows a tray icon (StatusNotifierItem) with a dot while chats wait. The
settings (your name › Configurações) turn it off, and with it the sidebar's
photos, read receipts, whether Enter sends and the update check; they're kept
as `key = value` lines in `~/.config/whatstui/config` (`tray`,
`sidebar_avatars`, `read_receipts`, `enter_sends`, `auto_update`).

## Omarchy plugin

![WhatsApp in the Omarchy bar: badges, the hover preview and the menu](https://raw.githubusercontent.com/leonavas/omarchy-whatstui/master/preview.png)

On [Omarchy](https://omarchy.org), the
[WhatsTUI bar plugin](https://github.com/leonavas/omarchy-whatstui)
(`leonavas.whatstui`) runs on whatstui and puts WhatsApp in the bar in
place of the tray icon:

- red and blue badges for unread direct chats and groups (muted chats stay
  quiet);
- a preview of the unread chats and their newest messages on hover, a click
  jumps to the chat;
- desktop notifications for new messages, with the contact's picture;
- whatstui parked in the scratchpad (SUPER+S), a click shows or hides it;
- a right-click menu: Launch on Login, Launch in Foreground, Open in
  Scratchpad, pause notifications.

Install whatstui first (above), then:

```sh
omarchy plugin add https://github.com/leonavas/omarchy-whatstui.git
omarchy plugin enable leonavas.whatstui --section right
```

Update it with `omarchy plugin update leonavas.whatstui`, then
`omarchy restart shell`. With the plugin in the bar, turn whatstui's own
tray icon off in Settings (your name › Configurações), or with `tray = false`
in the config. Remove it with `omarchy plugin remove leonavas.whatstui`.

## Auto-update

Every time it starts, and every 6 hours while open, whatstui asks GitHub
for the latest release in the background (errors are ignored). When there is
a newer one, a small green "↑ X.Y.Z" button shows at the right of your name in
the account button, at the bottom of the sidebar. Nothing is downloaded until
you click it:

- If whatstui was installed from a release into a folder you can write to
  (like `~/.local/bin`, where install.sh puts it), the click downloads the
  archive for your architecture, checks its SHA-256 and replaces
  `whatstui` in place (and the desktop entry and icons, if install.sh put
  them in `~/.local/share`). The button reads "atualizando…" meanwhile, then
  "↻ reiniciar": click it (or press `Ctrl+O`) to restart into the new version
  in the same terminal; the WhatsApp helper stops and the new one starts with
  its own. On failure the button comes back and the error shows in the status
  line.
- If the binary belongs to something else (`~/.cargo/bin`, `/usr`, `/opt`,
  `/nix`) or the folder is read-only, the click only tells in the status line
  how to update (for cargo, the `cargo install` command above).

Development builds (`target/`) never check, unless `WHATSTUI_UPDATE_DEV=1`
is set: then the button shows (with a made-up version when there is no
release) and a click only says it is a development build.

`whatstui update` checks and installs right away, printing what it does,
and "Procurar atualizações" in the account menu checks right away from the app
(also with `auto_update = false`). Turn the automatic check off in Settings
(your name › Configurações › Atualizações), or with `auto_update = false` in
the config.

## License

Apache-2.0. See [NOTICE](NOTICE) for the trademarks and third-party data.
