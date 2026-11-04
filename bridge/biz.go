package main

// A business's profile as WhatsApp Web's contact info shows it.

import (
	"context"
	"fmt"
	"net/url"
	"os"
	"slices"
	"strconv"
	"strings"
	"sync"
	"time"

	"go.mau.fi/whatsmeow"
	waBinary "go.mau.fi/whatsmeow/binary"
	"go.mau.fi/whatsmeow/types"
)

func logf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "%s %s\n", time.Now().Format("15:04:05.000"), fmt.Sprintf(format, args...))
}

// bizHours is one opening of a business: day 0 is Monday, times are minutes
// after midnight.
type bizHours struct {
	Day   int    `json:"day"`
	Mode  string `json:"mode"`
	Open  int    `json:"open"`
	Close int    `json:"close"`
}

// bizProfile asks for a business's profile the way GetBusinessProfile does,
// but keeps what whatsmeow's parser drops: the description, websites and
// where it is. It fills p and returns the coordinates, if any; false when
// it isn't a business.
func (b *Bridge) bizProfile(ctx context.Context, jid types.JID, p map[string]any) (lat, lon float64, ok bool) {
	resp, err := b.cli.DangerousInternals().SendIQ(ctx, whatsmeow.DangerousInfoQuery{
		Namespace: "w:biz",
		Type:      "get",
		To:        types.ServerJID,
		Content: []waBinary.Node{{
			Tag:     "business_profile",
			Attrs:   waBinary.Attrs{"v": "244"},
			Content: []waBinary.Node{{Tag: "profile", Attrs: waBinary.Attrs{"jid": jid}}},
		}},
	})
	if err != nil {
		logf("business_profile %s: %v", jid, err)
		bp, err := b.cli.GetBusinessProfile(ctx, jid)
		if err != nil || bp == nil {
			return 0, 0, false
		}
		return 0, 0, putBiz(p, bp, "", nil)
	}
	node, found := resp.GetOptionalChildByTag("business_profile")
	if !found {
		return 0, 0, false
	}
	logf("business_profile %s: %s", jid, node.String())
	bp, err := b.cli.DangerousInternals().ParseBusinessProfile(&node)
	if err != nil {
		return 0, 0, false
	}
	prof := node.GetChildByTag("profile")
	text := func(n waBinary.Node) string {
		s, _ := n.Content.([]byte)
		return strings.TrimSpace(string(s))
	}
	var sites []string
	for _, w := range prof.GetChildrenByTag("website") {
		if s := text(w); s != "" {
			sites = append(sites, s)
		}
	}
	ok = putBiz(p, bp, text(prof.GetChildByTag("description")), sites)
	lat, _ = strconv.ParseFloat(text(prof.GetChildByTag("latitude")), 64)
	lon, _ = strconv.ParseFloat(text(prof.GetChildByTag("longitude")), 64)
	return lat, lon, ok || lat != 0 || lon != 0
}

// putBiz copies a business profile into the event; false when there's
// nothing in it, as for someone who isn't a business.
func putBiz(p map[string]any, bp *types.BusinessProfile, description string, sites []string) bool {
	var cats []string
	for _, c := range bp.Categories {
		cats = append(cats, c.Name)
	}
	days := map[string]int{"mon": 0, "tue": 1, "wed": 2, "thu": 3, "fri": 4, "sat": 5, "sun": 6}
	hours := []bizHours{}
	for _, h := range bp.BusinessHours {
		d, known := days[h.DayOfWeek]
		if !known {
			continue
		}
		open, _ := strconv.Atoi(h.OpenTime)
		closes, _ := strconv.Atoi(h.CloseTime)
		hours = append(hours, bizHours{d, h.Mode, open, closes})
	}
	p["description"] = description
	p["website"] = sites
	p["address"] = bp.Address
	p["email"] = bp.Email
	p["category"] = strings.Join(cats, ", ")
	p["hours"] = hours
	// The business's clock, for whether it's open now.
	if loc, err := time.LoadLocation(bp.BusinessHoursTimeZone); err == nil && bp.BusinessHoursTimeZone != "" {
		_, off := time.Now().In(loc).Zone()
		p["tz"] = off
	}
	return description != "" || len(sites) > 0 || bp.Address != "" || bp.Email != "" || len(cats) > 0 || len(hours) > 0
}


// mapsLink opens Google Maps on a business: on its coordinates when they
// came, else on its address.
func mapsLink(address string, lat, lon float64) string {
	q := address
	if lat != 0 || lon != 0 {
		q = fmt.Sprintf("%f,%f", lat, lon)
	}
	return "https://www.google.com/maps/search/?api=1&query=" + url.QueryEscape(q)
}

// setVerified keeps a business's verification level, as GetUserInfo has
// it: WhatsApp Web's check goes by the name of the "high" ones.
func (b *Bridge) setVerified(chat string, vn *types.VerifiedName) {
	level := "none"
	if vn != nil && vn.VerifiedLevel != "" {
		level = vn.VerifiedLevel
	}
	var old string
	b.db.QueryRow(`SELECT verified FROM chats WHERE jid = ?`, chat).Scan(&old)
	if old != level {
		b.db.Exec(`UPDATE chats SET verified = ? WHERE jid = ?`, level, chat)
		b.dirty.Store(true)
	}
}

// setBizName keeps a business's verified name on its chat: the list, the
// header and its messages go by it when you haven't saved one.
func (b *Bridge) setBizName(chat, name string) {
	if name == "" {
		return
	}
	res, err := b.db.Exec(`UPDATE chats SET biz_name = ? WHERE jid = ? AND biz_name != ?`, name, chat, name)
	if err != nil {
		return
	}
	if n, _ := res.RowsAffected(); n > 0 {
		b.dirty.Store(true)
	}
}

// bizName is the verified name a chat keeps for its business, "" when none.
func (b *Bridge) bizName(j types.JID) string {
	var n string
	b.db.QueryRow(`SELECT biz_name FROM chats WHERE jid = ?`, j.String()).Scan(&n)
	return n
}

// savedContactName is the name you gave someone, "" when none: not their
// push name, nor a business's.
func (b *Bridge) savedContactName(j types.JID) string {
	if !b.stored() {
		return ""
	}
	c, err := b.cli.Store.Contacts.GetContact(b.ctx, j)
	if err != nil || c.FullName == "" {
		return c.FirstName
	}
	return c.FullName
}

// wantBizNames asks for a round of bizNames, unless one is due already.
func (b *Bridge) wantBizNames() {
	select {
	case b.bizDue <- struct{}{}:
	default:
	}
}

// bizNames asks WhatsApp about the people chats you haven't named that
// were never asked: businesses answer with their verified name, which
// WhatsApp Web shows in place of the number. Each chat once (the level
// kept, "none" for people, says it was), fifty to a request, a few seconds
// apart. A round comes on connecting and as new chats do (wantBizNames).
func (b *Bridge) bizNames() {
	for range b.bizDue {
		// A history sync brings chats by the hundred: one round for them.
		time.Sleep(10 * time.Second)
		select {
		case <-b.bizDue:
		default:
		}
		if b.cli.IsConnected() && b.stored() {
			b.askBizNames()
		}
	}
}

func (b *Bridge) askBizNames() {
	rows, err := b.db.Query(`SELECT jid FROM chats WHERE verified = '' AND jid LIKE '%@s.whatsapp.net'`)
	if err != nil {
		return
	}
	me := b.me()
	var jids []types.JID
	for rows.Next() {
		var s string
		if rows.Scan(&s) == nil {
			if j, err := types.ParseJID(s); err == nil && j != types.PSAJID && j.User != me.User {
				jids = append(jids, j)
			}
		}
	}
	rows.Close()
	// Named by you: no need to ask.
	jids = slices.DeleteFunc(jids, func(j types.JID) bool { return b.savedContactName(j) != "" })
	for i := 0; i < len(jids); i += 50 {
		if i > 0 {
			time.Sleep(3 * time.Second)
		}
		batch := jids[i:min(i+50, len(jids))]
		ctx, cancel := context.WithTimeout(b.ctx, 30*time.Second)
		info, err := b.cli.GetUserInfo(ctx, batch)
		cancel()
		if err != nil {
			// The rest on the next round.
			logf("business names: %v", err)
			return
		}
		// By number, however the answer names them; one that didn't come
		// back is no business either.
		got := map[string]*types.VerifiedName{}
		for j, u := range info {
			got[b.norm(j).String()] = u.VerifiedName
		}
		for _, j := range batch {
			vn := got[j.String()]
			b.setVerified(j.String(), vn)
			if vn != nil {
				b.setBizName(j.String(), vn.Details.GetVerifiedName())
			} else if c, err := b.cli.Store.Contacts.GetContact(b.ctx, j); err == nil && c.BusinessName != "" {
				// A business without a verified name (one on the Cloud API,
				// say) only has the name left in the contacts.
				b.setBizName(j.String(), c.BusinessName)
			}
		}
	}
}

var askedVerified sync.Map

// verifiedOnce asks, the first time a business's chat opens, whether its
// name gets the check; its messages say so too as they come.
func (b *Bridge) verifiedOnce(chat string) {
	jid, err := types.ParseJID(chat)
	if err != nil || jid.Server != types.DefaultUserServer || jid == types.PSAJID || !b.stored() {
		return
	}
	if _, busy := askedVerified.LoadOrStore(chat, true); busy {
		return
	}
	var level string
	if b.db.QueryRow(`SELECT verified FROM chats WHERE jid = ?`, chat).Scan(&level) != nil || level != "" {
		return
	}
	// Only businesses: their verified name is in the contact store.
	if c, err := b.cli.Store.Contacts.GetContact(b.ctx, jid); err != nil || c.BusinessName == "" {
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
	defer cancel()
	info, err := b.cli.GetUserInfo(ctx, []types.JID{jid})
	if err != nil {
		logf("verified %s: %v", jid, err)
		askedVerified.Delete(chat)
		return
	}
	for _, u := range info {
		b.setVerified(chat, u.VerifiedName)
		if u.VerifiedName != nil {
			b.setBizName(chat, u.VerifiedName.Details.GetVerifiedName())
		}
	}
}
