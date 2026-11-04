//! A message's info, as WhatsApp Web's "Dados da mensagem": for one of
//! yours, who got it, read it and played it (a voice note), and when. In a
//! group each stage lists its people and how many it hasn't reached yet; in
//! a chat of two, just the times. Asked of the bridge again whenever its
//! ticks move while it's open.

use crate::app::App;
use crate::attach::MENU_BG;
use crate::bridge::{Msg, Receipt};
use crate::chatmenu::boxed;
use crate::ui::{day_label, display_name, local, truncate, wrap, LINK_IN, META_IN, TEXT};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use serde_json::json;
use unicode_width::UnicodeWidthStr;

const TICKS: &str = "✓✓";
const MIC: &str = "\u{f130}";

/// The info open over the conversation.
pub struct Info {
    chat: String,
    id: String,
    /// None until the bridge answers.
    receipts: Option<Vec<Receipt>>,
    /// In a group, how many are in it besides you; None when not known.
    others: Option<i64>,
    /// The first row of the list shown, when it doesn't all fit.
    top: usize,
    // From the last draw.
    area: Rect,
}

impl App {
    /// "Dados da mensagem" in the message's menu.
    pub(crate) fn open_msg_info(&mut self, chat: String, id: String) {
        self.cmd(json!({"c": "msg_info", "chat": chat, "id": id}));
        self.msg_info = Some(Info { chat, id, receipts: None, others: None, top: 0, area: Rect::default() });
    }

    /// The bridge's answer, for the message open.
    pub(crate) fn msg_info_arrived(&mut self, chat: &str, id: &str, receipts: Vec<Receipt>, others: Option<i64>) {
        if let Some(i) = self.msg_info.as_mut().filter(|i| i.chat == chat && i.id == id) {
            i.receipts = Some(receipts);
            i.others = others;
        }
    }

    /// Its ticks moved (someone got or read it): asked again.
    pub(crate) fn msg_info_status(&mut self, chat: &str, ids: &[String]) {
        let Some(i) = self.msg_info.as_ref().filter(|i| i.chat == chat && ids.contains(&i.id)) else { return };
        let (chat, id) = (i.chat.clone(), i.id.clone());
        self.cmd(json!({"c": "msg_info", "chat": chat, "id": id}));
    }

    /// While it's open: ↑ ↓ scroll, Esc, q or Enter close it; it takes every key.
    pub fn msg_info_key(&mut self, k: KeyEvent) -> bool {
        let Some(i) = self.msg_info.as_mut() else { return false };
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => i.top = i.top.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => i.top += 1,
            KeyCode::PageUp => i.top = i.top.saturating_sub(10),
            KeyCode::PageDown => i.top += 10,
            KeyCode::Home => i.top = 0,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => self.msg_info = None,
            _ => {}
        }
        true
    }

    /// The wheel over it scrolls; a click away closes it.
    pub fn msg_info_mouse(&mut self, m: MouseEvent) -> bool {
        let Some(i) = self.msg_info.as_mut() else { return false };
        let inside = i.area.contains(Position::new(m.column, m.row));
        match m.kind {
            MouseEventKind::ScrollUp if inside => i.top = i.top.saturating_sub(1),
            MouseEventKind::ScrollDown if inside => i.top += 1,
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right) if !inside => self.msg_info = None,
            MouseEventKind::Down(_) => {}
            _ => return inside,
        }
        true
    }
}

/// "Hoje às 19:20", as WhatsApp says when it was read; "—" not yet.
fn when(ts: i64) -> String {
    if ts <= 0 {
        return "—".into();
    }
    let t = local(ts);
    format!("{} às {}", day_label(t), t.format("%H:%M"))
}

fn voice(m: &Msg) -> bool {
    m.media.as_ref().is_some_and(|md| md.kind == "audio" && md.ptt)
}

/// A row of the box `w` wide: `left` at its start, `right` at its end.
fn row(w: usize, left: Vec<Span<'static>>, right: Option<Span<'static>>) -> Line<'static> {
    let bg = Style::new().bg(MENU_BG);
    let used: usize = left.iter().map(|s| s.content.width()).sum::<usize>() + right.as_ref().map_or(0, |s| s.content.width());
    let mut spans = left;
    spans.push(Span::styled(" ".repeat(w.saturating_sub(used + 3)), bg));
    if let Some(r) = right {
        spans.push(r);
    }
    spans.push(Span::styled("   ", bg));
    Line::from(spans)
}

/// A stage's head: its ticks (or the microphone) and what it is.
fn head(w: usize, icon: &'static str, tint: Color, label: &str) -> Line<'static> {
    let bg = Style::new().bg(MENU_BG);
    row(w, vec![Span::styled("   ", bg), Span::styled(icon, bg.fg(tint)), Span::styled(format!("  {label}"), bg.fg(META_IN))], None)
}

/// A group's stage: who reached it, with when, and how many it hasn't;
/// nothing when nobody's in it and nobody's left (or that isn't known).
fn stage(lines: &mut Vec<Line<'static>>, w: usize, (icon, tint, label): (&'static str, Color, &str), people: &[&Receipt], at: fn(&Receipt) -> i64, remaining: Option<i64>) {
    if people.is_empty() && remaining.is_none_or(|n| n <= 0) {
        return;
    }
    let bg = Style::new().bg(MENU_BG);
    lines.push(row(w, Vec::new(), None));
    lines.push(head(w, icon, tint, label));
    for r in people {
        let time = when(at(r));
        let name = display_name(r.name.trim_start_matches('~'), &r.jid);
        let name = truncate(&name, w.saturating_sub(time.width() + 12));
        lines.push(row(w, vec![Span::styled("      ", bg), Span::styled(name, bg.fg(TEXT))], Some(Span::styled(time, bg.fg(META_IN)))));
    }
    if let Some(n) = remaining.filter(|&n| n > 0) {
        let text = if n == 1 { "1 restante".to_string() } else { format!("{n} restantes") };
        lines.push(row(w, vec![Span::styled("      ", bg), Span::styled(text, bg.fg(META_IN))], None));
    }
}

/// The box, at the top of the conversation as the delete dialog: the
/// message, then who got and read it.
pub fn draw(f: &mut Frame, app: &mut App) {
    let Some(info) = app.msg_info.as_ref() else { return };
    let msg = app.msgs.get(&info.chat).and_then(|l| l.iter().find(|m| m.id == info.id)).cloned();
    let (Some(m), true) = (msg, app.open.as_ref() == Some(&info.chat)) else {
        app.msg_info = None;
        return;
    };
    let area = app.msgs_area;
    let w = 56.min(area.width.saturating_sub(2)) as usize;
    if w < 30 || area.height < 8 {
        return;
    }
    let bg = Style::new().bg(MENU_BG);
    let group = info.chat.ends_with("@g.us");

    // The title and the message, as a quote; these stay put.
    let mut top_lines = vec![row(w, vec![Span::styled("   ", bg), Span::styled("Dados da mensagem", bg.fg(TEXT).add_modifier(Modifier::BOLD))], Some(Span::styled("Esc", bg.fg(META_IN))))];
    top_lines.push(row(w, Vec::new(), None));
    let text = crate::markup::mentions(m.text.strip_suffix(crate::msgmenu::EDITED).unwrap_or(&m.text), &m.mentions).0;
    let mut quoted = wrap(&text, w.saturating_sub(10));
    if quoted.len() > 2 {
        quoted[1] = truncate(&format!("{}…", quoted[1].trim_end()), w.saturating_sub(10));
    }
    for l in quoted.into_iter().take(2) {
        top_lines.push(row(w, vec![Span::styled("   ", bg), Span::styled("▎ ", bg.fg(LINK_IN)), Span::styled(l, bg.fg(META_IN))], None));
    }

    let mut list = Vec::new();
    let voice = voice(&m);
    match &info.receipts {
        None => {
            list.push(row(w, Vec::new(), None));
            list.push(row(w, vec![Span::styled("   Carregando…", bg.fg(META_IN))], None));
        }
        Some(rs) if group => {
            let others = info.others;
            let left = |n: usize| others.map(|o| o - n as i64);
            let played: Vec<&Receipt> = rs.iter().filter(|r| r.played > 0).collect();
            let read: Vec<&Receipt> = rs.iter().filter(|r| r.read > 0).collect();
            let delivered: Vec<&Receipt> = rs.iter().filter(|r| r.delivered > 0).collect();
            if voice {
                stage(&mut list, w, (MIC, LINK_IN, "Reproduzida por"), &played, |r| r.played, left(played.len()));
            }
            let read_only: Vec<&Receipt> = read.iter().copied().filter(|r| !voice || r.played == 0).collect();
            stage(&mut list, w, (TICKS, LINK_IN, "Lida por"), &read_only, |r| r.read, left(read.len()));
            let got: Vec<&Receipt> = delivered.iter().copied().filter(|r| r.read == 0).collect();
            stage(&mut list, w, (TICKS, META_IN, "Entregue para"), &got, |r| r.delivered, left(delivered.len()));
            if list.is_empty() {
                list.push(row(w, Vec::new(), None));
                list.push(row(w, vec![Span::styled("   Ainda não foi entregue", bg.fg(META_IN))], None));
            }
        }
        Some(rs) => {
            // A chat of two: the other one's times.
            let one = rs.first().cloned().unwrap_or_default();
            let mut moments = Vec::new();
            if voice {
                moments.push((MIC, LINK_IN, "Reproduzida", one.played));
            }
            moments.push((TICKS, LINK_IN, "Lida", one.read));
            moments.push((TICKS, META_IN, "Entregue", one.delivered));
            for (icon, tint, label, ts) in moments {
                list.push(row(w, Vec::new(), None));
                list.push(row(w, vec![Span::styled("   ", bg), Span::styled(icon, bg.fg(tint)), Span::styled(format!("  {label}"), bg.fg(TEXT))], None));
                list.push(row(w, vec![Span::styled(format!("      {}", when(ts)), bg.fg(META_IN))], None));
            }
        }
    }

    // The two blank rows `boxed` adds; what doesn't fit scrolls.
    let room = (area.height as usize).saturating_sub(top_lines.len() + 2);
    let shown = list.len().min(room);
    let top = info.top.min(list.len() - shown);
    let mut lines = top_lines;
    lines.extend(list.into_iter().skip(top).take(shown));
    let h = lines.len() as u16 + 2;
    let rect = Rect::new(area.x + (area.width - w as u16) / 2, area.y + 1, w as u16, h.min(area.height));
    if let Some(i) = app.msg_info.as_mut() {
        i.top = top;
        i.area = rect;
    }
    boxed(f, app, rect, lines);
}
