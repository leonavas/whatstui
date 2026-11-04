//! The account button at the foot of the sidebar and its menu, as in slick:
//! your picture, name and connection, pausing notifications, your profile,
//! whatstui's settings, new versions, about, and unlinking this device.

use crate::app::{App, Phase};
use crate::bridge::Profile;
use crate::config::{self, Opt};
use crate::update::{Note, State};
use crate::ui::{avatar_cells, dim, CAP_L, CAP_R, draw_avatar, phone, truncate, wrap, wrap_indexed, green, LINK_IN, TEXT};
use chrono::{Datelike, Days, Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use serde_json::json;
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

/// Nerd Font bell with a slash: notifications paused.
const PAUSED: &str = "\u{f1f6}";
const MENU_W: u16 = 40;
/// Rows the button takes at the foot of the sidebar, its top line included.
pub const BUTTON_H: u16 = 3;
const REPO: &str = "github.com/leonavas/whatstui";
/// Under a menu row whose submenu is open.
const OPENED: Color = Color::Indexed(236);

fn version() -> String {
    format!("WhatsTUI v{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Clone, Copy, PartialEq)]
enum Item {
    Sep,
    Notifications,
    Profile,
    Settings,
    Shortcuts,
    /// "Procurar atualizações"; with one installed, "Reinicie para atualizar".
    CheckUpdate,
    /// A newer version: download it, then install and restart (`update_row`).
    Update,
    About,
    Logout,
    // The notifications submenu.
    Resume,
    /// Minutes; 0 is "until tomorrow", 9 AM.
    Pause(i64),
    /// Until Monday, 9 AM.
    PauseWeek,
}

const MAIN: [Item; 9] = [
    Item::Notifications,
    Item::Sep,
    Item::Profile,
    Item::Settings,
    Item::Shortcuts,
    Item::Sep,
    Item::CheckUpdate,
    Item::About,
    Item::Logout,
];

/// The menu's rows: `MAIN`, and the update under "Procurar atualizações"
/// while there is one to get.
fn main_items(app: &App) -> Vec<Item> {
    let mut items = MAIN.to_vec();
    if app.account.update.as_ref().is_some_and(|s| !matches!(s, State::Installed(_))) {
        let at = items.iter().position(|i| *i == Item::CheckUpdate).map_or(items.len(), |n| n + 1);
        items.insert(at, Item::Update);
    }
    items
}

impl Item {
    fn in_sub(self) -> bool {
        matches!(self, Item::Resume | Item::Pause(_) | Item::PauseWeek)
    }
}

/// What "Perfil" lets you change, in its order.
#[derive(Clone, Copy, PartialEq)]
enum Act {
    Name,
    About,
    Photo,
    RemovePhoto,
}

const ACTS: [Act; 4] = [Act::Name, Act::About, Act::Photo, Act::RemovePhoto];

/// The picture types the profile photo can be picked from (what `image` reads).
const PHOTO_EXTS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    About,
}

impl Field {
    /// WhatsApp's limits, in characters.
    fn limit(self) -> usize {
        match self {
            Field::Name => 25,
            Field::About => 139,
        }
    }
}

/// Your name or recado being typed in "Perfil"; `caret` counts chars.
struct Edit {
    field: Field,
    text: String,
    caret: usize,
}

impl Edit {
    fn new(field: Field, text: &str) -> Edit {
        let text: String = text.chars().take(field.limit()).collect();
        let caret = text.chars().count();
        Edit { field, text, caret }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte(&self, at: usize) -> usize {
        self.text.char_indices().nth(at).map_or(self.text.len(), |(b, _)| b)
    }

    /// Typed or pasted at the caret, one line, up to the limit.
    fn insert(&mut self, s: &str) {
        let room = self.field.limit().saturating_sub(self.len());
        let s: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).filter(|c| !c.is_control()).take(room).collect();
        let at = self.byte(self.caret);
        self.text.insert_str(at, &s);
        self.caret += s.chars().count();
    }

    fn back(&mut self) {
        if self.caret > 0 {
            self.caret -= 1;
            let at = self.byte(self.caret);
            self.text.remove(at);
        }
    }

    fn delete(&mut self) {
        if self.caret < self.len() {
            let at = self.byte(self.caret);
            self.text.remove(at);
        }
    }
}

pub struct Popup {
    sel: Item,
    /// The notifications submenu, open, with its selected row.
    sub: Option<Item>,
    /// "Desconectar" was chosen once; choosing it again unlinks.
    confirm: bool,
}

#[derive(Default)]
pub struct Account {
    pub menu: Option<Popup>,
    /// The settings dialog, open, with its selected row in `config::opts()`.
    pub settings: Option<usize>,
    /// "Atalhos de teclado", open, scrolled down that many rows.
    pub shortcuts: Option<u16>,
    pub about: bool,
    /// "Perfil", open; what the bridge says about you once it answers.
    pub profile: Option<Option<Profile>>,
    /// The selected row of "Perfil"'s actions, in `ACTS`.
    profile_sel: usize,
    /// Your name or recado, being typed.
    edit: Option<Edit>,
    /// "Remover foto" was chosen once; choosing it again removes it.
    confirm_remove: bool,
    /// A change to your profile went out (name, about or photo): until the
    /// bridge answers.
    saving: Option<&'static str>,
    /// The file chooser is open for your photo.
    picking: bool,
    /// "Desconectar" went out: the logout that follows is ours, not the phone's.
    pub signing_out: bool,
    /// A newer version, as the menu and the button by your name show it (`update`).
    pub update: Option<State>,
    /// Asked to restart into the installed update: main execs it on the way out.
    pub restart: bool,
    // Where the last draw put things, for the mouse.
    button: Rect,
    /// The update button, inside `button`.
    update_btn: Rect,
    menu_area: Rect,
    sub_area: Rect,
    rows: Vec<(Rect, Item)>,
    dialog: Rect,
    /// The shortcuts' rows on screen, a page of them.
    pub(crate) shortcuts_page: u16,
    settings_rows: Vec<(Rect, usize)>,
    profile_rows: Vec<(Rect, Act)>,
    link: Rect,
}

impl Account {
    /// Something of it is on screen: it takes every key and click.
    fn open(&self) -> bool {
        self.menu.is_some() || self.settings.is_some() || self.shortcuts.is_some() || self.about || self.profile.is_some()
    }
}

// ---- time ------------------------------------------------------------------

/// 9 AM `days` from today, local time.
fn nine_am(days: u64) -> i64 {
    let now = Local::now();
    let at = (now.date_naive() + Days::new(days)).and_hms_opt(9, 0, 0).and_then(|t| Local.from_local_datetime(&t).earliest());
    at.map_or(now.timestamp() + 12 * 3600, |t| t.timestamp())
}

/// "até 14:30", "até amanhã, 09:00", "até 17/10, 09:00".
fn until(ts: i64) -> String {
    let Some(t) = Local.timestamp_opt(ts, 0).single() else { return String::new() };
    let today = Local::now().date_naive();
    let hhmm = t.format("%H:%M");
    match t.date_naive() {
        d if d == today => tr!(format!("until {hhmm}"), format!("até {hhmm}")),
        d if today.succ_opt() == Some(d) => tr!(format!("until tomorrow, {hhmm}"), format!("até amanhã, {hhmm}")),
        _ => tr!(format!("until {}, {hhmm}", t.format("%b %-d")), format!("até {}, {hhmm}", t.format("%d/%m"))),
    }
}

// ---- state -----------------------------------------------------------------

fn step(items: &[Item], cur: Item, delta: isize) -> Item {
    let n = items.len() as isize;
    let mut i = items.iter().position(|x| *x == cur).unwrap_or(0) as isize;
    for _ in 0..n {
        i = (i + delta).rem_euclid(n);
        if items[i as usize] != Item::Sep {
            break;
        }
    }
    items[i as usize]
}

/// The notifications submenu's rows; those without an item are only text.
fn sub_rows(app: &App) -> Vec<(Option<Item>, String)> {
    let mut rows = Vec::new();
    // Paused: until when, and a way back, before pausing anew.
    if let Some(t) = app.events.paused {
        rows.push((None, tr!(format!("{PAUSED} Paused {}", until(t)), format!("{PAUSED} Pausadas {}", until(t)))));
        rows.push((Some(Item::Sep), String::new()));
        rows.push((Some(Item::Resume), tr!("Resume notifications", "Retomar notificações").into()));
        rows.push((Some(Item::Sep), String::new()));
    }
    rows.push((None, tr!("Pause notifications for…", "Pausar notificações por…").into()));
    let pauses = [
        (Item::Pause(30), tr!("30 minutes", "30 minutos")),
        (Item::Pause(60), tr!("1 hour", "1 hora")),
        (Item::Pause(120), tr!("2 hours", "2 horas")),
        (Item::Pause(0), tr!("Until tomorrow", "Até amanhã")),
        (Item::PauseWeek, tr!("Until next week", "Até a próxima semana")),
    ];
    rows.extend(pauses.map(|(i, l)| (Some(i), l.to_string())));
    rows
}

fn sub_items(app: &App) -> Vec<Item> {
    sub_rows(app).into_iter().filter_map(|(i, _)| i).collect()
}

fn first_sub(app: &App) -> Option<Item> {
    sub_items(app).into_iter().find(|i| *i != Item::Sep)
}

/// Your WhatsApp name, else your number.
pub(crate) fn my_name(app: &App) -> String {
    if !app.me_name.is_empty() {
        return app.me_name.clone();
    }
    app.my_jid().map(|j| phone(&j)).filter(|p| !p.is_empty()).unwrap_or_else(|| tr!("You", "Você").into())
}

/// The line under your name: the connection, or the pause.
fn state(app: &App) -> String {
    let paused = app.events.paused.filter(|t| *t > Local::now().timestamp());
    match (app.phase == Phase::Ready, app.sync, paused) {
        (false, ..) => tr!("connecting…", "conectando…").into(),
        (true, Some(p), _) => tr!(format!("syncing {p}%"), format!("sincronizando {p}%")),
        (true, None, Some(t)) => tr!(format!("{PAUSED} paused {}", until(t)), format!("{PAUSED} pausado {}", until(t))),
        _ => tr!("connected", "conectado").into(),
    }
}

impl App {
    /// Your JID, once the bridge said who you are.
    pub fn my_jid(&self) -> Option<String> {
        (!self.me.is_empty()).then(|| format!("{}@s.whatsapp.net", self.me))
    }

    pub fn open_account_menu(&mut self) {
        // One popup at a time.
        self.attach.menu = None;
        self.emoji_panel.open = false;
        self.chat_menu.popup = None;
        self.account.menu = Some(Popup { sel: Item::Notifications, sub: None, confirm: false });
    }

    /// "Atalhos de teclado", over whatever else was open.
    pub fn open_shortcuts(&mut self) {
        self.attach.menu = None;
        self.emoji_panel.open = false;
        self.chat_menu.popup = None;
        let a = &mut self.account;
        (a.menu, a.settings, a.about, a.profile) = (None, None, false, None);
        a.shortcuts = Some(0);
    }

    fn choose(&mut self, item: Item) {
        let mut close = true;
        match item {
            Item::Sep => close = false,
            Item::Notifications => {
                close = false;
                let first = first_sub(self);
                if let Some(p) = self.account.menu.as_mut() {
                    (p.sel, p.sub) = (item, first);
                }
            }
            Item::Profile => self.open_my_profile(),
            Item::Settings => self.account.settings = Some(0),
            Item::Shortcuts => self.account.shortcuts = Some(0),
            // Open, so what it finds shows up under it.
            Item::CheckUpdate => close = !self.check_updates(),
            Item::Update => {
                close = false;
                self.update_click();
            }
            Item::About => self.account.about = true,
            Item::Logout => {
                if self.account.menu.as_ref().is_some_and(|p| p.confirm) {
                    self.sign_out();
                } else if let Some(p) = self.account.menu.as_mut() {
                    p.confirm = true;
                    close = false;
                }
            }
            Item::Resume => self.set_pause(None),
            Item::Pause(0) => self.set_pause(Some(nine_am(1))),
            Item::Pause(min) => self.set_pause(Some(Local::now().timestamp() + min * 60)),
            Item::PauseWeek => self.set_pause(Some(nine_am(7 - Local::now().weekday().num_days_from_monday() as u64))),
        }
        if close {
            self.account.menu = None;
        }
    }

    /// Unlinks this device: the bridge answers logged_out and exits, and the
    /// next one shows a QR code.
    fn sign_out(&mut self) {
        self.account.signing_out = true;
        self.cmd(json!({"c": "logout"}));
        self.note(tr!("Logging out…", "Desconectando…"));
    }

    /// Everything of the account that went away, on screen and on disk (the
    /// bridge wipes its own: chats, messages, pictures, downloads).
    pub(crate) fn forget_account(&mut self) {
        self.clear_search();
        self.names.clear();
        self.chats.clear();
        self.msgs.clear();
        self.open = None;
        self.cursor = None;
        self.archived = false;
        self.chat_menu.popup = None;
        self.profile = None;
        self.close_viewer();
        self.avatars.clear();
        self.events.forget();
        self.clear_drafts();
        self.emoji_panel.forget_recent();
    }

    fn open_my_profile(&mut self) {
        let Some(jid) = self.my_jid() else { return };
        let a = &mut self.account;
        (a.profile, a.profile_sel, a.edit, a.confirm_remove) = (Some(None), 0, None, false);
        self.cmd(json!({"c": "profile", "chat": jid}));
    }

    fn close_my_profile(&mut self) {
        let a = &mut self.account;
        (a.profile, a.edit, a.confirm_remove) = (None, None, false);
    }

    /// Your name or recado is being typed: a paste goes there.
    pub(crate) fn editing_profile(&self) -> bool {
        self.account.profile.is_some() && self.account.edit.is_some()
    }

    pub(crate) fn profile_paste(&mut self, s: &str) {
        if let Some(e) = self.account.edit.as_mut() {
            e.insert(s);
        }
    }

    /// One of "Perfil"'s actions, picked by its row or its key.
    fn profile_act(&mut self, act: Act) {
        if self.account.saving.is_some() {
            return;
        }
        if let Some(i) = ACTS.iter().position(|a| *a == act) {
            self.account.profile_sel = i;
        }
        let confirmed = std::mem::take(&mut self.account.confirm_remove);
        match act {
            Act::Name => self.account.edit = Some(Edit::new(Field::Name, &self.me_name)),
            Act::About => {
                let about = self.account.profile.as_ref().and_then(|p| p.as_ref()).map(|p| p.about.clone()).unwrap_or_default();
                self.account.edit = Some(Edit::new(Field::About, &about));
            }
            Act::Photo => self.pick_my_photo(),
            Act::RemovePhoto if confirmed => {
                self.account.saving = Some("photo");
                self.cmd(json!({"c": "set_photo", "text": ""}));
            }
            Act::RemovePhoto => self.account.confirm_remove = true,
        }
    }

    /// Enter on the name or recado being typed: sent to WhatsApp, unless
    /// empty or unchanged.
    fn save_edit(&mut self) {
        let Some(e) = self.account.edit.take() else { return };
        let text = e.text.trim().to_string();
        if text.is_empty() {
            self.note(if e.field == Field::Name { tr!("Your name can't be empty.", "O nome não pode ficar vazio.") } else { tr!("Your about can't be empty.", "O recado não pode ficar vazio.") });
            self.account.edit = Some(e);
            return;
        }
        let now = match e.field {
            Field::Name => self.me_name.clone(),
            Field::About => self.account.profile.as_ref().and_then(|p| p.as_ref()).map(|p| p.about.clone()).unwrap_or_default(),
        };
        if text == now {
            return;
        }
        let (cmd, what) = if e.field == Field::Name { ("set_name", "name") } else { ("set_about", "about") };
        self.account.saving = Some(what);
        self.cmd(json!({"c": cmd, "text": text}));
    }

    /// "Trocar foto": the desktop's file chooser, then the picture made
    /// ready for WhatsApp, off the UI thread (`on_my_photo`).
    fn pick_my_photo(&mut self) {
        if std::mem::replace(&mut self.account.picking, true) {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res = crate::attach::choose_one(tr!("Profile photo", "Foto do perfil"), &PHOTO_EXTS).and_then(|p| p.map(|p| square_jpeg(&p)).transpose());
            let _ = tx.send(crate::Ev::MyPhoto(res));
        });
    }

    /// The photo picked and made ready, to send; None when the chooser was cancelled.
    pub(crate) fn on_my_photo(&mut self, res: Result<Option<PathBuf>, String>) {
        self.account.picking = false;
        match res {
            Ok(Some(path)) => {
                self.account.saving = Some("photo");
                self.cmd(json!({"c": "set_photo", "text": path.to_string_lossy()}));
            }
            Ok(None) => {}
            Err(e) => self.note(e),
        }
    }

    /// How a change to your profile went: `what` is name, about or photo.
    pub(crate) fn on_profile_set(&mut self, what: &str, text: String, error: String) {
        self.account.saving = None;
        if !error.is_empty() {
            let whose = match what {
                "name" => tr!("your name", "seu nome"),
                "about" => tr!("your about", "seu recado"),
                _ => tr!("your photo", "sua foto"),
            };
            self.note(tr!(format!("couldn't change {whose}: {error}"), format!("não mudou {whose}: {error}")));
            return;
        }
        match what {
            "name" => {
                self.me_name = text;
                self.note(tr!("Name changed", "Nome alterado"));
            }
            "about" => {
                if let Some(Some(p)) = self.account.profile.as_mut() {
                    p.about = text;
                }
                self.note(tr!("About changed", "Recado alterado"));
            }
            // The new picture comes as an avatar, after.
            _ if text == "removed" => self.note(tr!("Profile photo removed", "Foto do perfil removida")),
            _ => self.note(tr!("Profile photo changed", "Foto do perfil alterada")),
        }
    }

    /// "Perfil"'s keys: the field being typed, else its actions.
    fn profile_key(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        if let Some(e) = self.account.edit.as_mut() {
            match k.code {
                KeyCode::Esc => self.account.edit = None,
                KeyCode::Enter => self.save_edit(),
                KeyCode::Backspace => e.back(),
                KeyCode::Delete => e.delete(),
                KeyCode::Left => e.caret = e.caret.saturating_sub(1),
                KeyCode::Right => e.caret = (e.caret + 1).min(e.len()),
                KeyCode::Home => e.caret = 0,
                KeyCode::End => e.caret = e.len(),
                KeyCode::Char('u') if ctrl => (e.text, e.caret) = (String::new(), 0),
                KeyCode::Char(c) if !ctrl && !alt => e.insert(&c.to_string()),
                _ => {}
            }
            return;
        }
        let (n, sel) = (ACTS.len(), self.account.profile_sel);
        let mv = |a: &mut Account, to: usize| {
            (a.profile_sel, a.confirm_remove) = (to, false);
        };
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_my_profile(),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => mv(&mut self.account, (sel + n - 1) % n),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => mv(&mut self.account, (sel + 1) % n),
            KeyCode::Enter | KeyCode::Char(' ') => self.profile_act(ACTS[sel.min(n - 1)]),
            KeyCode::Char('n') => self.profile_act(Act::Name),
            KeyCode::Char('r' | 'a') => self.profile_act(Act::About),
            KeyCode::Char('f' | 'p') => self.profile_act(Act::Photo),
            KeyCode::Char('x') => self.profile_act(Act::RemovePhoto),
            _ => {}
        }
    }

    /// The bridge's answer about you, for "Perfil"; false when it's someone else's.
    pub(crate) fn on_my_profile(&mut self, p: &Profile) -> bool {
        let mine = self.my_jid().as_deref() == Some(p.chat.as_str()) && self.account.profile.is_some();
        if mine {
            self.account.profile = Some(Some(p.clone()));
        }
        mine
    }

    /// What the update thread or a check found, or how a download or an install went.
    pub(crate) fn on_update(&mut self, note: Note) {
        let a = &mut self.account;
        match note {
            Note::Found { ver, assets } => a.update = Some(State::found(a.update.take(), ver, assets)),
            Note::Progress(p) => {
                if let Some(State::Downloading { pct, .. }) = &mut a.update {
                    *pct = Some(p);
                }
            }
            Note::Downloaded(Ok((ver, pkg))) => {
                let assets = match a.update.take() {
                    Some(State::Downloading { assets, .. }) => assets,
                    _ => vec![],
                };
                a.update = Some(State::Downloaded { ver: ver.clone(), assets, pkg });
                self.note(tr!(format!("WhatsTUI {ver} downloaded · install it from the account menu (Alt+S)"), format!("WhatsTUI {ver} baixado · instale pelo menu da conta (Alt+S)")));
            }
            Note::Downloaded(Err(e)) => {
                a.update = a.update.take().map(State::retry);
                self.note(tr!(format!("couldn't download the update: {e}"), format!("não baixou a atualização: {e}")));
            }
            Note::Installed(Ok(ver)) => {
                a.update = Some(State::Installed(ver));
                self.restart();
            }
            Note::Installed(Err(e)) => {
                a.update = a.update.take().map(State::retry);
                self.note(tr!(format!("couldn't update: {e}"), format!("não atualizou: {e}")));
            }
            Note::Checked(Ok(Some((ver, assets)))) => {
                a.update = Some(State::found(a.update.take(), ver.clone(), assets));
                // The menu still open on the check: onto what it found.
                if let Some(p) = a.menu.as_mut().filter(|p| p.sel == Item::CheckUpdate && p.sub.is_none()) {
                    p.sel = Item::Update;
                }
                match crate::update::how() {
                    Some(how) => self.note(tr!(format!("WhatsTUI {ver} available · {how}"), format!("WhatsTUI {ver} disponível · {how}"))),
                    None => self.note(tr!(format!("WhatsTUI {ver} available · download it from the account menu (Alt+S)"), format!("WhatsTUI {ver} disponível · baixe pelo menu da conta (Alt+S)"))),
                }
            }
            Note::Checked(Ok(None)) => self.note(tr!(format!("WhatsTUI is up to date ({})", crate::update::current()), format!("WhatsTUI está atualizado ({})", crate::update::current()))),
            Note::Checked(Err(e)) => self.note(tr!(format!("couldn't check for updates: {e}"), format!("não procurou atualizações: {e}"))),
        }
    }

    /// Runs `job` off the UI thread and hands its note back as an `Ev::Update`.
    fn update_job(&self, job: impl FnOnce() -> Note + Send + 'static) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::Ev::Update(job()));
        });
    }

    /// Out of the app and into the installed version (main does the exec).
    fn restart(&mut self) {
        self.account.restart = true;
        self.quit = true;
    }

    /// "Procurar atualizações": looks now; with one installed already,
    /// restarts. True when a check went out.
    fn check_updates(&mut self) -> bool {
        if matches!(self.account.update, Some(State::Installed(_))) {
            self.restart();
            return false;
        }
        if crate::update::check_disabled() {
            self.note(tr!("development build: updates are off", "build de desenvolvimento: atualizações desligadas"));
            return false;
        }
        self.note(tr!("Checking for updates…", "Procurando atualizações…"));
        self.update_job(crate::update::check_now);
        true
    }

    /// The update's row in the menu and the button by your name: download,
    /// then install and restart; or how to, when the binary isn't ours.
    fn update_click(&mut self) {
        match self.account.update.take() {
            Some(State::Available { ver, assets }) => {
                if let Some(how) = crate::update::how() {
                    self.account.update = Some(State::Available { ver, assets });
                    self.note(how);
                } else {
                    self.note(tr!(format!("downloading WhatsTUI {ver}…"), format!("baixando WhatsTUI {ver}…")));
                    self.account.update = Some(State::Downloading { ver: ver.clone(), assets: assets.clone(), pct: None });
                    let tx = self.tx.clone();
                    let progress = move |p| {
                        let _ = tx.send(crate::Ev::Update(Note::Progress(p)));
                    };
                    self.update_job(move || crate::update::download_now(ver, assets, progress));
                }
            }
            Some(State::Downloaded { ver, assets, pkg }) => {
                self.note(tr!(format!("installing WhatsTUI {ver}…"), format!("instalando WhatsTUI {ver}…")));
                self.account.update = Some(State::Installing { ver: ver.clone(), assets });
                self.update_job(move || crate::update::install_now(ver, pkg));
            }
            Some(State::Installed(ver)) => {
                self.account.update = Some(State::Installed(ver));
                self.restart();
            }
            other => self.account.update = other,
        }
    }

    /// Flips a switch; the dialog stays open. The tray follows in main's loop.
    fn choose_opt(&mut self, o: Opt) {
        if let Err(e) = self.settings.flip(o) {
            self.note(tr!(format!("couldn't save the setting: {e}"), format!("não salvou a configuração: {e}")));
        } else if o == config::PORTUGUESE {
            // Already in the new language: the helper's own texts follow on the next start.
            self.note(tr!(
                "English on. Texts from WhatsApp's side (calls, previews) switch fully after a restart.",
                "Português ativado. Textos do lado do WhatsApp (chamadas, prévias) mudam por completo após reiniciar."
            ));
        }
    }

    /// The menu and its dialogs take every key while open; Alt+S opens or
    /// closes the menu. True when taken.
    pub fn account_key(&mut self, k: KeyEvent) -> bool {
        let alt_s = k.modifiers.contains(KeyModifiers::ALT) && matches!(k.code, KeyCode::Char('s' | 'S'));
        // Ctrl+O: into the installed update, from anywhere.
        let ctrl_o = k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('o' | 'O'));
        if ctrl_o && matches!(self.account.update, Some(State::Installed(_))) {
            self.restart();
            return true;
        }
        // Ctrl+/, WhatsApp Web's, opens or closes the shortcuts from anywhere;
        // without the kitty protocol the terminal sends it as Ctrl+7.
        if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('/' | '7')) {
            if self.account.shortcuts.take().is_none() {
                self.open_shortcuts();
            }
            return true;
        }
        if !self.account.open() {
            if alt_s {
                self.open_account_menu();
            }
            return alt_s;
        }
        if self.account.profile.is_some() {
            self.profile_key(k);
        } else if self.account.about {
            match k.code {
                KeyCode::Enter => crate::links::open(&format!("https://{REPO}")),
                KeyCode::Esc | KeyCode::Char('q') => self.account.about = false,
                _ => {}
            }
        } else if let Some(top) = self.account.shortcuts {
            // Scrolled past the end comes back when drawn.
            let page = self.account.shortcuts_page.max(1);
            self.account.shortcuts = match k.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | '?') => None,
                KeyCode::Up | KeyCode::Char('k') => Some(top.saturating_sub(1)),
                KeyCode::Down | KeyCode::Char('j') => Some(top + 1),
                KeyCode::PageUp => Some(top.saturating_sub(page)),
                KeyCode::PageDown | KeyCode::Char(' ') => Some(top + page),
                KeyCode::Home | KeyCode::Char('g') => Some(0),
                KeyCode::End | KeyCode::Char('G') => Some(u16::MAX),
                _ => Some(top),
            };
        } else if let Some(sel) = self.account.settings {
            let all = config::opts();
            let n = all.len();
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => self.account.settings = None,
                KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => self.account.settings = Some((sel + n - 1) % n),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.account.settings = Some((sel + 1) % n),
                KeyCode::Enter | KeyCode::Char(' ') => self.choose_opt(all[sel]),
                _ => {}
            }
        } else {
            self.menu_key(k, alt_s);
        }
        true
    }

    fn menu_key(&mut self, k: KeyEvent, alt_s: bool) {
        let (sub, main) = (sub_items(self), main_items(self));
        let Some(p) = self.account.menu.as_mut() else { return };
        let go = matches!(k.code, KeyCode::Enter | KeyCode::Right | KeyCode::Char('l' | ' '));
        if alt_s {
            self.account.menu = None;
            return;
        }
        match p.sub {
            Some(s) => match k.code {
                KeyCode::Up | KeyCode::Char('k') => p.sub = Some(step(&sub, s, -1)),
                KeyCode::Down | KeyCode::Char('j') => p.sub = Some(step(&sub, s, 1)),
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => p.sub = None,
                _ if go => self.choose(s),
                _ => {}
            },
            None => match k.code {
                KeyCode::Up | KeyCode::Char('k') => (p.sel, p.confirm) = (step(&main, p.sel, -1), false),
                KeyCode::Down | KeyCode::Char('j') => (p.sel, p.confirm) = (step(&main, p.sel, 1), false),
                KeyCode::Esc => self.account.menu = None,
                _ if go => {
                    let sel = p.sel;
                    self.choose(sel);
                }
                _ => {}
            },
        }
    }

    /// The button, and while open, the menu and dialogs: hovering, clicks,
    /// a click away closes. True when the event was theirs.
    pub fn account_mouse(&mut self, m: MouseEvent) -> bool {
        // Only over the chats: the QR and the error screens have no button.
        if self.phase != Phase::Ready && self.chats.is_empty() {
            return false;
        }
        let at = Position::new(m.column, m.row);
        let left = m.kind == MouseEventKind::Down(MouseButton::Left);
        let down = matches!(m.kind, MouseEventKind::Down(_));
        let a = &self.account;
        let outside = !a.dialog.contains(at);
        if a.profile.is_some() {
            let hit = a.profile_rows.iter().find(|(r, _)| r.contains(at)).map(|(_, x)| *x);
            let i = hit.and_then(|x| ACTS.iter().position(|y| *y == x));
            match (m.kind, i) {
                (MouseEventKind::Moved, Some(i)) if a.edit.is_none() && i != a.profile_sel => {
                    (self.account.profile_sel, self.account.confirm_remove) = (i, false);
                }
                (_, Some(i)) if left => {
                    self.account.edit = None;
                    self.profile_act(ACTS[i]);
                }
                _ if down && outside => self.close_my_profile(),
                _ => {}
            }
            return true;
        }
        if a.about {
            if left && a.link.contains(at) {
                crate::links::open(&format!("https://{REPO}"));
            } else if down && outside {
                self.account.about = false;
            }
            return true;
        }
        if let Some(top) = a.shortcuts {
            match m.kind {
                MouseEventKind::ScrollUp => self.account.shortcuts = Some(top.saturating_sub(3)),
                MouseEventKind::ScrollDown => self.account.shortcuts = Some(top + 3),
                _ if down && outside => self.account.shortcuts = None,
                _ => {}
            }
            return true;
        }
        if a.settings.is_some() {
            let hit = a.settings_rows.iter().find(|(r, _)| r.contains(at)).map(|(_, i)| *i);
            match (m.kind, hit) {
                (MouseEventKind::Moved, Some(i)) => self.account.settings = Some(i),
                (_, Some(i)) if left => {
                    self.account.settings = Some(i);
                    self.choose_opt(config::opts()[i]);
                }
                _ if down && outside => self.account.settings = None,
                _ => {}
            }
            return true;
        }
        if a.menu.is_some() {
            let hit = a.rows.iter().find(|(r, _)| r.contains(at)).map(|(_, i)| *i);
            let inside = a.menu_area.contains(at) || a.sub_area.contains(at);
            let button = a.button.contains(at);
            let first = first_sub(self);
            let Some(p) = self.account.menu.as_mut() else { return true };
            match m.kind {
                MouseEventKind::Moved | MouseEventKind::Drag(_) => match hit {
                    Some(i) if i.in_sub() => p.sub = Some(i),
                    // As in slick, the submenu opens under the pointer.
                    Some(Item::Notifications) if p.sel != Item::Notifications || p.sub.is_none() => {
                        (p.sel, p.sub, p.confirm) = (Item::Notifications, first, false);
                    }
                    Some(i) if i != p.sel => (p.sel, p.sub, p.confirm) = (i, None, false),
                    _ => {}
                },
                _ if left && hit.is_some() => self.choose(hit.unwrap_or(Item::Sep)),
                _ if down && (button || !inside) => self.account.menu = None,
                _ => {}
            }
            return true;
        }
        // Its own button inside the account's: never opens the menu.
        if left && self.account.update_btn.contains(at) {
            self.update_click();
            return true;
        }
        if left && self.account.button.contains(at) {
            self.open_account_menu();
            return true;
        }
        false
    }
}

// ---- drawing ---------------------------------------------------------------

/// Your picture, name and the state under it.
pub fn button(f: &mut Frame, app: &mut App, area: Rect) {
    (app.account.button, app.account.update_btn) = Default::default();
    let block = Block::new().borders(Borders::TOP).border_style(dim());
    let inner = block.inner(area);
    f.render_widget(block, area);
    // The list down to its pictures (`divider`): yours alone, in line with
    // theirs, and a green arrow beside it for a newer version; a click on
    // either opens the menu, where the update is.
    if app.side_compact() && inner.height >= 2 {
        app.account.button = inner;
        let (jid, name) = (app.my_jid(), my_name(app));
        crate::divider::face(f, app, jid.as_deref(), &name, false, inner.x + 2, inner.y);
        if app.account.update.is_some() {
            let r = Rect::new(inner.x + 2 + crate::divider::slot(app), inner.y, 1, 1).intersection(inner);
            f.render_widget(Paragraph::new(Span::styled("↑", Style::new().fg(green()).bold())), r);
        }
        return;
    }
    if inner.height < 2 || inner.width < 8 {
        return;
    }
    app.account.button = inner;
    let under = state(app);
    // A newer version: a small button at the right of the name's line.
    let up = update_pill(app);
    let up_w = up.as_ref().map_or(0, |l| l.width() as u16);
    head(f, app, inner, &under, if up.is_some() { up_w + 1 } else { 0 });
    if let Some(line) = up {
        let r = Rect::new(inner.right().saturating_sub(up_w + 1).max(inner.x), inner.y, up_w, 1).intersection(inner);
        f.render_widget(Paragraph::new(line), r);
        app.account.update_btn = r;
    }
}

/// The update button: its words and colours by where the update stands.
fn update_pill(app: &App) -> Option<Line<'static>> {
    let (label, fg, bg) = match app.account.update.as_ref()? {
        State::Available { ver, .. } => (format!("↑ {ver}"), Color::Black, green()),
        // Faded while it works: there is nothing to click.
        State::Downloading { pct: Some(p), .. } => (tr!(format!("downloading {p}%"), format!("baixando {p}%")), Color::DarkGray, Color::Indexed(236)),
        State::Downloading { .. } => (tr!("downloading…", "baixando…").into(), Color::DarkGray, Color::Indexed(236)),
        State::Downloaded { .. } => (tr!("↻ install", "↻ instalar").into(), Color::Black, green()),
        State::Installing { .. } => (tr!("installing…", "instalando…").into(), Color::DarkGray, Color::Indexed(236)),
        State::Installed(_) => (tr!("↻ restart", "↻ reiniciar").into(), Color::Black, green()),
    };
    Some(Line::from(vec![
        Span::styled(CAP_L, Style::new().fg(bg)),
        Span::styled(label, Style::new().fg(fg).bg(bg).bold()),
        Span::styled(CAP_R, Style::new().fg(bg)),
    ]))
}

/// The picture, then your name over `under`, from `area`'s top left;
/// `room` cells kept free at the right of the name.
fn head(f: &mut Frame, app: &mut App, area: Rect, under: &str, room: u16) {
    let mut x = area.x + 1;
    if let (Some((cols, rows)), Some(jid)) = (avatar_cells(app), app.my_jid()) {
        draw_avatar(f, app, &jid, false, Rect::new(x, area.y, cols, rows));
        x += cols + 1;
    }
    let w = area.right().saturating_sub(x + 1);
    let name_style = if app.account.menu.is_some() { Style::new().fg(green()).bold() } else { Style::new().fg(TEXT).bold() };
    let lines = vec![
        Line::from(Span::styled(truncate(&my_name(app), w.saturating_sub(room) as usize), name_style)),
        Line::from(Span::styled(truncate(under, w as usize), dim())),
    ];
    f.render_widget(Paragraph::new(lines), Rect::new(x, area.y, w, 2));
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let a = &mut app.account;
    (a.menu_area, a.sub_area, a.dialog, a.link) = Default::default();
    a.rows.clear();
    a.settings_rows.clear();
    a.profile_rows.clear();
    if a.menu.is_some() {
        draw_menu(f, app);
    }
    if app.account.settings.is_some() {
        draw_settings(f, app);
    }
    if app.account.shortcuts.is_some() {
        crate::shortcuts::draw(f, app);
    }
    if app.account.about {
        draw_about(f, app);
    }
    if app.account.profile.is_some() {
        draw_profile(f, app);
    }
}

/// ` left      right `, filling `w` cells.
fn padded(left: &str, right: &str, w: usize) -> String {
    let left = truncate(left, w.saturating_sub(right.width() + 3));
    let gap = w.saturating_sub(left.width() + right.width() + 2);
    format!(" {left}{}{right} ", " ".repeat(gap))
}

/// The update's row: its words, the mark at its right, and whether it's
/// clickable (else faded, while it works).
fn update_row(app: &App) -> (String, String, bool) {
    let Some(s) = &app.account.update else { return Default::default() };
    match s {
        State::Available { ver, .. } => match crate::update::how_short() {
            Some(how) => (format!("{how} (v{ver})"), String::new(), true),
            None => (tr!(format!("Download update (v{ver})"), format!("Baixar atualização (v{ver})")), "↓".into(), true),
        },
        State::Downloading { pct: Some(p), .. } => (tr!(format!("Downloading… {p}%"), format!("Baixando… {p}%")), crate::media::spin().into(), false),
        State::Downloading { .. } => (tr!("Downloading…", "Baixando…").into(), crate::media::spin().into(), false),
        State::Downloaded { ver, .. } => (tr!(format!("Install and restart (v{ver})"), format!("Instalar e reiniciar (v{ver})")), "↻".into(), true),
        State::Installing { .. } => (tr!("Installing…", "Instalando…").into(), crate::media::spin().into(), false),
        State::Installed(_) => Default::default(),
    }
}

fn draw_menu(f: &mut Frame, app: &mut App) {
    let Some(p) = &app.account.menu else { return };
    let (sel, sub, confirm) = (p.sel, p.sub, p.confirm);
    let items = main_items(app);
    let area = f.area();
    let b = app.account.button;
    // You, a gap and the items.
    let (w, h) = (MENU_W.min(area.width), (items.len() as u16 + 5).min(area.height));
    // Over the button's top line, as if it grew out of it.
    let rect = Rect::new(b.x, b.y.saturating_sub(h).max(area.y), w, h).intersection(area);
    f.render_widget(Clear, rect);
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(dim());
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.account.menu_area = rect;
    if inner.height < 3 {
        return;
    }
    let under = state(app);
    head(f, app, inner, &under, 0);

    let iw = inner.width as usize;
    let mut sub_y = inner.y;
    for (n, item) in items.iter().enumerate() {
        let row = Rect::new(inner.x, inner.y + 3 + n as u16, inner.width, 1);
        if row.y >= inner.bottom() {
            break;
        }
        if *item == Item::Sep {
            f.render_widget(Paragraph::new("─".repeat(iw)).style(dim()), row);
            continue;
        }
        let mut style = if *item == sel { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() };
        let label = match item {
            Item::Notifications => {
                if *item == sel {
                    sub_y = row.y;
                    if sub.is_some() {
                        style = Style::new().bg(OPENED);
                    }
                }
                let right = if app.paused() { format!("{PAUSED} ›") } else { "›".into() };
                padded(tr!("Notifications", "Notificações"), &right, iw)
            }
            Item::Profile => padded(tr!("Profile", "Perfil"), "", iw),
            Item::Settings => padded(tr!("Settings", "Configurações"), "", iw),
            Item::Shortcuts => padded(tr!("Keyboard shortcuts", "Atalhos de teclado"), "Ctrl+/", iw),
            Item::CheckUpdate if matches!(app.account.update, Some(State::Installed(_))) => padded(tr!("Restart to update", "Reinicie para atualizar"), "↻", iw),
            Item::CheckUpdate => padded(tr!("Check for updates", "Procurar atualizações"), "", iw),
            // In the accent, as what's new; faded while it works.
            Item::Update => {
                let (label, right, live) = update_row(app);
                style = if live { style.fg(green()).bold() } else { style.patch(dim()) };
                padded(&label, &right, iw)
            }
            Item::About => padded(tr!("About WhatsTUI", "Sobre o WhatsTUI"), "", iw),
            Item::Logout if confirm => {
                style = style.fg(Color::Yellow).bold();
                padded(tr!("Log out? Enter confirms", "Desconectar? Enter confirma"), "", iw)
            }
            _ => padded(tr!("Log out", "Desconectar"), "", iw),
        };
        f.render_widget(Paragraph::new(Line::from(label).style(style)), row);
        app.account.rows.push((row, *item));
    }
    if let Some(s) = sub {
        draw_sub(f, app, rect, sub_y, s);
    }
}

/// The notifications submenu, beside the menu.
fn draw_sub(f: &mut Frame, app: &mut App, main: Rect, y: u16, sel: Item) {
    let rows = sub_rows(app);
    let area = f.area();
    let w = (rows.iter().map(|(_, l)| l.width() + 4).max().unwrap_or(20) as u16).max(30).min(area.width);
    let h = (rows.len() as u16 + 2).min(area.height);
    let x = main.right().min(area.right().saturating_sub(w));
    let y = y.saturating_sub(1).min(area.bottom().saturating_sub(h));
    let rect = Rect::new(x, y, w, h);
    f.render_widget(Clear, rect);
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(dim());
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.account.sub_area = rect;
    let iw = inner.width as usize;
    let paused = app.paused();
    for (n, (item, label)) in rows.iter().enumerate() {
        let row = Rect::new(inner.x, inner.y + n as u16, inner.width, 1);
        if row.y >= inner.bottom() {
            break;
        }
        let line = match item {
            Some(Item::Sep) => Line::from("─".repeat(iw)).style(dim()),
            None => {
                let style = if paused && n == 0 { Style::new().fg(Color::Yellow) } else { dim() };
                Line::from(padded(label, "", iw)).style(style)
            }
            Some(i) => {
                app.account.rows.push((row, *i));
                let style = if *i == sel { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() };
                Line::from(padded(label, "", iw)).style(style)
            }
        };
        f.render_widget(Paragraph::new(line), row);
    }
}

/// A rounded box in the middle of the screen, cleared, with its title; it
/// and the space inside, a column in from each side.
pub(crate) fn dialog(f: &mut Frame, app: &mut App, w: u16, h: u16, title: &str) -> (Rect, Rect) {
    let area = f.area();
    let (w, h) = (w.min(area.width), h.min(area.height));
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, rect);
    let mut block = Block::bordered().border_type(BorderType::Rounded).border_style(dim());
    if !title.is_empty() {
        block = block.title(Line::from(title.to_string()).bold());
    }
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);
    app.account.dialog = rect;
    (rect, inner)
}

/// whatstui's settings: a heading over each group of `config::SECTIONS`, its switches under it.
fn draw_settings(f: &mut Frame, app: &mut App) {
    let Some(sel) = app.account.settings else { return };
    let mut lines: Vec<(Option<usize>, Line)> = vec![(None, Line::default())];
    let mut i = 0;
    for (n, (heading, opts)) in config::SECTIONS.iter().enumerate() {
        if n > 0 {
            lines.push((None, Line::default()));
        }
        lines.push((None, Line::from(format!(" {}", tr!(heading.0, heading.1))).style(Style::new().fg(green()).bold())));
        for o in *opts {
            let mark = if app.settings.on(*o) { "[✓]" } else { "[ ]" };
            lines.push((Some(i), Line::from(format!("{mark} {}", o.label()))));
            i += 1;
        }
    }
    lines.push((None, Line::default()));
    lines.push((None, Line::from(tr!(" ↑↓ move · Enter selects · Esc closes", " ↑↓ move · Enter escolhe · Esc fecha")).style(dim())));
    lines.push((None, Line::from(format!(" {}", version())).style(dim())));

    let content_w = lines.iter().map(|(_, l)| l.width()).max().unwrap_or(0) as u16;
    let (_, inner) = dialog(f, app, (content_w + 8).max(44), lines.len() as u16 + 2, tr!(" Settings ", " Configurações "));
    for (n, (opt, line)) in lines.into_iter().enumerate() {
        let row = Rect::new(inner.x, inner.y + n as u16, inner.width, 1);
        if row.y >= inner.bottom() {
            break;
        }
        let Some(i) = opt else {
            f.render_widget(Paragraph::new(line), row);
            continue;
        };
        let mark = if i == sel { Span::styled("› ", Style::new().fg(green()).bold()) } else { Span::raw("  ") };
        let style = if i == sel { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() };
        let mut spans = vec![mark];
        spans.extend(line.spans.into_iter().map(|s| s.patch_style(style)));
        f.render_widget(Paragraph::new(Line::from(spans)), row);
        app.account.settings_rows.push((row, i));
    }
}

/// "Sobre o WhatsTUI": the name, the version and where the code is.
fn draw_about(f: &mut Frame, app: &mut App) {
    let hint = tr!("Enter opens the link · Esc closes", "Enter abre o link · Esc fecha");
    let lines = vec![
        Line::default(),
        Line::from("WhatsTUI").style(Style::new().fg(green()).bold()),
        Line::from(tr!(format!("version {}", env!("CARGO_PKG_VERSION")), format!("versão {}", env!("CARGO_PKG_VERSION")))).style(dim()),
        Line::default(),
        Line::from(REPO).style(Style::new().fg(LINK_IN).underlined()),
        Line::default(),
        Line::from(hint).style(dim()),
    ];
    let w = REPO.width().max(hint.width()) as u16 + 8;
    let (_, inner) = dialog(f, app, w, lines.len() as u16 + 2, "");
    let link_y = inner.y + 4;
    f.render_widget(Paragraph::new(lines).centered(), inner);
    let lw = REPO.width() as u16;
    let link = Rect::new(inner.x + (inner.width / 2).saturating_sub(lw / 2), link_y, lw, 1).intersection(inner);
    app.account.link = link;
    // The terminal's own Ctrl+click too.
    crate::links::osc8(f.buffer_mut(), link, &format!("https://{REPO}"), |c| c.modifier.contains(Modifier::UNDERLINED));
}

/// The picture at `path` as WhatsApp keeps profile photos: upright, cropped
/// square from the middle, 640px, a JPEG in the cache for the bridge to
/// send (it removes it).
fn square_jpeg(path: &Path) -> Result<PathBuf, String> {
    use image::ImageDecoder;
    let bad = |e: String| tr!(format!("couldn't read the image: {e}"), format!("não leu a imagem: {e}"));
    let reader = image::ImageReader::open(path).and_then(|r| r.with_guessed_format()).map_err(|e| bad(e.to_string()))?;
    let mut dec = reader.into_decoder().map_err(|e| bad(e.to_string()))?;
    let orientation = dec.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(dec).map_err(|e| bad(e.to_string()))?;
    img.apply_orientation(orientation);
    let img = img.resize_to_fill(640, 640, image::imageops::FilterType::Lanczos3).to_rgb8();
    let dir = crate::bridge::cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let to = dir.join(format!("profile-photo-{}.jpg", std::process::id()));
    let file = std::fs::File::create(&to).map_err(|e| tr!(format!("profile photo: {e}"), format!("foto do perfil: {e}")))?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 90)
        .encode_image(&img)
        .map_err(|e| tr!(format!("profile photo: {e}"), format!("foto do perfil: {e}")))?;
    Ok(to)
}

/// The text being typed, wrapped to `width`, with the caret on it.
fn edit_lines(e: &Edit, width: usize) -> Vec<Line<'static>> {
    let rows = wrap_indexed(&e.text, width.max(1));
    // The row and column of the caret: on the char at `e.caret`, or past the end.
    let at = rows.iter().enumerate().find_map(|(r, (_, idx))| idx.iter().position(|i| *i >= e.caret).map(|c| (r, c)));
    let last = rows.len().saturating_sub(1);
    let (cr, cc) = at.unwrap_or_else(|| (last, rows.get(last).map_or(0, |(l, _)| l.chars().count())));
    let caret = Style::new().add_modifier(Modifier::REVERSED);
    rows.into_iter()
        .enumerate()
        .map(|(r, (l, _))| {
            if r != cr {
                return Line::from(format!(" {l}"));
            }
            let chars: Vec<char> = l.chars().collect();
            let before: String = chars[..cc.min(chars.len())].iter().collect();
            let (on, after) = match chars.get(cc) {
                Some(c) => (c.to_string(), chars[cc + 1..].iter().collect()),
                None => (" ".to_string(), String::new()),
            };
            Line::from(vec![Span::raw(format!(" {before}")), Span::styled(on, caret), Span::raw(after)])
        })
        .collect()
}

/// "Perfil": your picture, name and number, your name and recado to
/// change, and your photo to change or remove.
fn draw_profile(f: &mut Frame, app: &mut App) {
    let Some(data) = app.account.profile.clone() else { return };
    let w = 52u16.min(f.area().width.saturating_sub(4));
    let iw = w.saturating_sub(4) as usize;
    let about = match &data {
        None => tr!("loading…", "carregando…").to_string(),
        Some(p) if !p.error.is_empty() => p.error.clone(),
        Some(p) if p.about.is_empty() => "—".into(),
        Some(p) => p.about.clone(),
    };
    let a = &app.account;
    let edit = a.edit.as_ref();
    // A heading, with how much is typed while it's being changed.
    let heading = |label: &str, field: Field| {
        let mut spans = vec![Span::styled(label.to_string(), Style::new().fg(green()).bold())];
        if let Some(e) = edit.filter(|e| e.field == field) {
            spans.push(Span::styled(format!("  {}/{}", e.len(), field.limit()), dim()));
        }
        Line::from(spans)
    };
    let mut lines = vec![heading(tr!("Name", "Nome"), Field::Name)];
    match edit.filter(|e| e.field == Field::Name) {
        Some(e) => lines.extend(edit_lines(e, iw.saturating_sub(2))),
        None => lines.push(Line::from(format!(" {}", truncate(&my_name(app), iw.saturating_sub(1))))),
    }
    lines.push(heading(tr!("About", "Recado"), Field::About));
    match edit.filter(|e| e.field == Field::About) {
        Some(e) => lines.extend(edit_lines(e, iw.saturating_sub(2))),
        None => lines.extend(wrap(&about, iw.saturating_sub(2)).into_iter().map(|l| Line::from(format!(" {l}")))),
    }
    lines.push(Line::default());
    let acts_at = lines.len();
    for (i, act) in ACTS.iter().enumerate() {
        let (label, key) = match act {
            Act::Name => (tr!("Change name", "Mudar nome"), "n"),
            Act::About => (tr!("Change about", "Mudar recado"), tr!("a", "r")),
            Act::Photo => (tr!("Change photo", "Trocar foto"), tr!("p", "f")),
            Act::RemovePhoto if a.confirm_remove => (tr!("Remove photo? Enter confirms", "Remover foto? Enter confirma"), "x"),
            Act::RemovePhoto => (tr!("Remove photo", "Remover foto"), "x"),
        };
        let mut style = if i == a.profile_sel && edit.is_none() { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() };
        if edit.is_some() || a.saving.is_some() {
            style = style.patch(dim());
        } else if *act == Act::RemovePhoto && a.confirm_remove {
            style = style.fg(Color::Yellow).bold();
        }
        lines.push(Line::from(padded(label, key, iw)).style(style));
    }
    lines.push(Line::default());
    let hint = match (a.saving, edit.is_some()) {
        (Some(_), _) => tr!(format!("{} saving…", crate::media::spin()), format!("{} salvando…", crate::media::spin())),
        (None, true) => tr!("Enter saves · Esc cancels", "Enter salva · Esc cancela").into(),
        (None, false) => tr!("↑↓ move · Enter selects · Esc closes", "↑↓ move · Enter escolhe · Esc fecha").into(),
    };
    lines.push(Line::from(hint).style(dim()));
    // A gap, you (two rows) and a gap over the rest.
    let h = 4 + lines.len() as u16 + 2;
    let (_, inner) = dialog(f, app, w, h, tr!(" Profile ", " Perfil "));
    if inner.height < 4 {
        return;
    }
    // Your number where the button shows the connection.
    let number = app.my_jid().map(|j| phone(&j)).unwrap_or_default();
    head(f, app, Rect::new(inner.x.saturating_sub(1), inner.y + 1, inner.width + 1, 2), &number, 0);
    let body = Rect::new(inner.x, inner.y + 4, inner.width, inner.height.saturating_sub(4));
    for (n, line) in lines.into_iter().enumerate() {
        let row = Rect::new(body.x, body.y + n as u16, body.width, 1);
        if row.y >= body.bottom() {
            break;
        }
        if let Some(act) = n.checked_sub(acts_at).and_then(|i| ACTS.get(i)) {
            app.account.profile_rows.push((row, *act));
        }
        f.render_widget(Paragraph::new(line), row);
    }
}
