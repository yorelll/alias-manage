//! Rendering. Every colour comes from [`Theme`]; status is also expressed as
//! text (`[on]`, `[ok]`, `[warn]`, `[error]`), so monochrome is fully usable.

use crate::{
    app::{
        filter_summary, App, DoctorState, Modal, PreviewState, Screen, ScrollState, StatusKind,
        SyncReportState,
    },
    data,
    model::{shell_label, target_type_label, target_type_semantics, visible_fields, Editor, Field},
    text,
    theme::Theme,
};
use aliasmgr_core::{
    executor::render_argv,
    model::{AliasRecord, ShellKind, ShellStatus, TargetType},
    sync,
};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserLayout {
    TooSmall,
    Narrow,
    Wide,
}

pub fn browser_layout(area: Rect) -> BrowserLayout {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        BrowserLayout::TooSmall
    } else if area.width >= 120 {
        BrowserLayout::Wide
    } else {
        BrowserLayout::Narrow
    }
}

fn span(value: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(value.into(), style)
}

fn line(spans: Vec<Span<'static>>) -> Line<'static> {
    Line::from(spans)
}

fn block<'a>(title: &str, theme: Theme) -> Block<'a> {
    Block::bordered()
        .border_type(theme.borders())
        .border_style(theme.muted())
        .title(Line::from(span(title.to_string(), theme.title())))
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area, app);
        return;
    }
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);
    let body = chunks[1];

    // Update the metrics key handling uses (page size, detail scroll, editor
    // auto-scroll) before taking an immutable view for rendering.
    match &mut app.screen {
        Screen::Browse => {
            app.wide_layout = browser_layout(body) == BrowserLayout::Wide;
            match browser_layout(body) {
                BrowserLayout::Wide => {
                    let panes = Layout::horizontal([
                        Constraint::Percentage(60),
                        Constraint::Percentage(40),
                    ])
                    .split(body);
                    app.list_height = panes[0].height.saturating_sub(2) as usize;
                    app.detail_height = panes[1].height.saturating_sub(2);
                }
                BrowserLayout::Narrow => {
                    if app.show_detail {
                        app.detail_height = body.height.saturating_sub(2);
                    } else {
                        app.list_height = body.height.saturating_sub(2) as usize;
                    }
                }
                BrowserLayout::TooSmall => {}
            }
            app.list_height = app.list_height.max(1);
            if app.visible.is_empty() {
                app.list_offset = 0;
            } else {
                let height = app.list_height;
                let mut offset = app.list_offset;
                if app.selected < offset {
                    offset = app.selected;
                } else if app.selected >= offset + height {
                    offset = app.selected + 1 - height;
                }
                offset = offset.min(app.visible.len().saturating_sub(height));
                app.list_offset = offset;
            }
        }
        Screen::Editor(editor) => {
            let inner_height = body.height.saturating_sub(2) as usize;
            let (_, selected_line) = editor_lines(editor, app.theme);
            editor.scroll = if selected_line >= inner_height {
                (selected_line + 1 - inner_height) as u16
            } else {
                0
            };
        }
        _ => {}
    }

    let app = &*app;
    render_header(frame, chunks[0], app);
    match &app.screen {
        Screen::Browse => render_browser(frame, body, app),
        Screen::Editor(editor) => render_editor(frame, body, app, editor),
        Screen::Preview(state) => render_preview(frame, body, app, state),
        Screen::Doctor(state) => render_doctor(frame, body, app, state),
        Screen::Help(state) => render_scroll(frame, body, app, state, false),
        Screen::SyncReport(state) => render_sync_report(frame, body, app, state),
    }
    render_footer(frame, chunks[2], app);
    render_modal(frame, area, app);
}

fn render_too_small(frame: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        line(vec![span("Alias Manager", app.theme.title())]),
        Line::default(),
        line(vec![span("Terminal too small", app.theme.error())]),
        line(vec![span(
            format!(
                "{}x{}; the TUI needs at least {}x{}.",
                area.width, area.height, MIN_WIDTH, MIN_HEIGHT
            ),
            app.theme.muted(),
        )]),
        line(vec![span(
            "Resize the terminal, or press q / Ctrl+C to exit.",
            app.theme.muted(),
        )]),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn screen_label(app: &App) -> &'static str {
    match &app.screen {
        Screen::Browse if app.show_detail => "Alias detail",
        Screen::Browse => "Aliases",
        Screen::Editor(_) => "Edit alias",
        Screen::Preview(_) => "Preview",
        Screen::Doctor(_) => "Doctor",
        Screen::Help(_) => "Help",
        Screen::SyncReport(_) => "Sync results",
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width as usize;
    let revision = format!("rev {}", app.snapshot.revision);
    let prefix = format!("Alias Manager  |  {}  |  root: ", screen_label(app));
    let suffix = format!("  |  {revision}");
    let root_width =
        width.saturating_sub(text::display_width(&prefix) + text::display_width(&suffix));
    let root = text::truncate_display(&app.root.display().to_string(), root_width, app.theme.ascii);
    let first = line(vec![
        span("Alias Manager", app.theme.title()),
        span("  |  ", app.theme.muted()),
        span(screen_label(app), app.theme.focus()),
        span("  |  root: ", app.theme.muted()),
        span(root, Style::default()),
        span(suffix, app.theme.muted()),
    ]);
    let sync = sync_status(app);
    let mut second_spans = vec![span(filter_summary(&app.filter), app.theme.muted())];
    second_spans.push(span("  |  ", app.theme.muted()));
    second_spans.push(span(
        sync.0,
        if sync.1 {
            app.theme.warning()
        } else {
            app.theme.success()
        },
    ));
    if let Some(busy) = app.busy_label() {
        second_spans.push(span("  |  ", app.theme.muted()));
        second_spans.push(span(format!("busy: {busy}"), app.theme.warning()));
    }
    if app.external_pending {
        second_spans.push(span("  |  ", app.theme.muted()));
        second_spans.push(span("external change seen", app.theme.warning()));
    }
    if app.filter.active {
        second_spans.push(span("  |  search: ", app.theme.muted()));
        second_spans.push(span(
            format!("{}_", text::one_line(app.filter.input.value())),
            app.theme.accent(),
        ));
    }
    frame.render_widget(
        Paragraph::new(Text::from(vec![first, line(second_spans)])).wrap(Wrap { trim: true }),
        area,
    );
}

/// `(text, needs_sync)` for the header.
fn sync_status(app: &App) -> (String, bool) {
    // Disabled aliases still need a sync to remove their old definitions, and
    // a delete leaves a stale shell_state/tombstone even when no alias remains.
    let mut needed = app.snapshot.shell_states.iter().any(|state| {
        state.status == ShellStatus::Failed || state.applied_revision < app.snapshot.revision
    });
    for alias in &app.snapshot.aliases {
        for shell in &alias.shells {
            let state = app
                .snapshot
                .shell_states
                .iter()
                .find(|state| state.shell == *shell);
            match state {
                None => needed = true,
                Some(state)
                    if state.status == ShellStatus::Failed
                        || state.applied_revision < app.snapshot.revision =>
                {
                    needed = true
                }
                _ => {}
            }
        }
    }
    if needed {
        ("sync: needed".to_string(), true)
    } else {
        ("sync: up to date".to_string(), false)
    }
}

fn render_browser(frame: &mut Frame, area: Rect, app: &App) {
    match browser_layout(area) {
        BrowserLayout::Wide => {
            let panes =
                Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(area);
            render_list(frame, panes[0], app);
            render_detail(frame, panes[1], app);
        }
        BrowserLayout::Narrow => {
            if app.show_detail {
                render_detail(frame, area, app);
            } else {
                render_list(frame, area, app);
            }
        }
        BrowserLayout::TooSmall => render_too_small(frame, area, app),
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &App) {
    let title = format!("Aliases ({})", app.visible.len());
    let container = block(&title, app.theme);
    let inner = container.inner(area);
    if app.visible.is_empty() {
        let message = if app.snapshot.aliases.is_empty() {
            vec![
                line(vec![span("No aliases yet.", app.theme.muted())]),
                line(vec![span(
                    "Press n to create one. The database is shared with the CLI and GUI.",
                    app.theme.muted(),
                )]),
            ]
        } else {
            vec![
                line(vec![span(
                    "No aliases match this filter.",
                    app.theme.muted(),
                )]),
                line(vec![span(
                    "Press / and Esc to clear it.",
                    app.theme.muted(),
                )]),
            ]
        };
        frame.render_widget(Paragraph::new(Text::from(message)).block(container), area);
        return;
    }
    let items = app
        .visible
        .iter()
        .filter_map(|index| app.snapshot.aliases.get(*index))
        .map(|alias| ListItem::new(list_line(alias, inner.width as usize, app.theme)))
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(container)
        .highlight_style(app.theme.selected())
        .highlight_symbol("> ");
    let mut state = ListState::default();
    state.select(Some(app.selected));
    *state.offset_mut() = app.list_offset;
    frame.render_stateful_widget(list, area, &mut state);
}

fn list_line(alias: &AliasRecord, width: usize, theme: Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let marker_style = if alias.enabled {
        theme.success()
    } else {
        theme.muted()
    };
    spans.push(span(theme.enabled_marker(alias.enabled), marker_style));
    spans.push(span(" ", Style::default()));
    let mut used = 5usize;
    let name_width = width.saturating_sub(used).clamp(8, 24);
    spans.push(span(
        text::fit(&alias.name, name_width, theme.ascii),
        theme.accent().add_modifier(Modifier::BOLD),
    ));
    used += name_width + 1;
    let show_shells = width >= 90;
    let show_tags = width >= 110;
    let shells_width = if show_shells { 18 } else { 0 };
    let tags_width = if show_tags { 20 } else { 0 };
    let target_width = width.saturating_sub(used + shells_width + tags_width + 2);
    if target_width > 0 {
        spans.push(span(" ", Style::default()));
        spans.push(span(
            text::truncate_display(&target_summary(alias), target_width, theme.ascii),
            Style::default(),
        ));
    }
    if show_shells {
        spans.push(span(" ", Style::default()));
        spans.push(span(
            text::truncate_display(&shell_summary(&alias.shells), shells_width, theme.ascii),
            theme.muted(),
        ));
    }
    if show_tags {
        spans.push(span(" ", Style::default()));
        spans.push(span(
            text::truncate_display(&alias.tags.join(","), tags_width, theme.ascii),
            theme.muted(),
        ));
    }
    line(spans)
}

pub fn target_summary(alias: &AliasRecord) -> String {
    if alias.target_type == TargetType::ChangeDirectory {
        return format!(
            "cd {}",
            alias
                .working_directory
                .as_deref()
                .unwrap_or(&alias.executable)
        );
    }
    let mut summary = alias.executable.clone();
    let count = alias.fixed_args.len();
    if count > 0 {
        summary.push_str(&format!(
            " +{count} arg{}",
            if count == 1 { "" } else { "s" }
        ));
    }
    summary
}

fn shell_summary(shells: &[ShellKind]) -> String {
    shells.iter().map(short_shell).collect::<Vec<_>>().join(",")
}

fn short_shell(shell: &ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::PowerShell5 => "ps5",
        ShellKind::PowerShell7 => "ps7",
        ShellKind::Fish => "fish",
        ShellKind::PosixSh => "sh",
    }
}

fn render_detail(frame: &mut Frame, area: Rect, app: &App) {
    let title = match &app.screen {
        Screen::Browse if !app.show_detail => "Detail  (Enter/Tab)",
        _ => "Detail  (Esc back)",
    };
    let container = block(title, app.theme);
    let Some(alias) = app.selected_alias() else {
        frame.render_widget(
            Paragraph::new(Text::from(vec![line(vec![span(
                "No alias selected.",
                app.theme.muted(),
            )])]))
            .block(container),
            area,
        );
        return;
    };
    let lines = detail_lines(app, alias);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(container)
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0)),
        area,
    );
}

fn detail_lines(app: &App, alias: &AliasRecord) -> Vec<Line<'static>> {
    let theme = app.theme;
    let mut lines = Vec::new();
    lines.push(line(vec![
        span(alias.name.clone(), theme.title()),
        span("  ", Style::default()),
        span(theme.enabled_marker(alias.enabled), theme.success()),
        span(format!("  DB revision {}", alias.revision), theme.muted()),
    ]));
    if !alias.description.is_empty() {
        lines.push(line(vec![
            span("Description: ", theme.label()),
            span(text::one_line(&alias.description), Style::default()),
        ]));
    }
    lines.push(line(vec![
        span("Target type: ", theme.label()),
        span(target_type_label(&alias.target_type), Style::default()),
    ]));
    lines.push(line(vec![
        span("  ", Style::default()),
        span(
            target_type_semantics(&alias.target_type).to_string(),
            theme.muted(),
        ),
    ]));
    lines.push(line(vec![
        span("Executable: ", theme.label()),
        span(text::one_line(&alias.executable), Style::default()),
    ]));
    lines.push(line(vec![
        span("Fixed args: ", theme.label()),
        span(
            format!("{} (one argv element per row)", alias.fixed_args.len()),
            theme.muted(),
        ),
    ]));
    for (index, argument) in alias.fixed_args.iter().enumerate() {
        lines.push(line(vec![
            span(format!("  [{index}] "), theme.muted()),
            if argument.is_empty() {
                span("(empty string)", theme.warning())
            } else {
                span(text::one_line(argument), Style::default())
            },
        ]));
    }
    lines.push(line(vec![
        span("Pass user args: ", theme.label()),
        span(
            if alias.pass_args { "yes" } else { "no" }.to_string(),
            Style::default(),
        ),
    ]));
    lines.push(line(vec![span("argv preview", theme.label())]));
    match render_argv(alias, &["<arg1>".to_string(), "<arg2>".to_string()]) {
        Ok(argv) => {
            for (index, value) in argv.iter().enumerate() {
                lines.push(line(vec![
                    span(format!("  {index}: "), theme.muted()),
                    if value.is_empty() {
                        span("(empty string)", theme.warning())
                    } else {
                        span(text::one_line(value), Style::default())
                    },
                ]));
            }
            lines.push(line(vec![span(
                "  {{args}} inserts user arguments at that position; nothing is split or evaluated.",
                theme.muted(),
            )]));
        }
        Err(error) => lines.push(line(vec![span(
            format!("! argv unavailable: {}", text::one_line(&error.to_string())),
            theme.error(),
        )])),
    }
    lines.push(line(vec![
        span("Shells: ", theme.label()),
        span(
            alias
                .shells
                .iter()
                .map(shell_label)
                .collect::<Vec<_>>()
                .join(", "),
            Style::default(),
        ),
    ]));
    lines.push(line(vec![
        span("Tags: ", theme.label()),
        span(
            alias
                .tags
                .iter()
                .map(|tag| text::one_line(tag))
                .collect::<Vec<_>>()
                .join(", "),
            Style::default(),
        ),
    ]));
    lines.push(line(vec![
        span("Working dir: ", theme.label()),
        span(
            alias
                .working_directory
                .as_deref()
                .map(text::one_line)
                .unwrap_or_else(|| "-".to_string()),
            Style::default(),
        ),
    ]));
    lines.push(line(vec![
        span("Path mode: ", theme.label()),
        span(
            match alias.path_mode {
                aliasmgr_core::model::PathMode::Absolute => "absolute",
                aliasmgr_core::model::PathMode::RelativeToWorkingDirectory => {
                    "relative_to_working_directory"
                }
            }
            .to_string(),
            Style::default(),
        ),
    ]));
    match &alias.restriction {
        Some(restriction) => {
            lines.push(line(vec![
                span("Restriction: ", theme.label()),
                span(
                    format!(
                        "{}: {}",
                        restriction.mode.as_str(),
                        restriction.paths.join(", ")
                    ),
                    Style::default(),
                ),
            ]));
        }
        None => lines.push(line(vec![
            span("Restriction: ", theme.label()),
            span("none", theme.muted()),
        ])),
    }
    lines.push(line(vec![span(
        format!(
            "Environment: {} key(s), values masked",
            alias.environment.len()
        ),
        theme.label(),
    )]));
    for key in alias.environment.keys() {
        lines.push(line(vec![
            span(format!("  {}=", text::one_line(key)), Style::default()),
            span("***", theme.muted()),
        ]));
    }
    lines.push(line(vec![span("Status layers", theme.title())]));
    for status in data::four_layer_lines(&app.root, &app.snapshot, alias) {
        lines.push(line(vec![span(format!("  {status}"), theme.muted())]));
    }
    for shell in &alias.shells {
        lines.push(line(vec![
            span(format!("Reload [{}]: ", shell_label(shell)), theme.muted()),
            span(
                text::one_line(&sync::reload_command(&app.root, shell)),
                Style::default(),
            ),
        ]));
    }
    lines.push(line(vec![span(
        "Saving writes the database only. Press s to sync; reload is never automatic.",
        theme.warning(),
    )]));
    lines.push(line(vec![span(
        format!("Updated: {}", alias.updated_at.to_rfc3339()),
        theme.muted(),
    )]));
    lines
}

fn render_editor(frame: &mut Frame, area: Rect, app: &App, editor: &Editor) {
    let title = if editor.is_dirty() {
        format!("{} *", editor.title())
    } else {
        editor.title()
    };
    if area.width >= 100 {
        let panes = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        let (lines, _) = editor_lines(editor, app.theme);
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block(&title, app.theme))
                .wrap(Wrap { trim: false })
                .scroll((editor.scroll, 0)),
            panes[0],
        );
        let info = editor_side_lines(editor, app.theme);
        frame.render_widget(
            Paragraph::new(Text::from(info))
                .block(block("Draft checks", app.theme))
                .wrap(Wrap { trim: false }),
            panes[1],
        );
    } else {
        let chunks = Layout::vertical([Constraint::Min(8), Constraint::Length(7)]).split(area);
        let (lines, _) = editor_lines(editor, app.theme);
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block(&title, app.theme))
                .wrap(Wrap { trim: false })
                .scroll((editor.scroll, 0)),
            chunks[0],
        );
        let info = editor_side_lines(editor, app.theme);
        frame.render_widget(
            Paragraph::new(Text::from(info))
                .block(block("Draft checks", app.theme))
                .wrap(Wrap { trim: false }),
            chunks[1],
        );
    }
}

fn text_field_value(editor: &Editor, field: Field) -> String {
    match field {
        Field::Name => editor.draft.name.clone(),
        Field::Executable => editor.draft.executable.clone(),
        Field::Description => editor.draft.description.clone(),
        Field::WorkingDirectory => editor.draft.working_directory.clone(),
        _ => String::new(),
    }
}

fn input_spans(input: &crate::input::TextInput, theme: Theme) -> Vec<Span<'static>> {
    let (before, after) = input.split_at_cursor();
    let mut spans = vec![span(text::one_line(&before), Style::default())];
    let mut after_chars = after.chars();
    match after_chars.next() {
        Some(character) => {
            spans.push(span(character.to_string(), theme.cursor()));
        }
        None => spans.push(span(" ", theme.cursor())),
    }
    spans.push(span(text::one_line(after_chars.as_str()), Style::default()));
    spans
}

fn field_prefix(selected: bool) -> &'static str {
    if selected {
        "> "
    } else {
        "  "
    }
}

fn editor_lines(editor: &Editor, theme: Theme) -> (Vec<Line<'static>>, usize) {
    let mut lines = Vec::new();
    let mut selected_line = 0;
    if let Some(error) = &editor.error {
        lines.push(line(vec![span(
            format!(
                "! {}: {}",
                error.field.map(Field::label).unwrap_or("record"),
                text::one_line(&error.message)
            ),
            theme.error(),
        )]));
    }
    for field in visible_fields(editor.advanced) {
        let selected = editor.field == field;
        if selected {
            selected_line = lines.len();
        }
        match field {
            Field::Name | Field::Executable | Field::Description | Field::WorkingDirectory => {
                let mut spans = vec![
                    span(field_prefix(selected), theme.accent()),
                    span(format!("{}: ", field.label()), theme.label()),
                ];
                if selected {
                    spans.extend(input_spans(&editor.input, theme));
                } else {
                    spans.push(span(
                        text::one_line(&text_field_value(editor, field)),
                        Style::default(),
                    ));
                }
                lines.push(line(spans));
            }
            Field::TargetType => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span("Target type: ", theme.label()),
                    span(
                        format!(
                            "{}  [left/right to change]",
                            target_type_label(&editor.draft.target_type)
                        ),
                        Style::default(),
                    ),
                ]));
            }
            Field::FixedArgs => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span(
                        format!("Fixed args ({}):", editor.draft.fixed_args.len()),
                        theme.label(),
                    ),
                    span("  a add  d delete  J/K reorder  Enter edit", theme.muted()),
                ]));
                if editor.draft.fixed_args.is_empty() {
                    lines.push(line(vec![span("    (none)", theme.muted())]));
                }
                for (index, argument) in editor.draft.fixed_args.iter().enumerate() {
                    let row_selected = selected && index == editor.cursor;
                    let style = if row_selected {
                        theme.selected()
                    } else {
                        Style::default()
                    };
                    let value = if argument.is_empty() {
                        "(empty string)".to_string()
                    } else {
                        text::one_line(argument)
                    };
                    lines.push(line(vec![
                        span(format!("    [{index}] "), theme.muted()),
                        span(value, style),
                    ]));
                }
                if selected && editor.field == Field::FixedArgs {
                    if let Some(input) = &editor.list_input {
                        let mut spans = vec![span("    edit: ", theme.accent())];
                        spans.extend(input_spans(input, theme));
                        lines.push(line(spans));
                    }
                }
            }
            Field::PassArgs => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span(
                        format!(
                            "{} Pass user args  [Space/Enter toggle]",
                            theme.checkbox(editor.draft.pass_args)
                        ),
                        theme.label(),
                    ),
                ]));
            }
            Field::Shells => {
                let choices = editor.shell_editor_choices();
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span("Shells:", theme.label()),
                    span("  Space toggle", theme.muted()),
                ]));
                for (index, shell) in choices.iter().enumerate() {
                    let checked = editor.draft.shells.contains(shell);
                    let row_selected = selected && index == editor.shell_cursor;
                    lines.push(line(vec![
                        span(
                            format!("    {} ", theme.checkbox(checked)),
                            Style::default(),
                        ),
                        span(
                            shell_label(shell),
                            if row_selected {
                                theme.selected()
                            } else if checked {
                                theme.accent()
                            } else {
                                theme.muted()
                            },
                        ),
                    ]));
                }
            }
            Field::Tags => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span(
                        format!("Tags ({}):", editor.draft.tags.len()),
                        theme.label(),
                    ),
                    span("  a add  d delete  J/K reorder  Enter edit", theme.muted()),
                ]));
                if editor.draft.tags.is_empty() {
                    lines.push(line(vec![span("    (none)", theme.muted())]));
                }
                for (index, tag) in editor.draft.tags.iter().enumerate() {
                    let row_selected = selected && index == editor.cursor;
                    lines.push(line(vec![
                        span(format!("    [{index}] "), theme.muted()),
                        span(
                            text::one_line(tag),
                            if row_selected {
                                theme.selected()
                            } else {
                                Style::default()
                            },
                        ),
                    ]));
                }
                if selected && editor.field == Field::Tags {
                    if let Some(input) = &editor.list_input {
                        let mut spans = vec![span("    edit: ", theme.accent())];
                        spans.extend(input_spans(input, theme));
                        lines.push(line(spans));
                    }
                }
            }
            Field::Environment => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span(
                        format!(
                            "Environment ({} keys, values masked):",
                            editor.draft.environment.len()
                        ),
                        theme.label(),
                    ),
                    span("  a add  d delete  Enter edit value", theme.muted()),
                ]));
                if editor.draft.environment.is_empty() {
                    lines.push(line(vec![span("    (none)", theme.muted())]));
                }
                for (index, (key, value)) in editor.draft.environment.iter().enumerate() {
                    let row_selected = selected && index == editor.cursor;
                    let style = if row_selected {
                        theme.selected()
                    } else {
                        Style::default()
                    };
                    lines.push(line(vec![
                        span(format!("    [{index}] "), theme.muted()),
                        span(
                            format!(
                                "{}={}",
                                if key.is_empty() {
                                    "(empty key)".to_string()
                                } else {
                                    text::one_line(key)
                                },
                                text::mask_value(value)
                            ),
                            style,
                        ),
                    ]));
                }
                if selected && editor.field == Field::Environment {
                    if let Some(input) = &editor.list_input {
                        let mut spans = vec![span(
                            if editor.list_input_stage == crate::model::ListInputStage::EnvKey {
                                "    edit key: "
                            } else {
                                "    edit value (visible only while editing): "
                            },
                            theme.accent(),
                        )];
                        spans.extend(input_spans(input, theme));
                        lines.push(line(spans));
                    }
                }
            }
            Field::PathMode => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span("Path mode: ", theme.label()),
                    span(
                        match editor.draft.path_mode {
                            aliasmgr_core::model::PathMode::Absolute => "absolute",
                            aliasmgr_core::model::PathMode::RelativeToWorkingDirectory => {
                                "relative_to_working_directory"
                            }
                        }
                        .to_string(),
                        Style::default(),
                    ),
                    span("  [Space toggle]", theme.muted()),
                ]));
            }
            Field::RestrictionMode => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span("Restriction: ", theme.label()),
                    span(
                        match editor.draft.restriction_mode {
                            None => "none",
                            Some(aliasmgr_core::model::RestrictionMode::Whitelist) => "whitelist",
                            Some(aliasmgr_core::model::RestrictionMode::Blacklist) => "blacklist",
                        }
                        .to_string(),
                        Style::default(),
                    ),
                    span("  [left/right to change]", theme.muted()),
                ]));
            }
            Field::RestrictionPaths => {
                lines.push(line(vec![
                    span(field_prefix(selected), theme.accent()),
                    span(
                        format!(
                            "Restriction paths ({}):",
                            editor.draft.restriction_paths.len()
                        ),
                        theme.label(),
                    ),
                ]));
                for (index, path) in editor.draft.restriction_paths.iter().enumerate() {
                    let row_selected = selected && index == editor.cursor;
                    lines.push(line(vec![
                        span(format!("    [{index}] "), theme.muted()),
                        span(
                            if path.is_empty() {
                                "(empty)".to_string()
                            } else {
                                text::one_line(path)
                            },
                            if row_selected {
                                theme.selected()
                            } else {
                                Style::default()
                            },
                        ),
                    ]));
                }
                if selected && editor.field == Field::RestrictionPaths {
                    if let Some(input) = &editor.list_input {
                        let mut spans = vec![span("    edit: ", theme.accent())];
                        spans.extend(input_spans(input, theme));
                        lines.push(line(spans));
                    }
                }
            }
        }
    }
    lines.push(line(vec![span(
        if editor.advanced {
            "F3 hide advanced fields"
        } else {
            "F3 show advanced fields (working dir, env, path mode, restriction)"
        },
        theme.muted(),
    )]));
    (lines, selected_line)
}

fn editor_side_lines(editor: &Editor, theme: Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(line(vec![span("argv preview", theme.title())]));
    lines.push(line(vec![span(
        "one element per line; | is not involved",
        theme.muted(),
    )]));
    match editor.draft.argv_preview(&editor.base) {
        Ok(argv) => {
            for (index, value) in argv.iter().enumerate() {
                lines.push(line(vec![
                    span(format!("{index}: "), theme.muted()),
                    if value.is_empty() {
                        span("(empty string)", theme.warning())
                    } else {
                        span(text::one_line(value), Style::default())
                    },
                ]));
            }
        }
        Err(error) => lines.push(line(vec![span(
            format!(
                "! {}: {}",
                error.field.map(Field::label).unwrap_or("record"),
                text::one_line(&error.message)
            ),
            theme.error(),
        )])),
    }
    lines.push(Line::default());
    lines.push(line(vec![span("target semantics", theme.title())]));
    lines.push(line(vec![span(
        target_type_semantics(&editor.draft.target_type).to_string(),
        theme.muted(),
    )]));
    lines.push(Line::default());
    match editor.validate_now() {
        Ok(_) => lines.push(line(vec![span(
            "[ok] core validate_alias accepts this draft",
            theme.success(),
        )])),
        Err(error) => lines.push(line(vec![span(
            format!(
                "! {}: {}",
                error.field.map(Field::label).unwrap_or("record"),
                text::one_line(&error.message)
            ),
            theme.error(),
        )])),
    }
    if let Some(revision) = editor.conflict_revision {
        lines.push(line(vec![span(
            format!("! conflict: stored revision is now {revision}"),
            theme.warning(),
        )]));
    }
    lines.push(Line::default());
    lines.push(line(vec![span(
        "Ctrl+S writes the database only.",
        theme.muted(),
    )]));
    lines.push(line(vec![span(
        "Sync and reload are separate, explicit steps.",
        theme.muted(),
    )]));
    lines
}

fn render_preview(frame: &mut Frame, area: Rect, app: &App, state: &PreviewState) {
    let shell = state.shell().map(shell_label).unwrap_or("-");
    let title = format!(
        "Preview: {} [{}]{}",
        state.alias_name,
        shell,
        if state.redacted {
            "  (environment values masked)"
        } else {
            ""
        }
    );
    let lines = if let Some(error) = &state.error {
        vec![line(vec![span(
            format!("! preview unavailable: {}", text::one_line(error)),
            app.theme.error(),
        )])]
    } else {
        state
            .body
            .lines()
            .map(|line| Line::from(text::one_line(line)))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block(&title, app.theme))
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0)),
        area,
    );
}

fn render_doctor(frame: &mut Frame, area: Rect, app: &App, state: &DoctorState) {
    let title = format!("Doctor ({} findings)", state.findings.len());
    let mut lines = Vec::new();
    if state.busy {
        lines.push(line(vec![span(
            "Scanning read-only diagnostics...",
            app.theme.muted(),
        )]));
    }
    if let Some(error) = &state.error {
        lines.push(line(vec![span(
            format!("! doctor failed: {}", text::one_line(error)),
            app.theme.error(),
        )]));
    }
    if !state.busy && state.findings.is_empty() && state.error.is_none() {
        lines.push(line(vec![span("No findings.", app.theme.success())]));
        lines.push(line(vec![span(
            "Press r to rescan, s to sync, Esc to go back.",
            app.theme.muted(),
        )]));
    }
    for (index, finding) in state.findings.iter().enumerate() {
        let (marker, style) = match finding.severity {
            "error" => ("[error]", app.theme.error()),
            "warning" => ("[warn]", app.theme.warning()),
            _ => ("[info]", app.theme.muted()),
        };
        let selected = index == state.selected;
        let mut spans = vec![
            span(if selected { "> " } else { "  " }, app.theme.accent()),
            span(format!("{marker} "), style),
        ];
        if let Some(shell) = &finding.shell {
            spans.push(span(format!("{}: ", shell_label(shell)), app.theme.label()));
        }
        spans.push(span(
            text::one_line(&finding.message),
            if selected {
                app.theme.selected()
            } else {
                Style::default()
            },
        ));
        spans.push(span(format!("  ({})", finding.code), app.theme.muted()));
        lines.push(line(spans));
        if let Some(action) = &finding.action {
            lines.push(line(vec![span(
                format!("      action: {}", text::one_line(action)),
                app.theme.success(),
            )]));
        }
        if let Some(detail) = &finding.detail {
            lines.push(line(vec![span(
                format!("      detail: {}", text::one_line(detail)),
                app.theme.muted(),
            )]));
        }
        lines.push(Line::default());
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block(&title, app.theme))
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0)),
        area,
    );
}

fn render_scroll(frame: &mut Frame, area: Rect, app: &App, state: &ScrollState, wrap: bool) {
    let lines = state
        .lines
        .iter()
        .map(|value| Line::from(value.clone()))
        .collect::<Vec<_>>();
    let paragraph = Paragraph::new(Text::from(lines))
        .block(block(&state.title, app.theme))
        .scroll((state.scroll, 0));
    let paragraph = if wrap {
        paragraph.wrap(Wrap { trim: false })
    } else {
        paragraph
    };
    frame.render_widget(paragraph, area);
}

fn render_sync_report(frame: &mut Frame, area: Rect, app: &App, state: &SyncReportState) {
    let title = format!("Sync results (revision {})", state.run.revision);
    let mut lines = Vec::new();
    for result in &state.run.results {
        let (marker, style) = match result.status {
            ShellStatus::Ok => ("[ok]", app.theme.success()),
            ShellStatus::Failed => ("[error]", app.theme.error()),
            ShellStatus::Stale => ("[warn]", app.theme.warning()),
            ShellStatus::LoaderMissing | ShellStatus::Unknown => ("[warn]", app.theme.warning()),
        };
        lines.push(line(vec![
            span(format!("{marker} "), style),
            span(
                format!("{}: {:?}", shell_label(&result.shell), result.status),
                style,
            ),
        ]));
        lines.push(line(vec![
            span("  generated: ", app.theme.label()),
            span(
                result.generated_path.display().to_string(),
                Style::default(),
            ),
        ]));
        lines.push(line(vec![
            span("  loader: ", app.theme.label()),
            span(
                if result.loader_installed {
                    "installed".to_string()
                } else {
                    format!(
                        "not installed{}",
                        result
                            .loader_error_code
                            .as_deref()
                            .map(|code| format!(" ({code})"))
                            .unwrap_or_default()
                    )
                },
                app.theme.muted(),
            ),
        ]));
        if let Some(error) = &result.error {
            lines.push(line(vec![span(format!("  ! {error}"), app.theme.error())]));
        }
        lines.push(line(vec![
            span("  reload: ", app.theme.label()),
            span(
                text::one_line(&sync::reload_command(&app.root, &result.shell)),
                Style::default(),
            ),
        ]));
        lines.push(Line::default());
    }
    lines.push(line(vec![span(
        "Reload is NOT executed automatically. Run the command above in this terminal.",
        app.theme.warning(),
    )]));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block(&title, app.theme))
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0)),
        area,
    );
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let keys = footer_keys(app);
    let (prefix, style) = match app.status.kind {
        StatusKind::Info => ("[info]", app.theme.muted()),
        StatusKind::Success => ("[ok]", app.theme.success()),
        StatusKind::Warning => ("[warn]", app.theme.warning()),
        StatusKind::Error => ("[error]", app.theme.error()),
    };
    let width = area.width as usize;
    let status = format!(
        "{prefix} {}{}",
        text::one_line(&app.status.text),
        app.busy_label()
            .map(|busy| format!("  |  busy: {busy}"))
            .unwrap_or_default()
    );
    let lines = vec![
        line(vec![span(
            text::truncate_display(&keys, width, app.theme.ascii),
            app.theme.muted(),
        )]),
        line(vec![span(
            text::truncate_display(&status, width, app.theme.ascii),
            style,
        )]),
    ];
    frame.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn footer_keys(app: &App) -> String {
    if let Some(modal) = &app.modal {
        return match modal {
            Modal::ConfirmDelete { .. } => "y confirm delete   n/Esc cancel".to_string(),
            Modal::ConfirmQuit => "y discard and quit   n/Esc keep editing".to_string(),
            Modal::ConfirmDiscard => "y discard   n/Esc keep editing".to_string(),
            Modal::Rename { .. } => "type a new name   Enter rename   Esc cancel".to_string(),
            Modal::TagFilter { .. } => {
                "j/k move   Space toggle   c clear   Enter apply   Esc cancel".to_string()
            }
            Modal::Conflict { .. } => {
                "r reload latest   d discard draft   o overwrite (explicit)   Esc keep draft"
                    .to_string()
            }
            Modal::Message { .. } => "Enter/Esc close".to_string(),
        };
    }
    match &app.screen {
        Screen::Browse if app.filter.active => {
            "type to search   Up/Down move   Enter apply   Esc clear".to_string()
        }
        Screen::Browse => {
            "Up/Down move  Enter detail  / search  n new  e edit  r rename  Space on/off  d delete  s sync  p preview  D doctor  t tags  o sort  ? help  q quit".to_string()
        }
        Screen::Editor(_) => {
            "Tab field  Ctrl+S save  Esc cancel  Up/Down select  a add  d delete  J/K reorder  Enter edit  Space toggle  F3 advanced".to_string()
        }
        Screen::Preview(_) => {
            "j/k scroll  PgUp/PgDn  Tab/[ ] switch shell  Esc back  q quit".to_string()
        }
        Screen::Doctor(_) => "j/k move  s sync  r rescan  Esc back  q quit".to_string(),
        Screen::Help(_) => "j/k scroll  PgUp/PgDn  Home/End  Esc back  q quit".to_string(),
        Screen::SyncReport(_) => "j/k scroll  PgUp/PgDn  Esc back  q quit".to_string(),
    }
}

fn render_modal(frame: &mut Frame, area: Rect, app: &App) {
    let Some(modal) = &app.modal else {
        return;
    };
    let (title, lines) = modal_lines(modal, app, area);
    let width = area.width.saturating_sub(4).clamp(20, 72).min(area.width);
    let height = (lines.len() as u16 + 2).min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block(&title, app.theme))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn modal_lines(modal: &Modal, app: &App, area: Rect) -> (String, Vec<Line<'static>>) {
    let theme = app.theme;
    match modal {
        Modal::ConfirmDelete { alias } => (
            "Delete alias".to_string(),
            vec![
                line(vec![span(
                    format!("Delete \"{}\"?", text::one_line(&alias.name)),
                    theme.title(),
                )]),
                Line::default(),
                line(vec![span(
                    "A tombstone is recorded for every bound shell; the next sync removes it.",
                    theme.muted(),
                )]),
                Line::default(),
                line(vec![span("y confirm    n/Esc cancel", theme.warning())]),
            ],
        ),
        Modal::ConfirmQuit => (
            "Unsaved changes".to_string(),
            vec![
                line(vec![span("The editor has unsaved changes.", theme.title())]),
                Line::default(),
                line(vec![span(
                    "y discard and quit    n/Esc keep editing",
                    theme.warning(),
                )]),
            ],
        ),
        Modal::ConfirmDiscard => (
            "Discard changes".to_string(),
            vec![
                line(vec![span("Discard the unsaved edit?", theme.title())]),
                Line::default(),
                line(vec![span(
                    "y discard    n/Esc keep editing",
                    theme.warning(),
                )]),
            ],
        ),
        Modal::Rename { input } => {
            let mut spans = vec![span("New name: ", theme.label())];
            spans.extend(input_spans(input, theme));
            (
                "Rename alias".to_string(),
                vec![
                    line(spans),
                    Line::default(),
                    line(vec![span(
                        "Enter rename    Esc cancel. The old name is retired for its shells.",
                        theme.muted(),
                    )]),
                ],
            )
        }
        Modal::TagFilter { entries, cursor } => {
            let mut lines = Vec::new();
            let visible = area.height.saturating_sub(6).max(1) as usize;
            let start = cursor.saturating_sub(visible.saturating_sub(1));
            for (index, (tag, checked)) in entries.iter().enumerate().skip(start).take(visible) {
                let selected = index == *cursor;
                lines.push(line(vec![
                    span(if selected { "> " } else { "  " }, theme.accent()),
                    span(
                        format!("{} {}", theme.checkbox(*checked), text::one_line(tag)),
                        if selected {
                            theme.selected()
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
            lines.push(Line::default());
            lines.push(line(vec![span(
                "Space toggle   c clear   Enter apply   Esc cancel",
                theme.muted(),
            )]));
            ("Tag filter".to_string(), lines)
        }
        Modal::Conflict { current } => (
            "Record changed elsewhere".to_string(),
            vec![
                line(vec![span(
                    format!(
                        "{} was modified in another front end (revision {}).",
                        text::one_line(&current.name),
                        current.revision
                    ),
                    theme.warning(),
                )]),
                Line::default(),
                line(vec![span(
                    "Your draft is NOT saved. Choose explicitly:",
                    theme.title(),
                )]),
                line(vec![span(
                    "r reload latest (discards draft)",
                    theme.muted(),
                )]),
                line(vec![span("d discard draft and go back", theme.muted())]),
                line(vec![span(
                    "o overwrite with this draft (explicit)",
                    theme.warning(),
                )]),
                Line::default(),
                line(vec![span("Esc keep editing without saving", theme.muted())]),
            ],
        ),
        Modal::Message { title, lines } => (
            title.clone(),
            lines
                .iter()
                .map(|value| Line::from(value.clone()))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, StatusKind};
    use crate::theme::Theme;
    use aliasmgr_core::{
        doctor::DoctorFinding,
        model::{AliasRecord, PathMode, ShellState},
        service::AliasService,
        sync::SyncRun,
    };
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-tui-ui-{label}-{}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn test_app(label: &str) -> (PathBuf, App) {
        let root = temp_root(label);
        let mut app = App::new(root.clone()).unwrap();
        app.theme = Theme::color();
        (root, app)
    }

    fn seed(app: &App, name: &str, executable: &str) -> AliasRecord {
        AliasService::new(&app.database)
            .create(AliasRecord {
                name: name.into(),
                executable: executable.into(),
                fixed_args: vec!["status".into()],
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap()
    }

    fn render_string(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                output.push_str(buffer[(x, y)].symbol());
            }
            output.push('\n');
        }
        output
    }

    #[test]
    fn layout_switches_wide_narrow_and_too_small() {
        assert_eq!(
            browser_layout(Rect::new(0, 0, 120, 40)),
            BrowserLayout::Wide
        );
        assert_eq!(
            browser_layout(Rect::new(0, 0, 80, 24)),
            BrowserLayout::Narrow
        );
        assert_eq!(
            browser_layout(Rect::new(0, 0, 40, 15)),
            BrowserLayout::TooSmall
        );
        assert_eq!(
            browser_layout(Rect::new(0, 0, 79, 24)),
            BrowserLayout::TooSmall
        );
        assert_eq!(
            browser_layout(Rect::new(0, 0, 80, 19)),
            BrowserLayout::TooSmall
        );
        assert_eq!(
            browser_layout(Rect::new(0, 0, 119, 40)),
            BrowserLayout::Narrow
        );
    }

    #[test]
    fn too_small_terminal_renders_an_actionable_message_at_40x15() {
        let (root, mut app) = test_app("small");
        seed(&app, "gs", "git");
        app.reload();
        let rendered = render_string(&mut app, 40, 15);
        assert!(rendered.contains("Terminal too small"));
        assert!(rendered.contains("80x20"));
        // The app did not panic and no layout metrics were invented.
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn empty_list_renders_create_hint() {
        let (root, mut app) = test_app("empty");
        let rendered = render_string(&mut app, 80, 24);
        assert!(rendered.contains("No aliases yet"));
        assert!(rendered.contains("Press n"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_renders_at_both_standard_sizes_with_cjk_and_long_paths() {
        let (root, mut app) = test_app("list");
        let mut alias = seed(&app, "gs", "git");
        alias.fixed_args = vec!["参数 一".into(), String::new()];
        alias.executable = "C:\\very\\long\\path\\with\\spaces\\工具.exe".into();
        alias.description = "中文描述 with a very long line that must not overflow".into();
        alias.tags = vec!["工作".into()];
        AliasService::new(&app.database).update(alias).unwrap();
        app.reload();
        for (width, height) in [(80, 24), (120, 40)] {
            let rendered = render_string(&mut app, width, height);
            assert!(rendered.contains("gs"));
            assert!(rendered.contains("[on]"));
            assert_eq!(rendered.lines().count(), height as usize);
        }
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn detail_masks_environment_values_and_shows_four_layers() {
        let (root, mut app) = test_app("detail");
        let mut alias = AliasRecord {
            name: "secret".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into(), "two words".into()],
            shells: vec![ShellKind::Bash],
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "hunter2".into());
        let saved = AliasService::new(&app.database).create(alias).unwrap();
        app.reload();
        let rendered = render_string(&mut app, 120, 40);
        assert!(rendered.contains("secret"));
        assert!(rendered.contains("TOKEN=***"));
        assert!(!rendered.contains("hunter2"));
        assert!(rendered.contains("current shell"));
        assert!(rendered.contains("argv preview"));
        assert!(rendered.contains("generated"));
        assert!(rendered.contains("loader"));
        assert!(rendered.contains("Reload [bash]"));
        assert!(rendered.contains("two words"));
        let _ = saved;
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editor_renders_fields_empty_args_and_argv_preview() {
        let (root, mut app) = test_app("editor");
        let alias = seed(&app, "gs", "git");
        app.reload();
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('e'),
            crossterm::event::KeyModifiers::NONE,
        ));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.draft.fixed_args = vec![String::new(), "with space".into()];
        }
        let rendered = render_string(&mut app, 120, 40);
        assert!(rendered.contains("Name"));
        assert!(rendered.contains("Fixed args"));
        assert!(rendered.contains("(empty string)"));
        assert!(rendered.contains("argv preview"));
        assert!(rendered.contains("with space"));
        assert!(rendered.contains("Ctrl+S"));
        let _ = alias;
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn status_and_modal_are_visible_as_text_without_colour() {
        let (root, mut app) = test_app("status");
        app.theme = Theme::mono();
        seed(&app, "gs", "git");
        app.reload();
        app.set_status(StatusKind::Error, "something failed");
        let rendered = render_string(&mut app, 80, 24);
        assert!(rendered.contains("[error] something failed"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn deletion_modal_renders_choices() {
        let (root, mut app) = test_app("modal");
        seed(&app, "gs", "git");
        app.reload();
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('d'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let rendered = render_string(&mut app, 80, 24);
        assert!(rendered.contains("Delete alias"));
        assert!(rendered.contains("y confirm"));
        assert!(rendered.contains("n/Esc cancel"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preview_render_masks_environment_values() {
        let (root, mut app) = test_app("preview");
        let mut alias = AliasRecord {
            name: "secret".into(),
            executable: "git".into(),
            shells: vec![ShellKind::Bash],
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "hunter2".into());
        AliasService::new(&app.database).create(alias).unwrap();
        app.reload();
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(matches!(app.screen, Screen::Preview(_)));
        let rendered = render_string(&mut app, 120, 40);
        assert!(rendered.contains("environment values masked"));
        assert!(rendered.contains("***"));
        assert!(!rendered.contains("hunter2"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn doctor_render_shows_findings_and_actions() {
        let (root, mut app) = test_app("doctor");
        seed(&app, "gs", "git");
        app.reload();
        app.screen = Screen::Doctor(DoctorState {
            findings: vec![DoctorFinding {
                shell: Some(ShellKind::Bash),
                severity: "warning",
                code: "loader_missing",
                message: "Shell Bash 加载块未安装".into(),
                action: Some("重新安装 Shell 集成".into()),
                detail: Some("profile: /tmp/.bashrc".into()),
            }],
            selected: 0,
            scroll: 0,
            busy: false,
            error: None,
        });
        let rendered = render_string(&mut app, 100, 30);
        assert!(rendered.contains("[warn]"));
        assert!(rendered.contains("loader_missing"));
        assert!(rendered.contains("action:"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_report_renders_reload_commands_and_never_claims_automatic_reload() {
        let (root, mut app) = test_app("sync-report");
        app.screen = Screen::SyncReport(SyncReportState {
            run: SyncRun {
                revision: 3,
                results: vec![aliasmgr_core::sync::ShellSyncOutcome {
                    shell: ShellKind::Bash,
                    status: ShellStatus::Ok,
                    error: None,
                    error_code: None,
                    generated_path: root.join("generated/bash.sh"),
                    loader_installed: true,
                    loader_error_code: None,
                }],
            },
            scroll: 0,
        });
        let rendered = render_string(&mut app, 120, 30);
        assert!(rendered.contains("reload: source"));
        assert!(rendered.contains("NOT executed automatically"));
        assert!(rendered.contains("[ok]"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn header_tracks_sync_needed_and_up_to_date_including_disabled_aliases() {
        let (root, mut app) = test_app("sync-header");
        let alias = seed(&app, "gs", "git");
        app.database
            .upsert_shell_state(&ShellState {
                shell: ShellKind::Bash,
                applied_revision: alias.revision,
                file_checksum: String::new(),
                loader_installed: false,
                status: ShellStatus::Ok,
                last_error: None,
            })
            .unwrap();
        app.reload();
        let rendered = render_string(&mut app, 120, 24);
        assert!(rendered.contains("sync: up to date"), "{rendered}");

        // Saving advances the revision; the header must say sync is needed.
        let mut edit = alias.clone();
        edit.description = "changed".into();
        AliasService::new(&app.database).update(edit).unwrap();
        app.reload();
        let rendered = render_string(&mut app, 120, 24);
        assert!(rendered.contains("sync: needed"), "{rendered}");

        // A disabled alias still needs the sync that removes its definition.
        let disabled = app.selected_alias_cloned().unwrap();
        AliasService::new(&app.database)
            .set_enabled(&disabled.name, false)
            .unwrap();
        app.reload();
        let rendered = render_string(&mut app, 120, 24);
        assert!(rendered.contains("sync: needed"), "{rendered}");

        // A matching shell_state clears it again.
        let current = app.selected_alias_cloned().unwrap();
        app.database
            .upsert_shell_state(&ShellState {
                shell: ShellKind::Bash,
                applied_revision: current.revision + 10,
                file_checksum: String::new(),
                loader_installed: true,
                status: ShellStatus::Ok,
                last_error: None,
            })
            .unwrap();
        app.reload();
        let rendered = render_string(&mut app, 120, 24);
        assert!(rendered.contains("sync: up to date"), "{rendered}");
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn narrow_layout_switches_to_detail_and_back() {
        let (root, mut app) = test_app("narrow");
        seed(&app, "gs", "git");
        app.reload();
        let list = render_string(&mut app, 80, 24);
        assert!(list.contains("Aliases"));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(app.show_detail);
        let detail = render_string(&mut app, 80, 24);
        assert!(detail.contains("Detail"));
        assert!(detail.contains("Executable"));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(!app.show_detail);
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn wide_layout_enter_focuses_detail_without_switching_to_single_pane() {
        let (root, mut app) = test_app("wide-keys");
        seed(&app, "gs", "git");
        app.reload();
        let _ = render_string(&mut app, 120, 40);
        assert!(app.wide_layout);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(matches!(app.focus, crate::app::Focus::Detail));
        assert!(!app.show_detail);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(matches!(app.focus, crate::app::Focus::List));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn help_render_lists_keys_and_target_type_semantics() {
        let (root, mut app) = test_app("help");
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('?'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let rendered = render_string(&mut app, 120, 70);
        assert!(rendered.contains("BROWSE"));
        assert!(rendered.contains("python_script"));
        assert!(rendered.contains("STATUS LAYERS"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn monochrome_render_keeps_text_markers_and_uses_no_colour_cells() {
        let (root, mut app) = test_app("mono");
        app.theme = Theme::mono();
        seed(&app, "gs", "git");
        app.reload();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        for y in 0..24 {
            for x in 0..80 {
                assert_eq!(buffer[(x, y)].fg, ratatui::style::Color::Reset);
                assert_eq!(buffer[(x, y)].bg, ratatui::style::Color::Reset);
            }
        }
        let text = render_string(&mut app, 80, 24);
        assert!(text.contains("[on]"));
        assert!(text.contains("[info]"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn target_summary_is_unambiguous() {
        let alias = AliasRecord {
            executable: "git".into(),
            fixed_args: vec!["status".into(), "two words".into()],
            ..Default::default()
        };
        assert_eq!(target_summary(&alias), "git +2 args");
        let cd = AliasRecord {
            executable: "/tmp".into(),
            target_type: TargetType::ChangeDirectory,
            path_mode: PathMode::Absolute,
            ..Default::default()
        };
        assert_eq!(target_summary(&cd), "cd /tmp");
    }

    #[test]
    fn path_mode_and_shells_render_in_the_editor() {
        let (root, mut app) = test_app("advanced");
        let alias = seed(&app, "gs", "git");
        app.reload();
        app.screen = Screen::Editor(Box::new(Editor::new(Some(&alias))));
        if let Screen::Editor(editor) = &mut app.screen {
            editor.toggle_advanced();
        }
        let rendered = render_string(&mut app, 120, 40);
        assert!(rendered.contains("Working dir"));
        assert!(rendered.contains("Environment"));
        assert!(rendered.contains("Path mode"));
        assert!(rendered.contains("Restriction"));
        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }
}
