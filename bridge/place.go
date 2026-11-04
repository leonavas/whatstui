package main

// Shared places: where, what the place is called and its address, and the
// small map the phone sends with it. The TUI shows the map's picture and
// the name, and opens the place in Google Maps.

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode"

	"go.mau.fi/whatsmeow/proto/waE2E"
)

// Place is a shared place as the TUI sees it.
type Place struct {
	Lat  float64 `json:"lat"`
	Lon  float64 `json:"lon"`
	Name string  `json:"name,omitempty"`
	Addr string  `json:"addr,omitempty"`
	// What a click opens: Google Maps at the place, else the link it came with.
	URL string `json:"url,omitempty"`
	// The small map the phone sent, on disk; empty when none came.
	Thumb string `json:"thumb,omitempty"`
}

// placeOf is a message's place, with the map's picture as it came; nil
// when it's no place.
func placeOf(m *waE2E.Message) (*Place, []byte) {
	if x := m.GetLocationMessage(); x != nil {
		p := &Place{Lat: x.GetDegreesLatitude(), Lon: x.GetDegreesLongitude(), Name: x.GetName(), Addr: x.GetAddress(), URL: x.GetURL()}
		if p.Name == "" && x.GetComment() != "" {
			p.Name = x.GetComment()
		}
		return p, x.GetJPEGThumbnail()
	}
	if x := m.GetLiveLocationMessage(); x != nil {
		return &Place{Lat: x.GetDegreesLatitude(), Lon: x.GetDegreesLongitude()}, x.GetJPEGThumbnail()
	}
	return nil, nil
}

// mapsURL is Google Maps at the place; without its coordinates, the link it
// came with, if it's a web page.
func (p *Place) mapsURL() string {
	if p.Lat != 0 || p.Lon != 0 {
		return fmt.Sprintf("https://maps.google.com/maps?q=%f,%f&z=17&hl=%s", p.Lat, p.Lon, tr("en", "pt-BR"))
	}
	if (strings.HasPrefix(p.URL, "https://") || strings.HasPrefix(p.URL, "http://")) && strings.IndexFunc(p.URL, unicode.IsControl) == -1 {
		return p.URL
	}
	return ""
}

// placeThumb is where a place's map picture goes, beside the media.
func placeThumb(chat, id string) string {
	name := strings.NewReplacer("@", "_", ".", "_", ":", "_", "/", "_").Replace(chat + "_" + id)
	return filepath.Join(mediaDir(), name+".place.jpg")
}

// storePlace keeps a message's place and returns it as shown; nil when it
// has none.
func (b *Bridge) storePlace(chat, id string, m *waE2E.Message) *Place {
	p, thumb := placeOf(m)
	if p == nil {
		return nil
	}
	b.db.Exec(`INSERT INTO places (chat, id, lat, lon, name, addr, url, thumb) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT (chat, id) DO UPDATE SET lat = excluded.lat, lon = excluded.lon, name = excluded.name, addr = excluded.addr,
			url = excluded.url, thumb = coalesce(excluded.thumb, thumb)`,
		chat, id, p.Lat, p.Lon, p.Name, p.Addr, p.URL, thumb)
	p.shown(chat, id, thumb)
	return p
}

// shown makes it ready for the TUI: the link to open, the picture on disk.
func (p *Place) shown(chat, id string, thumb []byte) {
	p.URL = p.mapsURL()
	path := placeThumb(chat, id)
	if _, err := os.Stat(path); err == nil {
		touch(path)
		p.Thumb = path
	} else if len(thumb) > 0 && os.MkdirAll(filepath.Dir(path), 0o700) == nil && os.WriteFile(path, thumb, 0o600) == nil {
		p.Thumb = path
	}
}

// withPlaces fills in the messages' places, a query per message that is one
// (by its text, as the bridge writes it).
func (b *Bridge) withPlaces(ms []Msg) {
	for i := range ms {
		m := &ms[i]
		if m.Media != nil || !strings.HasPrefix(m.Text, "📍") {
			continue
		}
		p := &Place{}
		var thumb []byte
		if b.db.QueryRow(`SELECT lat, lon, name, addr, url, thumb FROM places WHERE chat = ? AND id = ?`, m.Chat, m.ID).
			Scan(&p.Lat, &p.Lon, &p.Name, &p.Addr, &p.URL, &thumb) != nil {
			continue
		}
		p.shown(m.Chat, m.ID, thumb)
		m.Place = p
	}
}
