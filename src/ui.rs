use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState, Tabs, Wrap,
};
use ratatui::Frame;

use crate::api::PortfolioSummary;
use crate::app::{App, FormState, RowState, Tab};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const GOOD: Color = Color::Green;
const BAD: Color = Color::Red;
const WARN: Color = Color::Yellow;
// const HEADER_BG: Color = Color::Rgb(74, 60, 150);
const HEADER_BG: Color = Color::Rgb(150, 110, 45);
const TOTAL_BG: Color = Color::Rgb(150, 110, 45);

const MASK: &str = "••••";

fn money(app: &App, v: f64) -> String {
    if app.hide_amounts {
        MASK.to_string()
    } else {
        fmt_money(v)
    }
}

fn fmt_money(v: f64) -> String {
    let s = format!("{:.2}", v);
    let (int, frac) = s.split_once('.').unwrap();
    let neg = int.starts_with('-');
    let digits = int.trim_start_matches('-');
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{}.{}", if neg { "-" } else { "" }, grouped, frac)
}

fn fmt_qty(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{:.0}", v)
    } else {
        fmt_money(v)
    }
}

fn pl_span(app: &App, v: f64) -> Span<'static> {
    if app.hide_amounts {
        return Span::styled(MASK, Style::default().fg(MUTED));
    }
    let (color, sign) = if v >= 0.0 { (GOOD, "+") } else { (BAD, "") };
    Span::styled(
        format!("{}{}", sign, fmt_money(v)),
        Style::default().fg(color),
    )
}

fn title_block<'a>(title: &'a str) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .title_alignment(Alignment::Left)
}

fn make_row(lines: Vec<Line<'static>>) -> Row<'static> {
    Row::new(padded_cells(lines)).height(3)
}

fn padded_cells(lines: Vec<Line<'static>>) -> Vec<Cell<'static>> {
    lines
        .into_iter()
        .map(|l| Cell::from(vec![Line::from(""), l, Line::from("")]))
        .collect()
}

fn make_header(lines: Vec<Line<'static>>) -> Row<'static> {
    Row::new(padded_cells(lines))
        .height(3)
        .style(Style::default().bg(HEADER_BG).fg(Color::White))
}

fn make_footer_row(lines: Vec<Line<'static>>) -> Row<'static> {
    Row::new(padded_cells(lines))
        .height(3)
        .style(Style::default().bg(TOTAL_BG).add_modifier(Modifier::BOLD))
}

fn render_selectable_table(
    f: &mut Frame,
    area: Rect,
    title: &str,
    rows: Vec<Row>,
    header: Row,
    widths: Vec<Constraint>,
    selected: usize,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .padding(Padding::new(1, 1, 1, 1))
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .title_alignment(Alignment::Left);

    let inner = block.inner(area);

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(2);

    let mut state = TableState::new();
    state.select(Some(selected));
    f.render_stateful_widget(table, area, &mut state);

    let offset = state.offset();
    let y = inner.y + 3 + (selected.saturating_sub(offset)) as u16 * 3;
    let sel_area = Rect::new(inner.x, y, inner.width, 3);
    let sel_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));
    f.render_widget(sel_block, sel_area);
}

pub fn render(f: &mut Frame, app: &App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(0),
        Constraint::Length(5),
    ])
    .areas(f.area());

    render_header(f, app, header);
    match app.tab {
        Tab::Dashboard => render_dashboard(f, app, body),
        Tab::Accounts => render_accounts(f, app, body),
    }
    render_footer(f, app, footer);

    if let Some(state) = &app.form {
        render_form(f, state);
    } else if let Some(idx) = app.detail {
        render_detail(f, app, idx);
    } else if let Some(idx) = app.confirm_delete {
        render_confirm(f, app, idx);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let tabs = Tabs::new(vec![
        Line::from("  Dashboard  "),
        Line::from("  Accounts  "),
    ])
    .select(match app.tab {
        Tab::Dashboard => 0,
        Tab::Accounts => 1,
    })
    .highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD),
    )
    .divider(Span::raw(""));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .padding(Padding::new(1, 1, 1, 1))
        .title(Line::from(Span::styled(
            " MeroShare TUI ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .title_alignment(Alignment::Left);

    f.render_widget(tabs.block(block), area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let (left, right) =
        if app.tab == Tab::Accounts && app.form.is_none() && app.confirm_delete.is_none() {
            (
                " q quit  |  Tab switch  |  a add  |  d delete  |  s hide  |  r refresh  ",
                (if app.loading {
                    format!("{} fetching...", app.spinner())
                } else {
                    app.message.clone()
                })
                .to_string(),
            )
        } else {
            (
                " q quit  |  Tab switch  |  s hide  |  r refresh  |  h/j/k/l move  ",
                (if app.loading {
                    format!("{} fetching...", app.spinner())
                } else {
                    app.message.clone()
                })
                .to_string(),
            )
        };

    let line = Line::from(vec![
        Span::raw(left),
        Span::styled(right, Style::default().fg(MUTED)),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(line).alignment(Alignment::Left),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
}

fn render_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let [summary, _gap, _table_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    let ok_rows: Vec<&PortfolioSummary> = app
        .rows
        .iter()
        .filter_map(|r| match r {
            RowState::Ok(p) => Some(p),
            _ => None,
        })
        .collect();

    let total_shares: f64 = ok_rows.iter().map(|p| p.total_shares).sum();
    let total_prev: f64 = ok_rows.iter().map(|p| p.prev_close).sum();
    let total_last: f64 = ok_rows.iter().map(|p| p.last_traded).sum();
    let total_pl: f64 = ok_rows.iter().map(|p| p.profit_loss).sum();

    let pl_color = if total_pl >= 0.0 { GOOD } else { BAD };
    let pl_text = if app.hide_amounts {
        MASK.to_string()
    } else {
        format!(
            "{}{}",
            if total_pl >= 0.0 { "+" } else { "" },
            fmt_money(total_pl)
        )
    };
    let stats = Line::from(vec![
        Span::styled("Quantity: ", Style::default().fg(MUTED)),
        Span::styled(
            fmt_qty(total_shares),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled("Prev Close: ", Style::default().fg(MUTED)),
        Span::styled(
            money(app, total_prev),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled("Last Traded: ", Style::default().fg(MUTED)),
        Span::styled(
            money(app, total_last),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled("Total P/L: ", Style::default().fg(MUTED)),
        Span::styled(
            pl_text,
            Style::default().fg(pl_color).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(stats), summary);

    let mut widths = vec![
        Constraint::Length(8),
        Constraint::Length(20),
        Constraint::Length(16),
        Constraint::Length(36),
        Constraint::Length(16),
        Constraint::Length(16),
        Constraint::Length(16),
        Constraint::Length(14),
    ];
    if app.accounts.len() > 20 {
        widths = widths
            .into_iter()
            .map(|c| match c {
                Constraint::Length(l) => Constraint::Min(l),
                other => other,
            })
            .collect();
    }

    let mut rows: Vec<Row> = Vec::new();
    for (i, state) in app.rows.iter().enumerate() {
        let lines = match state {
            RowState::Loading => vec![
                Line::from((i + 1).to_string()).style(Style::default().fg(MUTED)),
                Line::from(
                    app.accounts
                        .get(i)
                        .map(|a| a.name.as_str())
                        .unwrap_or("...")
                        .to_string(),
                )
                .style(Style::default().fg(MUTED)),
                Line::from("...").style(Style::default().fg(MUTED)),
                Line::from("...").style(Style::default().fg(MUTED)),
                Line::from(format!("{}", app.spinner())).style(Style::default().fg(ACCENT)),
                Line::from(""),
                Line::from(""),
                Line::from(""),
            ],
            RowState::Err(e) => vec![
                Line::from((i + 1).to_string()).style(Style::default().fg(MUTED)),
                Line::from(
                    app.accounts
                        .get(i)
                        .map(|a| a.name.as_str())
                        .unwrap_or("?")
                        .to_string(),
                ),
                Line::from("ERR").style(Style::default().fg(BAD).add_modifier(Modifier::BOLD)),
                Line::from(""),
                Line::from(""),
                Line::from(""),
                Line::from(""),
                Line::from(format!("{:.0}", e.chars().take(14).collect::<String>()))
                    .style(Style::default().fg(BAD)),
            ],
            RowState::Ok(p) => vec![
                Line::from((i + 1).to_string())
                    .style(Style::default().fg(MUTED))
                    .alignment(Alignment::Center),
                Line::from(p.name.clone()).add_modifier(Modifier::BOLD),
                Line::from(p.username.clone())
                    .style(Style::default().fg(MUTED))
                    .alignment(Alignment::Center),
                Line::from(p.email.clone()).style(Style::default().fg(MUTED)),
                Line::from(fmt_qty(p.total_shares))
                    .style(Style::default().fg(WARN).add_modifier(Modifier::BOLD))
                    .alignment(Alignment::Center),
                Line::from(money(app, p.prev_close)).add_modifier(Modifier::BOLD),
                Line::from(money(app, p.last_traded)).add_modifier(Modifier::BOLD),
                Line::from(pl_span(app, p.profit_loss)).add_modifier(Modifier::BOLD),
            ],
        };
        rows.push(make_row(lines));
    }

    // rows.push(make_footer_row(vec![
    //     Line::from("—").alignment(Alignment::Center),
    //     Line::from("TOTAL").style(
    //         Style::default()
    //             .fg(Color::Black)
    //             .add_modifier(Modifier::BOLD),
    //     ),
    //     Line::from(""),
    //     Line::from(""),
    //     Line::from(fmt_qty(total_shares)).style(
    //         Style::default()
    //             .fg(Color::Black)
    //             .add_modifier(Modifier::BOLD),
    //     ),
    //     Line::from(fmt_money(total_prev)).add_modifier(Modifier::BOLD),
    //     Line::from(fmt_money(total_last)).add_modifier(Modifier::BOLD),
    //     Line::from(pl_span(total_pl)).add_modifier(Modifier::BOLD),
    // ]));
    //
    let header = make_header(vec![
        Line::from("SN")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Name").add_modifier(Modifier::BOLD),
        Line::from("Username")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Email").add_modifier(Modifier::BOLD),
        Line::from("Qty")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Prev Close").add_modifier(Modifier::BOLD),
        Line::from("Last Traded").add_modifier(Modifier::BOLD),
        Line::from("P/L").add_modifier(Modifier::BOLD),
    ]);

    render_selectable_table(
        f,
        _table_area,
        " Portfolio Overview ",
        rows,
        header,
        widths,
        app.selected,
    );
}

fn render_accounts(f: &mut Frame, app: &App, area: Rect) {
    if app.accounts.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No accounts yet  ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press a to add your first account",
                Style::default().fg(MUTED),
            )),
        ];
        f.render_widget(
            Paragraph::new(text)
                .block(title_block(" Accounts "))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let mut rows: Vec<Row> = Vec::new();
    for (i, acc) in app.accounts.iter().enumerate() {
        let mask = "*".repeat(acc.password.len());
        let email = match app.rows.get(i) {
            Some(RowState::Ok(p)) => p.email.clone(),
            Some(RowState::Loading) => "...".into(),
            _ => "ERR".into(),
        };
        rows.push(make_row(vec![
            Line::from((i + 1).to_string())
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center),
            Line::from(acc.name.clone()).add_modifier(Modifier::BOLD),
            Line::from(acc.client_id.to_string())
                .style(Style::default().fg(WARN).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center),
            Line::from(acc.username.clone())
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center),
            Line::from(email).style(Style::default().fg(MUTED)),
            Line::from(acc.demat.clone())
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center),
            Line::from(acc.client_code.clone())
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center),
            Line::from(mask).style(Style::default().fg(MUTED)),
        ]));
    }

    let header = make_header(vec![
        Line::from("SN")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Name").add_modifier(Modifier::BOLD),
        Line::from("Client ID")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Username")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Email").add_modifier(Modifier::BOLD),
        Line::from("Demat")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Client Code")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Password").add_modifier(Modifier::BOLD),
    ]);

    let widths = vec![
        Constraint::Length(8),
        Constraint::Min(14),
        Constraint::Length(14),
        Constraint::Min(14),
        Constraint::Min(36),
        Constraint::Min(28),
        Constraint::Length(14),
        Constraint::Min(10),
    ];

    render_selectable_table(
        f,
        area,
        " Accounts ",
        rows,
        header,
        widths,
        app.accounts_selected,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

fn render_form(f: &mut Frame, form: &FormState) {
    let area = f.area();
    let popup = centered_rect(46, 55, area);
    let inner = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .padding(Padding::horizontal(2))
        .title(Span::styled(
            " Add Account ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner_area = inner.inner(popup);
    f.render_widget(Clear, popup);
    f.render_widget(inner, popup);

    let mut lines: Vec<Line> = Vec::new();
    for (i, field) in form.fields.iter().enumerate() {
        let value = if field.secret {
            "*".repeat(field.value.len())
        } else {
            field.value.clone()
        };
        let label = Span::styled(format!("{:<13}", field.label), Style::default().fg(MUTED));
        let styled_value = if i == form.focus {
            Span::styled(
                value,
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Rgb(30, 40, 60)),
            )
        } else {
            Span::styled(value, Style::default().fg(Color::White))
        };
        let arrow = if i == form.focus {
            Span::styled("▎ ", Style::default().fg(ACCENT))
        } else {
            Span::raw("  ")
        };
        lines.push(Line::from(vec![arrow, label, styled_value]));
    }

    if let Some(err) = &form.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(err, Style::default().fg(BAD))));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Tab: next  Enter: save  Esc: cancel",
        Style::default().fg(MUTED),
    )));

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner_area);
}

fn render_detail(f: &mut Frame, app: &App, idx: usize) {
    let area = f.area();
    let popup = centered_rect(78, 85, area);

    let name = app.accounts.get(idx).map(|a| a.name.as_str()).unwrap_or("");
    let title = format!(" Holdings: {} ", name);
    f.render_widget(Clear, popup);

    let summary = match app.rows.get(idx) {
        Some(RowState::Ok(p)) => p.clone(),
        _ => return,
    };

    let mut rows: Vec<Row> = Vec::new();
    for (i, h) in summary.holdings.iter().enumerate() {
        let pl = (h.ltp - h.prev_close) * h.qty;
        rows.push(make_row(vec![
            Line::from((i + 1).to_string())
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center),
            Line::from(h.script.clone())
                .style(Style::default().fg(WARN).add_modifier(Modifier::BOLD)),
            Line::from(h.company.clone()),
            Line::from(fmt_qty(h.qty)).alignment(Alignment::Center),
            Line::from(money(app, h.prev_close)),
            Line::from(money(app, h.val_prev_close)),
            Line::from(money(app, h.ltp)),
            Line::from(money(app, h.val_ltp)),
            Line::from(pl_span(app, pl)),
        ]));
    }

    let total_value_ltp: f64 = summary.last_traded;
    let total_value_lcp: f64 = summary.prev_close;
    let total_pl: f64 = summary.profit_loss;

    rows.push(make_footer_row(vec![
        Line::from(""),
        Line::from("TOTAL").style(
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from(fmt_qty(summary.total_shares))
            .style(
                Style::default()
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        Line::from(""),
        Line::from(money(app, total_value_lcp)).style(
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from(money(app, total_value_ltp)).style(
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(pl_span(app, total_pl)).add_modifier(Modifier::BOLD),
    ]));

    let header = make_header(vec![
        Line::from("SN")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("Scrip").add_modifier(Modifier::BOLD),
        Line::from("Company").add_modifier(Modifier::BOLD),
        Line::from("Qty")
            .add_modifier(Modifier::BOLD)
            .alignment(Alignment::Center),
        Line::from("LCP").add_modifier(Modifier::BOLD),
        Line::from("Value of LCP").add_modifier(Modifier::BOLD),
        Line::from("LTP").add_modifier(Modifier::BOLD),
        Line::from("Value of LTP").add_modifier(Modifier::BOLD),
        Line::from("P/L").add_modifier(Modifier::BOLD),
    ]);

    let widths = vec![
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(18),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(10),
    ];

    render_selectable_table(
        f,
        popup,
        &title,
        rows,
        header,
        widths,
        app.detail_selected,
    );
}

fn render_confirm(f: &mut Frame, app: &App, idx: usize) {
    let area = f.area();
    let popup = centered_rect(44, 25, area);
    let name = app
        .accounts
        .get(idx)
        .map(|a| a.name.as_str())
        .unwrap_or("?");
    let inner = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BAD))
        .title(Span::styled(
            " Delete Account ",
            Style::default().fg(BAD).add_modifier(Modifier::BOLD),
        ));
    let inner_area = inner.inner(popup);
    f.render_widget(Clear, popup);
    f.render_widget(inner, popup);

    let lines = vec![
        Line::from(Span::styled(
            format!("Delete {}?", name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " y: yes   n: cancel ",
            Style::default().fg(MUTED),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        inner_area,
    );
}
