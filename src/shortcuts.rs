//! "Atalhos de teclado": the keys whatstui takes, grouped, each key a
//! chip, as WhatsApp Web's shortcuts dialog shows them; two columns when
//! the screen is wide enough, scrolled when it's too short.

use crate::app::App;
use crate::config::ENTER_SENDS;
use crate::ui::{dim, truncate, CAP_L, CAP_R, green, TEXT};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// A key's chip: a shade lighter than the dialog.
const KEY: Color = Color::Rgb(0x3B, 0x3E, 0x3E);
/// Between the two columns.
const GAP: u16 = 4;

type Section = (&'static str, Vec<(&'static str, &'static str)>);

/// Each key as it's written here: " / " between alternatives, "+" between
/// keys held together, a space between keys of a list.
fn sections(app: &App) -> Vec<Section> {
    let (send, newline) =
        if app.settings.on(ENTER_SENDS) { ("Enter", "Shift+Enter / Alt+Enter") } else { ("Shift+Enter / Alt+Enter", "Enter") };
    vec![
        (
            tr!("Navigation", "Navegação"),
            vec![
                ("↑ ↓ / k j", tr!("Move through the list", "Mover pela lista")),
                ("g / G", tr!("Top / bottom of the list", "Início / fim da lista")),
                ("Enter / → / l", tr!("Open the chat", "Abrir a conversa")),
                ("Ctrl+N / Ctrl+P", tr!("Next / previous chat", "Próxima / anterior conversa")),
                ("Ctrl+K / /", tr!("Search chats and messages", "Pesquisar conversas e mensagens")),
                ("1 2 3 4", tr!("All, Unread, Favorites, Groups", "Tudo, Não lidas, Favoritas, Grupos")),
                ("Esc", tr!("Clear the search", "Limpar a pesquisa")),
                ("Esc / ← / h", tr!("Leave archived", "Sair das arquivadas")),
            ],
        ),
        (
            tr!("Chat", "Conversa"),
            vec![
                ("Tab", tr!("From the list to the message", "Da lista para a mensagem")),
                ("m", tr!("Chat menu", "Menu da conversa")),
                ("i", tr!("Contact or group info", "Dados do contato ou grupo")),
                ("I", tr!("Contact or group photo", "Foto do contato ou grupo")),
                ("Esc", tr!("Close the info", "Fechar os dados")),
                (tr!("Space", "Espaço"), tr!("Play or pause the last audio", "Tocar ou pausar o último áudio")),
                ("v", tr!("Open the last video", "Abrir o último vídeo")),
                ("PgUp PgDn", tr!("Scroll the chat", "Rolar a conversa")),
            ],
        ),
        (
            tr!("Photos and videos", "Fotos e vídeos"),
            vec![
                (tr!("Space / Enter", "Espaço / Enter"), tr!("Play or stop the video", "Tocar ou parar o vídeo")),
                ("b", tr!("Download to the Downloads folder", "Baixar para a pasta Downloads")),
                ("p", tr!("Download and show in folder", "Baixar e mostrar na pasta")),
                ("o", tr!("Open outside the app", "Abrir fora do app")),
                ("Esc / q", tr!("Close", "Fechar")),
            ],
        ),
        (
            tr!("Message", "Mensagem"),
            vec![
                (send, tr!("Send", "Enviar")),
                (newline, tr!("New line", "Quebrar linha")),
                (":", tr!("Emoji by name", "Emoji pelo nome")),
                ("Alt+E", tr!("Emoji panel", "Painel de emojis")),
                ("Alt+A", tr!("Attach", "Anexar")),
                ("Ctrl+V", tr!("Paste image or files", "Colar imagem ou arquivos")),
                ("Ctrl+W", tr!("Delete the previous word", "Apagar a palavra anterior")),
                ("Ctrl+U", tr!("Delete the message", "Apagar a mensagem")),
                ("↑", tr!("Edit the last message sent", "Editar a última mensagem enviada")),
                ("Alt+R", tr!("Reply to the last one; again, to the one before", "Responder à última; de novo, à anterior")),
                ("Esc", tr!("Cancel the reply", "Cancelar a resposta")),
                ("Esc / Tab", tr!("Back to the list", "Voltar para a lista")),
            ],
        ),
        (
            tr!("Select and forward", "Selecionar e encaminhar"),
            vec![
                ("↑ ↓ / k j", tr!("Move between messages", "Mover entre as mensagens")),
                (tr!("Space", "Espaço"), tr!("Select or deselect the message", "Marcar ou desmarcar a mensagem")),
                ("Enter", tr!("Forward (or delete) the selected", "Encaminhar (ou apagar) as marcadas")),
                ("Esc", tr!("Leave selection", "Sair da seleção")),
                ("Ctrl+C", tr!("Copy the selected, or the highlighted text", "Copiar as marcadas, ou o texto selecionado")),
                ("Enter", tr!("Select the chat, when forwarding", "Marcar a conversa, ao encaminhar")),
                ("Alt+Enter / Ctrl+Enter", tr!("Forward to the selected", "Encaminhar para as marcadas")),
            ],
        ),
        (
            tr!("Formatting", "Formatação"),
            vec![
                ("Ctrl+B / Alt+B", tr!("Bold", "Negrito")),
                ("Ctrl+I / Alt+I", tr!("Italic", "Itálico")),
                ("Ctrl+Shift+X / Alt+X", tr!("Strikethrough", "Tachado")),
                ("Ctrl+Shift+M / Alt+M", tr!("Monospace", "Monoespaçado")),
                ("Ctrl+Shift+C / Alt+C", tr!("Code", "Código")),
                ("Ctrl+Shift+9", tr!("Quote", "Citação")),
            ],
        ),
        (
            "App",
            vec![
                ("Alt+S / ,", tr!("Account menu", "Menu da conta")),
                ("Ctrl+/ / ?", tr!("Keyboard shortcuts", "Atalhos de teclado")),
                ("↑ ↓ Enter Esc", tr!("Move, choose and close in menus", "Mover, escolher e fechar nos menus")),
                ("Ctrl+O", tr!("Restart into the installed version", "Reiniciar na versão instalada")),
                ("q / Ctrl+C", tr!("Quit", "Sair")),
            ],
        ),
    ]
}

fn chip(key: &str) -> [Span<'static>; 3] {
    [
        Span::styled(CAP_L, Style::new().fg(KEY)),
        Span::styled(key.to_string(), Style::new().fg(TEXT).bg(KEY)),
        Span::styled(CAP_R, Style::new().fg(KEY)),
    ]
}

/// A key as it's written in `sections`, in chips.
fn chips(spec: &str) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, alt) in spec.split(" / ").enumerate() {
        if i > 0 {
            out.push(Span::styled(" / ", dim()));
        }
        for (j, seq) in alt.split(' ').enumerate() {
            if j > 0 {
                out.push(Span::raw(" "));
            }
            for (n, key) in seq.split('+').enumerate() {
                if n > 0 {
                    out.push(Span::styled("+", dim()));
                }
                out.extend(chip(key));
            }
        }
    }
    out
}

fn width(spans: &[Span]) -> u16 {
    spans.iter().map(|s| s.content.width()).sum::<usize>() as u16
}

/// A column of sections: its lines, and its keys' and descriptions' widths.
struct Column {
    rows: Vec<Line<'static>>,
    keys: u16,
    words: u16,
}

impl Column {
    fn of(sections: &[Section], words: Option<u16>) -> Column {
        let keys = sections.iter().flat_map(|(_, r)| r).map(|(k, _)| width(&chips(k))).max().unwrap_or(0);
        let full = sections.iter().flat_map(|(_, r)| r).map(|(_, d)| d.width() as u16).max().unwrap_or(0);
        let words = words.map_or(full, |w| w.min(full));
        let mut rows = Vec::new();
        for (n, (heading, keys_of)) in sections.iter().enumerate() {
            if n > 0 {
                rows.push(Line::default());
            }
            rows.push(Line::from(format!(" {heading}")).style(Style::new().fg(green()).bold()));
            for (k, d) in keys_of {
                let mut spans = vec![Span::raw("  ")];
                let c = chips(k);
                let pad = keys.saturating_sub(width(&c)) + 2;
                spans.extend(c);
                spans.push(Span::raw(" ".repeat(pad as usize)));
                spans.push(Span::raw(truncate(d, words as usize)));
                rows.push(Line::from(spans));
            }
        }
        Column { rows, keys, words }
    }

    fn width(&self) -> u16 {
        2 + self.keys + 2 + self.words
    }
}

/// The dialog, over everything, scrolled `app.account.shortcuts` rows down.
pub(crate) fn draw(f: &mut Frame, app: &mut App) {
    let Some(top) = app.account.shortcuts else { return };
    let area = f.area();
    let all = sections(app);
    // Beside the box: its border and a column of margin each side, and some
    // of the screen left around it.
    let room = area.width.saturating_sub(8);
    // Two columns, split where they come out the most even.
    let height = |s: &[Section]| s.iter().map(|(_, r)| r.len() + 2).sum::<usize>();
    let split = (1..all.len()).min_by_key(|&s| height(&all[..s]).max(height(&all[s..]))).unwrap_or(1);
    let (left, right) = (Column::of(&all[..split], None), Column::of(&all[split..], None));
    let cols = if left.width() + GAP + right.width() <= room {
        vec![left, right]
    } else {
        let one = Column::of(&all, None);
        let words = room.saturating_sub(one.keys + 4);
        vec![Column::of(&all, Some(words))]
    };

    let body: Vec<Line> = (0..cols.iter().map(|c| c.rows.len()).max().unwrap_or(0))
        .map(|i| {
            let mut spans = Vec::new();
            for (n, c) in cols.iter().enumerate() {
                let line = c.rows.get(i).cloned().unwrap_or_default();
                let w = line.width() as u16;
                spans.extend(line.spans.into_iter().map(|s| s.patch_style(line.style)));
                if n + 1 < cols.len() {
                    spans.push(Span::raw(" ".repeat((c.width() + GAP).saturating_sub(w) as usize)));
                }
            }
            Line::from(spans)
        })
        .collect();
    let content_w = cols.iter().map(Column::width).sum::<u16>() + GAP * (cols.len() as u16 - 1);
    // A blank row on top; under the keys, a blank row and the hint.
    let (_, inner) = crate::account::dialog(f, app, content_w + 5, body.len() as u16 + 5, tr!(" Keyboard shortcuts ", " Atalhos de teclado "));
    let shown = inner.height.saturating_sub(3);
    let max = (body.len() as u16).saturating_sub(shown);
    let top = top.min(max);
    app.account.shortcuts = Some(top);
    app.account.shortcuts_page = shown;
    let rows: Vec<Line> = body.into_iter().skip(top as usize).take(shown as usize).collect();
    f.render_widget(Paragraph::new(rows), Rect { y: inner.y + 1, height: shown, ..inner });
    let hint = match (top > 0, top < max) {
        (false, false) => tr!(" Esc closes", " Esc fecha").to_string(),
        (up, down) => {
            let arrows = if up && down { "▲▼" } else if up { "▲" } else { "▼" };
            tr!(format!(" {arrows} ↑↓ scrolls · Esc closes"), format!(" {arrows} ↑↓ rola · Esc fecha"))
        }
    };
    let foot = Rect { y: inner.bottom().saturating_sub(1), height: 1, ..inner };
    f.render_widget(Paragraph::new(hint).style(dim()), foot);
}
