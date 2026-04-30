use crate::{
    tui::widgets::SELECTOR,
    ui_state::{PopupType, SetupMode, UiState},
};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Line,
    widgets::{
        Block, BorderType, HighlightSpacing, List, Padding, Paragraph, StatefulWidget, Widget, Wrap,
    },
};

pub struct SetupWizard;

impl StatefulWidget for SetupWizard {
    type State = UiState;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        let mode = match &state.popup.current {
            PopupType::Setup(m) => m.clone(),
            _ => return,
        };

        let theme = state.theme_manager.get_display_theme(true);
        let title = match &mode {
            SetupMode::ChooseKind => " NoctaVox — Library source ",
            SetupMode::NavUrl => " Navidrome — Server URL ",
            SetupMode::NavUser => " Navidrome — Username ",
            SetupMode::NavPassword => " Navidrome — Password ",
        };

        let block = Block::bordered()
            .border_type(theme.border_type)
            .border_style(theme.border)
            .title(title)
            .title_bottom(" [Enter] confirm  [Esc] cancel ")
            .title_alignment(ratatui::layout::Alignment::Center)
            .padding(Padding {
                left: 2,
                right: 2,
                top: 1,
                bottom: 0,
            })
            .bg(theme.bg);

        let inner = block.inner(area);
        block.render(area, buf);

        match &mode {
            SetupMode::ChooseKind => render_choose_kind(inner, buf, state),
            SetupMode::NavUrl | SetupMode::NavUser | SetupMode::NavPassword => {
                render_text_step(inner, buf, state, &mode);
            }
        }
    }
}

fn render_choose_kind(
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
    state: &mut UiState,
) {
    let theme = state.theme_manager.get_display_theme(true);
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);
    Paragraph::new("Use arrow keys and Enter to choose how your library is loaded:")
        .fg(theme.text_secondary)
        .wrap(Wrap { trim: true })
        .render(chunks[0], buf);

    let items = vec![
        Line::from("Local — scan folders on this computer"),
        Line::from("Navidrome — stream from a Subsonic-compatible server"),
    ];
    let list = List::new(items)
        .fg(state.theme_manager.active.text_muted)
        .highlight_symbol(SELECTOR)
        .highlight_style(Style::new().fg(theme.accent))
        .highlight_spacing(HighlightSpacing::Always);
    StatefulWidget::render(list, chunks[1], buf, &mut state.popup.selection);
}

fn render_text_step(
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
    state: &mut UiState,
    mode: &SetupMode,
) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .split(area);

    let hint = match mode {
        SetupMode::NavUrl => "Base URL only (no /rest path). Example: https://music.home.arpa",
        SetupMode::NavUser => "Your Navidrome login name",
        SetupMode::NavPassword => "Password is stored locally in the app database (MVP).",
        _ => "",
    };
    Paragraph::new(hint)
        .fg(state.theme_manager.active.text_muted)
        .wrap(Wrap { trim: true })
        .render(chunks[0], buf);

    let theme = state.theme_manager.get_display_theme(true);
    state.popup.input.set_block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .fg(theme.accent)
            .padding(Padding {
                left: 1,
                right: 1,
                top: 0,
                bottom: 0,
            }),
    );
    state
        .popup
        .input
        .set_style(Style::new().fg(theme.text_primary));
    state.popup.input.render(chunks[1], buf);

    if matches!(mode, SetupMode::NavUrl) {
        Paragraph::new("Example: https://navidrome.example.com")
            .fg(theme.text_muted)
            .render(chunks[2], buf);
    }
}
