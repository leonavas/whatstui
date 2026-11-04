package main

// A message's info, as WhatsApp Web's "Dados da mensagem": who got one of
// yours, read it and played it, and when. Each receipt's first time per
// person is kept, from the phone's history too; in a group the ticks move
// to read once everyone has.

import (
	"context"
	"time"

	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
)

// ownJID says whether it's you: your number or your hidden id.
func (b *Bridge) ownJID(j types.JID) bool {
	if j.IsEmpty() {
		return false
	}
	return j.User == b.me().User || (b.stored() && j.User == b.cli.Store.GetLID().User)
}

// receiptsOf keeps who a receipt is from, for the message's info (before
// the TUI hears of it and asks); your other devices aren't anyone.
func (b *Bridge) receiptsOf(chat types.JID, e *events.Receipt) {
	who := b.norm(e.Sender)
	if b.ownJID(who) || e.IsFromMe {
		return
	}
	ts := e.Timestamp.Unix()
	if e.Timestamp.IsZero() {
		ts = time.Now().Unix()
	}
	var delivered, read, played int64
	switch e.Type {
	case types.ReceiptTypeDelivered:
		delivered = ts
	case types.ReceiptTypeRead:
		read = ts
	case types.ReceiptTypePlayed:
		played = ts
	default:
		return
	}
	for _, id := range e.MessageIDs {
		b.receipted(chat.String(), id, who.String(), delivered, read, played)
	}
}

// groupTicks moves yours in a group on as its members' receipts come, as
// WhatsApp does: delivered at the first, read (or played) once everyone in
// it has.
func (b *Bridge) groupTicks(chat types.JID, ids []types.MessageID, read bool) {
	others := 0
	if read {
		others = b.othersIn(chat)
	}
	by := map[int][]types.MessageID{}
	for _, id := range ids {
		s := 2
		if others > 0 {
			var n int
			b.db.QueryRow(`SELECT count(*) FROM receipts WHERE chat = ? AND id = ? AND read > 0`, chat.String(), id).Scan(&n)
			if n >= others {
				s = 3
			}
		}
		by[s] = append(by[s], id)
	}
	for s, ids := range by {
		b.setStatus(chat, ids, s)
	}
}

// receipted keeps when someone got, read or played one of your messages,
// the first time of each: a play is also a read, a read also a delivery.
func (b *Bridge) receipted(chat, id, user string, delivered, read, played int64) {
	if played > 0 && (read == 0 || played < read) {
		read = played
	}
	if read > 0 && (delivered == 0 || read < delivered) {
		delivered = read
	}
	b.db.Exec(`INSERT INTO receipts (chat, id, user, delivered, read, played) VALUES (?, ?, ?, ?, ?, ?)
		ON CONFLICT (chat, id, user) DO UPDATE SET
			delivered = min(coalesce(nullif(delivered, 0), excluded.delivered), coalesce(nullif(excluded.delivered, 0), delivered)),
			read = min(coalesce(nullif(read, 0), excluded.read), coalesce(nullif(excluded.read, 0), read)),
			played = min(coalesce(nullif(played, 0), excluded.played), coalesce(nullif(excluded.played, 0), played))`,
		chat, id, user, delivered, read, played)
}

// msgInfo answers with who got, read and played one of yours, and when,
// the latest first; in a group, also how many others are in it, for the
// ones it hasn't reached yet.
func (b *Bridge) msgInfo(chat, id string) {
	type receipt struct {
		JID       string `json:"jid"`
		Name      string `json:"name"`
		Delivered int64  `json:"delivered"`
		Read      int64  `json:"read"`
		Played    int64  `json:"played"`
	}
	out := map[string]any{"t": "msg_info", "chat": chat, "id": id}
	list := []receipt{}
	rows, err := b.db.Query(`SELECT user, delivered, read, played FROM receipts WHERE chat = ? AND id = ? ORDER BY played DESC, read DESC, delivered DESC`, chat, id)
	if err == nil {
		for rows.Next() {
			var r receipt
			if rows.Scan(&r.JID, &r.Delivered, &r.Read, &r.Played) == nil {
				list = append(list, r)
			}
		}
		rows.Close()
	}
	for i := range list {
		list[i].Name = b.quoteName(list[i].JID)
	}
	out["receipts"] = list
	if jid, err := types.ParseJID(chat); err == nil && jid.Server == types.GroupServer {
		if n := b.othersIn(jid); n >= 0 {
			out["others"] = n
		}
	}
	emit(out)
}

// othersIn is how many are in a group besides you, as whatsmeow keeps its
// members for sending: asked once, then kept up to date by the group's
// notices; -1 when it can't be had.
func (b *Bridge) othersIn(group types.JID) int {
	ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
	defer cancel()
	g, err := b.cli.DangerousInternals().GetCachedGroupData(ctx, group)
	if err != nil || g == nil {
		return -1
	}
	n := 0
	for _, m := range g.Members {
		if !b.ownJID(m) {
			n++
		}
	}
	return n
}
