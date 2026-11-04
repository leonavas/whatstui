package main

// Your own profile, changed from the account menu: your name, your about
// (the recado) and your picture, as WhatsApp Web's profile changes them.

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/appstate"
	waBinary "go.mau.fi/whatsmeow/binary"
	"go.mau.fi/whatsmeow/types"
)

// WhatsApp's limits for your name and your about.
const (
	maxNameLen  = 25
	maxAboutLen = 139
)

// setDone tells the TUI how a change to your profile went: what is name,
// about or photo, text what it is now; err when it didn't take.
func setDone(what, text string, err error) {
	out := map[string]any{"t": "profile_set", "what": what, "text": text}
	if err != nil {
		out["error"] = err.Error()
	}
	emit(out)
}

// setName changes your name, the one others see: an app state setting, as
// WhatsApp Web's profile does, which the phone and your other devices follow.
func (b *Bridge) setName(text string) {
	name := strings.TrimSpace(text)
	if name == "" || utf8.RuneCountInString(name) > maxNameLen {
		setDone("name", "", errors.New("nome inválido"))
		return
	}
	// A business account's name is its business name, which only the phone
	// changes (WhatsApp Web Business shows it read-only).
	if b.cli.Store.BusinessName != "" {
		setDone("name", "", errors.New("numa conta comercial o nome muda só no celular"))
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
	defer cancel()
	if err := b.cli.SendAppState(ctx, appstate.BuildSettingPushName(name)); err != nil {
		logf("set_name: %v", err)
		setDone("name", "", err)
		return
	}
	// Fetching the patch back already sets it; when that didn't happen, the
	// presence still goes out with the new name.
	if b.cli.Store.PushName != name {
		logf("set_name: sent, but the patch didn't come back; keeping it locally")
		b.cli.Store.PushName = name
		_ = b.cli.Store.Save(ctx)
	}
	logf("set_name: ok")
	setDone("name", name, nil)
}

// setAbout changes your about, the text under your name. Two places keep
// it: the text status, which the phones show and edit now (and so what
// everyone sees), and the older "status" WhatsApp Web's profile still sends,
// for the clients that read that one. Both go, as the phone does; the text
// status is the one that counts, and it's read back to tell it took.
func (b *Bridge) setAbout(text string) {
	about := strings.TrimSpace(text)
	if about == "" || utf8.RuneCountInString(about) > maxAboutLen {
		setDone("about", "", errors.New("recado inválido"))
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
	defer cancel()
	// No emoji, and -1 for one that doesn't expire (the server refuses 0,
	// what whatsmeow's SetStatusMessage sends, and a missing duration).
	res, err := b.cli.DangerousInternals().SendMexIQ(ctx, mutationUpdateTextStatus, map[string]any{
		"input": map[string]any{"text": about, "ephemeral_duration_sec": -1},
	})
	if err != nil {
		logf("set_about: text status: %v", err)
		setDone("about", "", err)
		return
	}
	if _, err := b.cli.DangerousInternals().SendIQ(ctx, whatsmeow.DangerousInfoQuery{
		Namespace: "status",
		Type:      "set",
		To:        types.ServerJID,
		Content:   []waBinary.Node{{Tag: "status", Content: []byte(about)}},
	}); err != nil {
		logf("set_about: status IQ: %v", err)
	}
	if ts, ok := b.textStatus(ctx, b.me()); ok && ts.Text != about {
		logf("set_about: the server kept %q (%s)", ts.Text, cutBytes(string(res), 200))
		setDone("about", "", errors.New("não salvou"))
		return
	}
	logf("set_about: ok")
	setDone("about", about, nil)
}

// setPhoto changes your profile picture to the JPEG at path, which the TUI
// already made square and 640px (what WhatsApp keeps), and then removes;
// an empty path removes the picture.
func (b *Bridge) setPhoto(path string) {
	var jpeg []byte
	if path != "" {
		data, err := os.ReadFile(path)
		os.Remove(path)
		if err != nil || len(data) == 0 {
			logf("set_photo: read: %v", err)
			setDone("photo", "", errors.New("não leu a imagem"))
			return
		}
		jpeg = data
	}
	ctx, cancel := context.WithTimeout(b.ctx, 60*time.Second)
	defer cancel()
	// No target: your own picture.
	if _, err := b.cli.SetGroupPhoto(ctx, types.EmptyJID, jpeg); err != nil {
		logf("set_photo: %v", err)
		setDone("photo", "", err)
		return
	}
	removed := jpeg == nil
	logf("set_photo: ok (removed: %v)", removed)
	if removed {
		setDone("photo", "removed", nil)
	} else {
		setDone("photo", "set", nil)
	}
	// The new one (or none) for the avatars, and full size for the info panel.
	me := b.me().String()
	select {
	case b.avatars <- avatarReq{jid: me, force: true}:
	default:
	}
	b.photo(me, b.me())
}

// The query WhatsApp Web's contacts ask the newer about with (the "text
// status": text, emoji and how long it lasts), which the phones show and edit.
const (
	queryTextStatusList      = "24072923595647473"
	mutationUpdateTextStatus = "9152604461510864"
)

type textStatusInfo struct {
	JID      string `json:"jid"`
	Text     string `json:"text"`
	Duration *int64 `json:"ephemeral_duration_sec"`
	Updated  any    `json:"last_update_time"`
	Emoji    *struct {
		Content string `json:"content"`
	} `json:"emoji"`
}

// textStatus asks the text status of jid; ok is false when it couldn't.
func (b *Bridge) textStatus(ctx context.Context, jid types.JID) (textStatusInfo, bool) {
	data, err := b.cli.DangerousInternals().SendMexIQ(ctx, queryTextStatusList, map[string]any{
		"input": []map[string]any{{"jid": jid.ToNonAD().String()}},
	})
	if err != nil {
		logf("text status %s: %v", jid, err)
		return textStatusInfo{}, false
	}
	var resp struct {
		List []textStatusInfo `json:"xwa2_text_status_list"`
	}
	if err := json.Unmarshal(data, &resp); err != nil || len(resp.List) == 0 {
		logf("text status %s: %v (%s)", jid, err, cutBytes(string(data), 300))
		return textStatusInfo{}, false
	}
	return resp.List[0], true
}

// aboutOf is the about the others see for jid: the text status when it has
// one that hasn't expired, else the older status (legacy, from usync).
func (b *Bridge) aboutOf(ctx context.Context, jid types.JID, legacy string) string {
	ts, ok := b.textStatus(ctx, jid)
	if !ok || ts.Text == "" {
		return legacy
	}
	if ts.Duration != nil && *ts.Duration > 0 {
		var at int64
		switch v := ts.Updated.(type) {
		case float64:
			at = int64(v)
		case string:
			at, _ = strconv.ParseInt(v, 10, 64)
		}
		if at > 0 && time.Now().Unix() > at+*ts.Duration {
			return legacy
		}
	}
	return ts.Text
}

// cutBytes is s, at most n bytes of it, not cutting a character in half.
func cutBytes(s string, n int) string {
	if len(s) <= n {
		return s
	}
	for n > 0 && (s[n]&0xC0) == 0x80 {
		n--
	}
	return s[:n]
}
