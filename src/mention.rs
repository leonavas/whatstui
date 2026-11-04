//! The @ list in a group's message box, as WhatsApp Web's: "@" and part of
//! a name offers "todos" (everyone in the group) first, then the members
//! that match; ↑ ↓ pick, Enter or Tab take it, Esc hides it. "@Name" in the
//! box stands for "@number" in what's sent (the hidden id when the number
//! isn't known), and "@todos" for "@all", which the bridge sends as
//! WhatsApp's mention of everyone.

use crate::app::{App, Focus};
use crate::bridge::Member;
use crate::ui::{box_corners, draw_deco, phone, truncate, DAY_PILL, META_IN, SELECTED, TEXT};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

/// Rows shown at once; the rest scroll.
const ROWS: usize = 6;
/// What "@all" reads as, in the box and in the chat.
pub const EVERYONE: &str = "todos";

/// One row of the list: everyone, or a member.
#[derive(Clone)]
enum Choice {
    All,
    One(Member),
}

#[derive(Default)]
pub struct Picker {
    /// Each group's members, as the bridge last said.
    members: HashMap<String, Vec<Member>>,
    /// Groups whose members were asked for.
    asked: HashSet<String>,
    /// Per chat, what each "@Name" picked in the box goes out as.
    names: HashMap<String, Vec<(String, String)>>,
    sel: usize,
    /// The first row on screen.
    off: usize,
    /// The fragment the selection is for.
    query: String,
    /// Esc hides it until the fragment changes.
    dismissed: Option<String>,
    // Where it was on the last draw, for the mouse.
    area: Rect,
    rows: Vec<(Rect, usize)>,
}

/// The `@fragment` right before the caret: the @ at the start or after a
/// space, then up to 30 characters with no space nor @. The @'s index
/// (chars) and the text after it.
fn fragment(text: &str, caret: usize) -> Option<(usize, String)> {
    let before: Vec<char> = text.chars().take(caret).collect();
    let mut i = before.len();
    while i > 0 && !before[i - 1].is_whitespace() && before[i - 1] != '@' {
        i -= 1;
    }
    if i == 0 || before[i - 1] != '@' || (i >= 2 && !before[i - 2].is_whitespace()) || before.len() - i > 30 {
        return None;
    }
    Some((i - 1, before[i..].iter().collect()))
}

/// The name a member goes by in the box: the saved one, else their own
/// (without the "~"), else the number.
fn label(m: &Member) -> String {
    match m.name.trim_start_matches('~') {
        "" => phone(&m.jid),
        n if n.starts_with('+') => phone(&m.jid),
        n => n.to_string(),
    }
}

impl App {
    /// A group's members arrived.
    pub(crate) fn members_arrived(&mut self, chat: String, members: Vec<Member>) {
        self.mention.members.insert(chat, members);
    }

    /// The rows for what's typed, everyone first; None while it doesn't show.
    fn mention_choices(&mut self) -> Option<Vec<Choice>> {
        let chat = self.open.clone().filter(|c| c.ends_with("@g.us"))?;
        if self.focus != Focus::Input || self.input_area.height == 0 || self.emoji_panel.open || self.msg_menu.editing.is_some() {
            return None;
        }
        let Some((_, q)) = fragment(&self.input, self.caret) else {
            self.mention.dismissed = None;
            return None;
        };
        if self.mention.dismissed.as_deref() == Some(q.as_str()) {
            return None;
        }
        self.mention.dismissed = None;
        if self.mention.asked.insert(chat.clone()) {
            self.cmd(json!({"c": "members", "chat": chat}));
        }
        if self.mention.query != q {
            self.mention.query = q.clone();
            self.mention.sel = 0;
            self.mention.off = 0;
        }
        let members = self.mention.members.get(&chat)?;
        let q = crate::emoji::fold(&q);
        let mut out: Vec<Choice> = members
            .iter()
            .filter(|m| q.is_empty() || crate::emoji::fold(&m.name).contains(&q) || m.phone.contains(&q))
            .cloned()
            .map(Choice::One)
            .collect();
        if !members.is_empty() && (EVERYONE.starts_with(&q) || "all".starts_with(&q)) {
            out.insert(0, Choice::All);
        }
        (!out.is_empty()).then_some(out)
    }

    /// Replaces the `@fragment` with "@Name " and keeps what it stands for.
    fn mention_pick(&mut self, c: &Choice) {
        let (Some(chat), Some((start, _))) = (self.open.clone(), fragment(&self.input, self.caret)) else { return };
        let (name, sent) = match c {
            Choice::All => (EVERYONE.to_string(), "@all".to_string()),
            Choice::One(m) => (label(m), format!("@{}", if m.user.is_empty() { m.jid.split('@').next().unwrap_or_default() } else { m.user.as_str() })),
        };
        let shown = format!("@{name}");
        let chars: Vec<char> = self.input.chars().collect();
        let after: String = chars[self.caret..].iter().collect();
        self.input = chars[..start].iter().collect::<String>() + &shown + " " + &after;
        self.caret = start + shown.chars().count() + 1;
        let names = self.mention.names.entry(chat).or_default();
        names.retain(|(l, _)| *l != shown);
        names.push((shown, sent));
    }

    /// What's sent for what's typed: each "@Name" picked as "@number",
    /// "@todos" as "@all"; the longest names first, so one isn't cut by
    /// another that starts it.
    pub(crate) fn mention_text(&mut self, chat: &str, text: String) -> String {
        let Some(mut names) = self.mention.names.remove(chat) else { return text };
        names.sort_by_key(|(l, _)| std::cmp::Reverse(l.chars().count()));
        names.iter().fold(text, |t, (l, s)| t.replace(l.as_str(), s))
    }

    /// Keys for the list, ahead of the message box's; true when it took the key.
    pub fn mention_key(&mut self, k: KeyEvent) -> bool {
        let Some(list) = self.mention_choices() else { return false };
        let n = list.len();
        let plain = !k.modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::CONTROL);
        let p = &mut self.mention;
        match k.code {
            KeyCode::Up => p.sel = (p.sel + n - 1) % n,
            KeyCode::Down => p.sel = (p.sel + 1) % n,
            KeyCode::Enter | KeyCode::Tab if plain => {
                let c = list[p.sel.min(n - 1)].clone();
                self.mention_pick(&c);
            }
            KeyCode::Esc => p.dismissed = Some(p.query.clone()),
            _ => return false,
        }
        true
    }

    /// A click on the list: that one goes in; true when it took the click.
    pub fn mention_click(&mut self, m: MouseEvent) -> bool {
        let at = Position::new(m.column, m.row);
        if m.kind != MouseEventKind::Down(MouseButton::Left) || !self.mention.area.contains(at) {
            return false;
        }
        let Some(list) = self.mention_choices() else { return false };
        if let Some(c) = self.mention.rows.iter().find(|(r, _)| r.contains(at)).and_then(|(_, i)| list.get(*i)).cloned() {
            self.mention_pick(&c);
        }
        true
    }
}

/// The list: a rounded box over the message box, left-aligned with it, a
/// name a row and what's under it in WhatsApp dim at its right; the
/// selected row lit.
pub fn draw(f: &mut Frame, app: &mut App) {
    app.mention.area = Rect::default();
    app.mention.rows.clear();
    let input = app.input_area;
    let Some(list) = app.mention_choices() else { return };
    let n = list.len();
    let k = n.min(ROWS);
    let h = k as u16 + 2;
    let Some(y) = input.y.checked_sub(h + 1).filter(|&y| y >= app.msgs_area.y) else { return };
    let x = input.x + 1;
    let w = 52.min(input.width.saturating_sub(2) as usize);
    if w < 20 {
        return;
    }
    let p = &mut app.mention;
    p.sel = p.sel.min(n - 1);
    p.off = p.off.min(p.sel).max((p.sel + 1).saturating_sub(k)).min(n - k);

    let rounded = app.picker.is_some();
    let bg = Style::new().bg(DAY_PILL);
    let edge = if rounded { Style::new() } else { bg };
    let blank = Line::from(vec![Span::styled(" ", edge), Span::styled(" ".repeat(w - 2), bg), Span::styled(" ", edge)]);
    let mut lines = vec![blank.clone()];
    for (j, c) in list.iter().enumerate().skip(p.off).take(k) {
        let lit = if j == p.sel { Style::new().bg(SELECTED) } else { bg };
        let (name, sub) = match c {
            Choice::All => (format!("@{EVERYONE}"), "Mencione todos os membros do grupo".to_string()),
            Choice::One(m) => {
                let sub = if m.name.starts_with('~') || m.name.is_empty() { phone(&m.jid) } else { String::new() };
                (label(m), sub)
            }
        };
        let name = truncate(&name, w.saturating_sub(6));
        let room = w.saturating_sub(name.width() + 8);
        let sub = if room > 4 { truncate(&sub, room) } else { String::new() };
        let pad = w.saturating_sub(5 + name.width() + sub.width());
        lines.push(Line::from(vec![
            Span::styled(" ", bg),
            Span::styled(" ", lit),
            Span::styled(name, lit.fg(TEXT)),
            Span::styled(" ".repeat(pad), lit),
            Span::styled(sub, lit.fg(META_IN)),
            Span::styled("  ", lit),
            Span::styled(" ", bg),
        ]));
        p.rows.push((Rect::new(x, y + 1 + (j - p.off) as u16, w as u16, 1), j));
    }
    lines.push(blank);
    let area = Rect::new(x, y, w as u16, h);
    p.area = area;
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(Paragraph::new(lines), area);
    if rounded {
        for (row, col, d) in box_corners(0, h as usize, 0, w, DAY_PILL, 0) {
            draw_deco(f, app, d, x + col as u16, y + row as u16);
        }
    }
}
