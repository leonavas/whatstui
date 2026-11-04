package main

// Mentions: WhatsApp writes an @ in the text with the person's number (or
// hidden id), and the apps show their name in its place.

import (
	"context"
	"sort"
	"strings"
	"time"

	"go.mau.fi/whatsmeow/types"
)

// Mention is one @ in a message's text: the digits it's written with, who
// that is and the name shown for it.
type Mention struct {
	User string `json:"user"`
	JID  string `json:"jid"`
	Name string `json:"name"`
	// It's you.
	Me bool `json:"me,omitempty"`
	// It's everyone in the group: "@all" in the text, which WhatsApp shows
	// as "@todos".
	All bool `json:"all,omitempty"`
}

// allAt is whether an "@all" (everyone in the group) starts at i.
func allAt(t string, i int) bool {
	return strings.HasPrefix(t[i:], "@all") && (i == 0 || !isWord(t[i-1])) && (i+4 == len(t) || !isWord(t[i+4]))
}

// mentionAt finds the digits of an @ starting at i: at a word's start, five
// digits or more. Returns them and where they end.
func mentionAt(t string, i int) (string, int) {
	if t[i] != '@' || (i > 0 && isWord(t[i-1])) {
		return "", i
	}
	j := i + 1
	for j < len(t) && t[j] >= '0' && t[j] <= '9' {
		j++
	}
	if j-i-1 < 5 || (j < len(t) && isWord(t[j])) {
		return "", i
	}
	return t[i+1 : j], j
}

func isWord(c byte) bool {
	return c == '_' || c >= '0' && c <= '9' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'
}

// mentions are the @s in text that name someone; `seen` keeps them across
// a page of messages.
func (b *Bridge) mentions(text string, seen map[string]*Mention) []Mention {
	var out []Mention
	got := map[string]bool{}
	for i := 0; i < len(text); i++ {
		if allAt(text, i) && !got["all"] {
			got["all"] = true
			out = append(out, Mention{User: "all", Name: "todos", All: true})
			i += 3
			continue
		}
		user, end := mentionAt(text, i)
		if user == "" || got[user] {
			continue
		}
		got[user] = true
		i = end - 1
		if m := b.mention(user, seen); m != nil {
			out = append(out, *m)
		}
	}
	return out
}

// mention is who the digits are: by the hidden id's number when it's known,
// else as a number; nil for a hidden id nobody has a name for.
func (b *Bridge) mention(user string, seen map[string]*Mention) *Mention {
	if m, ok := seen[user]; ok {
		return m
	}
	var m *Mention
	defer func() { seen[user] = m }()
	if !b.stored() {
		return nil
	}
	if me := b.me(); user == me.User || user == b.cli.Store.LID.User {
		// Your name, as WhatsApp Web shows it; "Você" only without one.
		name := b.cli.Store.PushName
		if name == "" {
			name = tr("You", "Você")
		}
		m = &Mention{User: user, JID: me.String(), Name: name, Me: true}
		return m
	}
	lid := types.NewJID(user, types.HiddenUserServer)
	pn := types.NewJID(user, types.DefaultUserServer)
	who := lid
	if j := b.norm(lid); j.Server == types.DefaultUserServer {
		who = j
	} else if b.contactName(pn) != "" {
		who = pn
	} else if b.contactName(lid) == "" {
		// Someone who wrote here has a name to show, by the push one.
		var push string
		for _, j := range []types.JID{lid, pn} {
			if b.db.QueryRow(`SELECT push_name FROM messages WHERE (sender = ? OR wa_sender = ?) AND push_name != '' ORDER BY ts DESC LIMIT 1`, j.String(), j.String()).Scan(&push) == nil {
				who = j
				break
			}
		}
		if push == "" {
			// A hidden id is no number to show; a phone number is.
			if len(user) > 13 {
				return nil
			}
			who = pn
		}
	}
	var push string
	if b.contactName(who) == "" {
		b.db.QueryRow(`SELECT push_name FROM messages WHERE (sender = ? OR wa_sender = ?) AND push_name != '' ORDER BY ts DESC LIMIT 1`, who.String(), who.String()).Scan(&push)
	}
	m = &Mention{User: user, JID: who.String(), Name: b.senderName(who, push)}
	return m
}

// withMentions is text with each @'s digits replaced by the name, for where
// it's only read: a chat's preview, a quote.
func withMentions(text string, ms []Mention) string {
	if len(ms) == 0 {
		return text
	}
	names := map[string]string{}
	for _, m := range ms {
		names[m.User] = m.Name
	}
	var sb strings.Builder
	for i := 0; i < len(text); i++ {
		if n, ok := names["all"]; ok && allAt(text, i) {
			sb.WriteString("@" + n)
			i += 3
			continue
		}
		user, end := mentionAt(text, i)
		if n, ok := names[user]; ok && user != "" {
			sb.WriteString("@" + n)
			i = end - 1
			continue
		}
		sb.WriteByte(text[i])
	}
	return sb.String()
}

// mentionedJIDs are the people the text's @s name: any @number in it (the
// TUI writes the picked ones so). An "@all" in a group names everyone
// (all), as WhatsApp's "@todos": every member goes along too, for the apps
// that only know mentions by number.
func (b *Bridge) mentionedJIDs(chat types.JID, text string) (out []string, all bool) {
	got := map[string]bool{}
	for i := 0; i < len(text); i++ {
		if chat.Server == types.GroupServer && allAt(text, i) {
			all = true
			continue
		}
		user, end := mentionAt(text, i)
		if user == "" || got[user] {
			continue
		}
		got[user] = true
		i = end - 1
		// A hidden id is longer than any phone number.
		server := types.DefaultUserServer
		if len(user) > 13 {
			server = types.HiddenUserServer
		}
		out = append(out, types.NewJID(user, server).String())
	}
	if all {
		ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
		defer cancel()
		if g, err := b.cli.GetGroupInfo(ctx, chat); err == nil {
			me := b.me()
			for _, p := range g.Participants {
				j := p.JID
				if j.User == me.User || p.PhoneNumber.User == me.User || got[j.User] || got[p.PhoneNumber.User] {
					continue
				}
				got[j.User] = true
				out = append(out, j.String())
			}
		}
	}
	return out, all
}

// members answers with a group's members, for the @ list: each by the
// number an @ is written with (the hidden id when it isn't known), you
// left out.
func (b *Bridge) members(chat string) {
	jid, err := types.ParseJID(chat)
	if err != nil || jid.Server != types.GroupServer {
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
	defer cancel()
	g, err := b.cli.GetGroupInfo(ctx, jid)
	if err != nil {
		return
	}
	type member struct {
		JID   string `json:"jid"`
		Name  string `json:"name"`
		Phone string `json:"phone"`
		// What follows the @ in what's sent.
		User string `json:"user"`
	}
	me := b.me()
	list := []member{}
	for _, p := range g.Participants {
		pj := p.PhoneNumber
		if pj.IsEmpty() {
			pj = b.norm(p.JID)
		}
		if pj.User == me.User || b.ownJID(p.JID) {
			continue
		}
		phone := ""
		if pj.Server == types.DefaultUserServer {
			phone = pj.User
		}
		user := phone
		if user == "" {
			user = p.JID.User
		}
		list = append(list, member{pj.String(), b.senderName(pj, p.DisplayName), phone, user})
	}
	// By name: the saved ones, then by their own name ("~"), then by number.
	rank := func(n string) string {
		switch {
		case strings.HasPrefix(n, "~"):
			return "1" + fold(n[1:])
		case strings.HasPrefix(n, "+"):
			return "2" + n
		}
		return "0" + fold(n)
	}
	sort.SliceStable(list, func(i, j int) bool { return rank(list[i].Name) < rank(list[j].Name) })
	emit(map[string]any{"t": "members", "chat": chat, "members": list})
}
