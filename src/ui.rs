//! Drawing the interface with ratatui.
//!
//! The screen is split into: tab bar (top), system summary (left, with the logo above it
//! if enabled in the configuration), content of the active tab (right) and keyboard
//! shortcuts (bottom). Every function only reads [`App`], except the tables' selection
//! state, which ratatui needs to be mutable.

use std::{
    collections::VecDeque,
    sync::{LazyLock, OnceLock},
};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Sparkline, Table, Tabs, Wrap,
    },
};

use crate::{
    app::{App, Mode, ProcessSort, Tab},
    config::Theme,
    i18n,
    logo::Sprite,
    system::{
        BatteryState, HISTORY_LEN, Process, ProcessState, Temperature, fmt_bytes, fmt_duration,
        fmt_rate,
    },
};

static THEME: OnceLock<Theme> = OnceLock::new();

/// Sets the interface colors. Called once at startup, before the first draw.
pub fn set_theme(theme: Theme) {
    let _ = THEME.set(theme);
}

fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::default)
}

// ───────────────────────────── helpers ─────────────────────────────

fn titled_block(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme().muted))
        .title(Span::styled(
            title.into(),
            Style::new().fg(theme().accent).add_modifier(Modifier::BOLD),
        ))
}

fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

fn pct_color(p: f64) -> Color {
    if p < 60.0 {
        Color::Green
    } else if p < 85.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn temp_color(t: f32) -> Color {
    if t < 60.0 {
        Color::Green
    } else if t < 80.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// `label ▕██████░░░░▏ text` on a single line, colored by the percentage.
fn bar(label: &str, label_width: usize, width: u16, pct: f64, text: String) -> Line<'static> {
    bar_colored(label, label_width, width, pct, text, pct_color(pct))
}

fn bar_colored(
    label: &str,
    label_width: usize,
    width: u16,
    pct: f64,
    text: String,
    color: Color,
) -> Line<'static> {
    let pct = pct.clamp(0.0, 100.0);
    let bar_width = (width as usize)
        .saturating_sub(label_width + text.chars().count() + 3)
        .max(4);
    let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
    Line::from(vec![
        Span::styled(
            format!("{label:<label_width$}"),
            Style::new().fg(theme().label),
        ),
        Span::styled("▕", Style::new().fg(theme().muted)),
        Span::styled("█".repeat(filled), Style::new().fg(color)),
        Span::styled(
            "░".repeat(bar_width - filled),
            Style::new().fg(Color::Rgb(55, 55, 65)),
        ),
        Span::styled("▏", Style::new().fg(theme().muted)),
        Span::raw(format!(" {text}")),
    ])
}

fn key_value(k: &str, v: impl Into<String>, width: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<width$}"), Style::new().fg(theme().label)),
        Span::raw(v.into()),
    ])
}

fn key_temp(k: &str, t: f32, width: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<width$}"), Style::new().fg(theme().label)),
        Span::styled(format!("{t:.0} °C"), Style::new().fg(temp_color(t))),
    ])
}

/// Last `n` values of the history (the most recent ends up on the right).
fn last_n(h: &VecDeque<u64>, n: usize) -> Vec<u64> {
    h.iter().skip(h.len().saturating_sub(n)).copied().collect()
}

fn chart(
    f: &mut Frame,
    area: Rect,
    title: String,
    data: &VecDeque<u64>,
    max: Option<u64>,
    color: Color,
) {
    let block = titled_block(title);
    let v = last_n(data, block.inner(area).width as usize);
    let mut s = Sparkline::default()
        .block(block)
        .data(&v)
        .style(Style::new().fg(color));
    if let Some(m) = max {
        s = s.max(m);
    }
    f.render_widget(s, area);
}

fn pct(a: u64, b: u64) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 * 100.0 / b as f64
    }
}

fn header_row<const N: usize>(c: [String; N]) -> Row<'static> {
    Row::new(c).style(bold(Color::Yellow))
}

// ───────────────────────────── main view ─────────────────────────────

pub fn draw(f: &mut Frame, app: &mut App) {
    let [top_bar, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_tabs(f, app, top_bar);

    let left_width = (body.width * 34 / 100).clamp(36, 64).min(body.width / 2);
    let [left, right] =
        Layout::horizontal([Constraint::Length(left_width), Constraint::Min(30)]).areas(body);

    let system_lines = system_panel(app);
    // Without the logo, the System panel takes the whole left column.
    let system_area = if app.logo {
        let system_height = (system_lines.len() as u16 + 2).min(left.height.saturating_sub(6));
        let [logo, system_area] =
            Layout::vertical([Constraint::Min(6), Constraint::Length(system_height)]).areas(left);
        draw_logo(f, logo);
        system_area
    } else {
        left
    };
    f.render_widget(
        Paragraph::new(system_lines).block(titled_block(padded(i18n::t().title_system))),
        system_area,
    );

    match app.tab {
        Tab::Summary => tab_summary(f, app, right),
        Tab::Cpu => tab_cpu(f, app, right),
        Tab::Ram => tab_ram(f, app, right),
        Tab::Storage => tab_storage(f, app, right),
        Tab::Gpu => tab_gpu(f, app, right),
        Tab::Network => tab_network(f, app, right),
        Tab::Processes => tab_processes(f, app, right),
    }

    draw_footer(f, app, footer);

    match &app.mode {
        Mode::Help => draw_help(f),
        Mode::Confirm { pid, name, force } => draw_confirm(f, *pid, name, *force),
        _ => {}
    }
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, p)| {
            Line::from(vec![
                Span::styled(format!("{} ", i + 1), Style::new().fg(theme().muted)),
                Span::raw(p.title()),
            ])
        })
        .collect();
    let sel = Tab::ALL.iter().position(|p| *p == app.tab).unwrap_or(0);

    let info = &app.monitor.info;
    let mut right_side = vec![Span::styled(
        format!(" {}@{} ", info.user, info.host),
        Style::new().fg(Color::Gray),
    )];
    if app.paused {
        right_side.push(Span::styled(
            format!(" ⏸ {} ", i18n::t().paused),
            Style::new().fg(Color::Black).bg(Color::Yellow),
        ));
        right_side.push(Span::raw(" "));
    }
    let block = titled_block(" memtop ").title(Line::from(right_side).alignment(Alignment::Right));
    let tabs = Tabs::new(titles)
        .select(sel)
        .block(block)
        .style(Style::new().fg(Color::Gray))
        .highlight_style(
            Style::new()
                .fg(Color::Black)
                .bg(theme().accent)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::new().fg(theme().muted)));
    f.render_widget(tabs, area);
}

fn draw_logo(f: &mut Frame, area: Rect) {
    static LOGO: LazyLock<Sprite> = LazyLock::new(Sprite::logo);
    let block = titled_block(" memtop ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    draw_sprite(f, &LOGO, inner);
}

/// Draws the sprite scaled (nearest neighbor) and centered in `area`,
/// using `▀`/`▄` to fit two pixels in each cell.
fn draw_sprite(f: &mut Frame, sp: &Sprite, area: Rect) {
    let s = sp.scale(area.width, area.height);
    if s <= 0.0 {
        return;
    }
    let width = ((sp.width as f64 * s).round() as u16).min(area.width);
    let height_px = ((sp.height as f64 * s).round() as u16).min(area.height * 2);
    let rows = height_px.div_ceil(2);
    let x0 = area.x + (area.width - width) / 2;
    let y0 = area.y + (area.height - rows) / 2;
    let source = |t: u16, max: usize| ((t as f64 / s) as usize).min(max - 1);

    let buf = f.buffer_mut();
    for cy in 0..rows {
        for cx in 0..width {
            let sx = source(cx, sp.width);
            let upper = sp.pixel(sx, source(cy * 2, sp.height));
            let lower = if cy * 2 + 1 < height_px {
                sp.pixel(sx, source(cy * 2 + 1, sp.height))
            } else {
                None
            };
            let (symbol, style) = match (upper, lower) {
                (None, None) => continue,
                (Some(a), None) => ("▀", Style::new().fg(a)),
                (None, Some(b)) => ("▄", Style::new().fg(b)),
                (Some(a), Some(b)) => ("▀", Style::new().fg(a).bg(b)),
            };
            if let Some(cell) = buf.cell_mut((x0 + cx, y0 + cy)) {
                cell.set_symbol(symbol).set_style(style);
            }
        }
    }
}

/// `"Memory"` → `" Memory "`: block titles have a space on each side.
fn padded(text: &str) -> String {
    format!(" {text} ")
}

fn process_state_text(e: ProcessState) -> &'static str {
    let t = i18n::t();
    match e {
        ProcessState::Running => t.proc_running,
        ProcessState::Sleeping => t.proc_sleeping,
        ProcessState::Idle => t.proc_idle,
        ProcessState::DiskWait => t.proc_disk_wait,
        ProcessState::Stopped => t.proc_stopped,
        ProcessState::Zombie => t.proc_zombie,
        ProcessState::Other => t.proc_other,
    }
}

fn battery_state_text(e: BatteryState) -> &'static str {
    let t = i18n::t();
    match e {
        BatteryState::Charging => t.battery_charging,
        BatteryState::Discharging => t.battery_discharging,
        BatteryState::Full => t.battery_full,
        BatteryState::NotCharging => t.battery_not_charging,
        BatteryState::Unknown => t.battery_unknown,
    }
}

/// fastfetch-style block.
fn system_panel(app: &App) -> Vec<Line<'static>> {
    let t = i18n::t();
    let m = &app.monitor;
    let i = &m.info;
    let w = 11;
    let mut l = vec![
        Line::from(vec![
            Span::styled(i.user.clone(), bold(theme().accent)),
            Span::raw("@"),
            Span::styled(i.host.clone(), bold(theme().accent)),
        ]),
        Line::styled(
            "─".repeat(i.user.chars().count() + i.host.chars().count() + 1),
            Style::new().fg(theme().muted),
        ),
    ];
    if let Some(e) = &i.model {
        l.push(key_value(t.machine, e.clone(), w));
    }
    l.push(key_value(t.os, i.os.clone(), w));
    l.push(key_value(t.kernel, i.kernel.clone(), w));
    l.push(key_value(t.uptime, fmt_duration(m.uptime), w));
    if let Some(e) = &i.desktop {
        l.push(key_value(t.desktop, e.clone(), w));
    }
    if let Some(s) = &i.shell {
        l.push(key_value(t.shell, s.clone(), w));
    }
    if let Some(term) = &i.terminal {
        l.push(key_value(t.terminal, term.clone(), w));
    }
    l.push(key_value(
        t.cpu,
        format!("{} ({})", i.cpu_model, i.threads),
        w,
    ));
    for g in &m.gpus {
        l.push(key_value(t.gpu, format!("{} {}", g.vendor, g.name), w));
    }
    l.push(key_value(
        t.memory,
        format!(
            "{} / {}",
            fmt_bytes(m.memory.used()),
            fmt_bytes(m.memory.total)
        ),
        w,
    ));
    if let Some(root) = m.mounts.iter().find(|x| x.mount_point == "/") {
        l.push(key_value(
            t.root_disk,
            format!(
                "{} / {} ({})",
                fmt_bytes(root.used),
                fmt_bytes(root.total),
                root.fs
            ),
            w,
        ));
    }
    if let Some(b) = &m.battery {
        l.push(key_value(
            t.battery,
            format!("{}% ({})", b.percent, battery_state_text(b.state)),
            w,
        ));
    }
    l.push(Line::raw(""));
    l.push(Line::from(
        [
            Color::Black,
            Color::Red,
            Color::Green,
            Color::Yellow,
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
            Color::White,
        ]
        .into_iter()
        .map(|c| Span::styled("███", Style::new().fg(c)))
        .collect::<Vec<_>>(),
    ));
    l
}

// ───────────────────────────── tabs ─────────────────────────────

fn tab_summary(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let m = &app.monitor;
    let active: Vec<_> = m
        .interfaces
        .iter()
        .filter(|i| i.physical && !i.ips.is_empty())
        .collect();
    let storage_height = m.mounts.len().max(1) as u16 + 2;
    let others_height =
        (m.gpus.len() + active.len().max(1) + usize::from(m.battery.is_some())) as u16 + 2;
    let [cpu, mem, storage_area, others_area, top] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(storage_height),
        Constraint::Length(others_height),
        Constraint::Min(4),
    ])
    .areas(area);

    // CPU: bar + chart
    let block = titled_block(padded(t.cpu));
    let inner = block.inner(cpu);
    f.render_widget(block, cpu);
    let [l1, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let mut extra = format!("{:5.1}%", m.cpu_total);
    if let Some(celsius) = m.cpu_temperature() {
        extra.push_str(&format!("  {celsius:.0}°C"));
    }
    let freq = m.cpu_freq_mhz.iter().max().copied().unwrap_or(0);
    if freq > 0 {
        extra.push_str(&format!("  {:.2} GHz", freq as f64 / 1000.0));
    }
    f.render_widget(bar(t.usage, 8, l1.width, m.cpu_total as f64, extra), l1);
    let v = last_n(&m.hist_cpu, chart_area.width as usize);
    f.render_widget(
        Sparkline::default()
            .data(&v)
            .max(100)
            .style(Style::new().fg(theme().accent)),
        chart_area,
    );

    // Memory
    let mm = &m.memory;
    let w = mem.width.saturating_sub(2);
    let lines = vec![
        bar(
            "RAM",
            8,
            w,
            pct(mm.used(), mm.total),
            format!("{} / {}", fmt_bytes(mm.used()), fmt_bytes(mm.total)),
        ),
        bar(
            t.swap,
            8,
            w,
            pct(mm.swap_used(), mm.swap_total),
            format!(
                "{} / {}",
                fmt_bytes(mm.swap_used()),
                fmt_bytes(mm.swap_total)
            ),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(titled_block(padded(t.memory))),
        mem,
    );

    // Storage
    let label_width = m
        .mounts
        .iter()
        .map(|x| x.mount_point.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 18)
        + 1;
    let lines: Vec<Line> = m
        .mounts
        .iter()
        .map(|x| {
            bar(
                &x.mount_point,
                label_width,
                storage_area.width.saturating_sub(2),
                pct(x.used, x.total),
                format!("{} / {}", fmt_bytes(x.used), fmt_bytes(x.total)),
            )
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(titled_block(padded(t.tab_storage))),
        storage_area,
    );

    // GPU, network, battery
    let mut lines = Vec::new();
    for g in &m.gpus {
        let state = if g.is_suspended() {
            Span::styled(t.gpu_idle, Style::new().fg(theme().muted))
        } else {
            Span::styled(t.gpu_active, Style::new().fg(Color::Green))
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<9}", t.gpu), Style::new().fg(theme().label)),
            Span::raw(format!("{} · ", g.name)),
            state,
        ]));
    }
    if active.is_empty() {
        lines.push(key_value(t.network, t.network_disconnected, 9));
    }
    for i in &active {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<9}", t.network), Style::new().fg(theme().label)),
            Span::raw(format!("{:<12}", i.name)),
            Span::styled(
                format!("↓ {:<13}", fmt_rate(i.rx_bps)),
                Style::new().fg(Color::Green),
            ),
            Span::styled(
                format!("↑ {}", fmt_rate(i.tx_bps)),
                Style::new().fg(Color::Magenta),
            ),
        ]));
    }
    if let Some(b) = &m.battery {
        // For the battery, more is better: the colors are reversed.
        let c = if b.percent > 40 {
            Color::Green
        } else if b.percent > 15 {
            Color::Yellow
        } else {
            Color::Red
        };
        lines.push(bar_colored(
            t.battery,
            9,
            others_area.width.saturating_sub(2),
            b.percent as f64,
            format!("{}% {}", b.percent, battery_state_text(b.state)),
            c,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(titled_block(padded(t.title_gpu_network_power))),
        others_area,
    );

    let list = app.top(false, top.height.saturating_sub(3) as usize);
    mini_table(
        f,
        top,
        &padded(t.title_top_cpu),
        &list,
        app.monitor.memory.total,
    );
}

fn mini_table(f: &mut Frame, area: Rect, title: &str, list: &[&Process], ram_total: u64) {
    let t = i18n::t();
    let rows = list.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::new().fg(theme().muted)),
            Cell::from(p.name.clone()),
            Cell::from(format!("{:5.1}", p.cpu)).style(Style::new().fg(pct_color(p.cpu as f64))),
            Cell::from(format!("{:5.1}", pct(p.memory, ram_total)))
                .style(Style::new().fg(Color::LightGreen)),
            Cell::from(fmt_bytes(p.memory)),
            Cell::from(p.user.clone()).style(Style::new().fg(Color::Gray)),
        ])
    });
    let t = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header_row(
        [
            t.col_pid, t.col_name, t.col_cpu, t.col_mem, t.col_res, t.col_user,
        ]
        .map(String::from),
    ))
    .block(titled_block(title.to_string()));
    f.render_widget(t, area);
}

fn tab_cpu(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let m = &app.monitor;
    let i = &m.info;
    let [info, chart_area, bottom] = Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(5),
    ])
    .areas(area);

    let fmin = m.cpu_freq_mhz.iter().min().copied().unwrap_or(0);
    let fmax = m.cpu_freq_mhz.iter().max().copied().unwrap_or(0);
    let w = 14;
    let physical = i
        .physical_cores
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());
    let mut l = vec![
        key_value(t.model, i.cpu_model.clone(), w),
        key_value(
            t.cores,
            format!("{physical} {} · {} {}", t.physical, i.threads, t.threads),
            w,
        ),
        key_value(
            t.frequency,
            format!(
                "{:.2} – {:.2} GHz",
                fmin as f64 / 1000.0,
                fmax as f64 / 1000.0
            ),
            w,
        ),
        key_value(
            t.load_average,
            format!(
                "{:.2}  {:.2}  {:.2}   (1, 5, 15 min)",
                m.load.0, m.load.1, m.load.2
            ),
            w,
        ),
        key_value(t.architecture, i.arch.clone(), w),
    ];
    if let Some(celsius) = m.cpu_temperature() {
        l.push(key_temp(t.temperature, celsius, w));
    }
    f.render_widget(
        Paragraph::new(l).block(titled_block(padded(t.title_processor))),
        info,
    );

    chart(
        f,
        chart_area,
        format!(
            " {} {:.1}% · {} {HISTORY_LEN} s ",
            t.total_usage, m.cpu_total, t.last
        ),
        &m.hist_cpu,
        Some(100),
        theme().accent,
    );

    let [threads_area, temps] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(bottom);

    // Threads in two columns.
    let block = titled_block(padded(t.title_per_thread));
    let inner = block.inner(threads_area);
    f.render_widget(block, threads_area);
    let n = m.cpu_per_thread.len();
    let half = n.div_ceil(2);
    let [c1, c2] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner);
    for (col, range) in [(c1, 0..half), (c2, half..n)] {
        let lines: Vec<Line> = range
            .map(|k| {
                let u = m.cpu_per_thread[k] as f64;
                let ghz = m.cpu_freq_mhz.get(k).copied().unwrap_or(0) as f64 / 1000.0;
                bar(
                    &format!("{k:>2}"),
                    3,
                    col.width.saturating_sub(1),
                    u,
                    format!("{u:3.0}% {ghz:.1}G"),
                )
            })
            .collect();
        f.render_widget(Paragraph::new(lines), col);
    }

    let mut ts: Vec<&Temperature> = m.temperatures.iter().collect();
    ts.sort_by(|a, b| a.label.cmp(&b.label));
    let width = temps.width.saturating_sub(10) as usize;
    let lines: Vec<Line> = ts
        .iter()
        .map(|sensor| {
            let name: String = sensor.label.chars().take(width).collect();
            let mut s = vec![
                Span::styled(format!("{name:<width$}"), Style::new().fg(Color::Gray)),
                Span::styled(
                    format!("{:>5.0}°C", sensor.celsius),
                    Style::new().fg(temp_color(sensor.celsius)),
                ),
            ];
            if sensor.critical.is_some_and(|c| sensor.celsius >= c) {
                s.push(Span::styled(" !", Style::new().fg(Color::Red)));
            }
            Line::from(s)
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(titled_block(padded(t.title_sensors))),
        temps,
    );
}

fn tab_ram(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let mm = &app.monitor.memory;
    let [bars_area, middle, top] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(11),
        Constraint::Min(5),
    ])
    .areas(area);

    let w = bars_area.width.saturating_sub(2);
    let lines = vec![
        bar(
            t.used,
            10,
            w,
            pct(mm.used(), mm.total),
            format!("{} / {}", fmt_bytes(mm.used()), fmt_bytes(mm.total)),
        ),
        bar_colored(
            t.cache,
            10,
            w,
            pct(mm.cache + mm.buffers, mm.total),
            fmt_bytes(mm.cache + mm.buffers),
            Color::Blue,
        ),
        bar(
            t.swap,
            10,
            w,
            pct(mm.swap_used(), mm.swap_total),
            format!(
                "{} / {}",
                fmt_bytes(mm.swap_used()),
                fmt_bytes(mm.swap_total)
            ),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(titled_block(padded(t.memory))),
        bars_area,
    );

    let [details, chart_area] =
        Layout::horizontal([Constraint::Length(30), Constraint::Min(10)]).areas(middle);
    let w = 13;
    let l = vec![
        key_value(t.total, fmt_bytes(mm.total), w),
        key_value(t.used, fmt_bytes(mm.used()), w),
        key_value(t.available, fmt_bytes(mm.available), w),
        key_value(t.free, fmt_bytes(mm.free), w),
        key_value(t.cache, fmt_bytes(mm.cache), w),
        key_value(t.buffers, fmt_bytes(mm.buffers), w),
        key_value(t.shared, fmt_bytes(mm.shared), w),
        key_value(t.swap_total, fmt_bytes(mm.swap_total), w),
        key_value(t.swap_used, fmt_bytes(mm.swap_used()), w),
    ];
    f.render_widget(
        Paragraph::new(l).block(titled_block(padded(t.title_details))),
        details,
    );
    chart(
        f,
        chart_area,
        format!(
            " {} {:.0}% · {} {HISTORY_LEN} s ",
            t.usage,
            pct(mm.used(), mm.total),
            t.last
        ),
        &app.monitor.hist_ram,
        Some(100),
        Color::LightGreen,
    );

    let list = app.top(true, top.height.saturating_sub(3) as usize);
    mini_table(f, top, &padded(t.title_top_memory), &list, mm.total);
}

fn tab_storage(f: &mut Frame, app: &mut App, area: Rect) {
    let tx = i18n::t();
    let m = &app.monitor;
    let disks_height = m.disks.len() as u16 + 4;
    let [disks_area, parts] =
        Layout::vertical([Constraint::Length(disks_height), Constraint::Min(4)]).areas(area);

    let rows = m.disks.iter().map(|d| {
        Row::new(vec![
            Cell::from(d.name.clone()).style(Style::new().fg(theme().accent)),
            Cell::from(d.model.clone().unwrap_or_else(|| "—".into())),
            Cell::from(d.kind).style(Style::new().fg(Color::LightMagenta)),
            Cell::from(fmt_bytes(d.size)),
            match d.temperature {
                Some(t) => Cell::from(format!("{t:.0}°C")).style(Style::new().fg(temp_color(t))),
                None => Cell::from("—").style(Style::new().fg(theme().muted)),
            },
            Cell::from(fmt_rate(d.read_bps)).style(Style::new().fg(Color::Green)),
            Cell::from(fmt_rate(d.write_bps)).style(Style::new().fg(Color::Magenta)),
        ])
    });
    let t = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Fill(1),
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(
        header_row(
            [
                tx.col_disk,
                tx.model,
                tx.col_type,
                tx.col_size,
                tx.col_temp,
                tx.col_read,
                tx.col_write,
            ]
            .map(String::from),
        )
        .bottom_margin(1),
    )
    .block(titled_block(padded(tx.title_disks)));
    f.render_widget(t, disks_area);

    let block = titled_block(padded(tx.title_partitions));
    let inner = block.inner(parts);
    f.render_widget(block, parts);
    let mut lines = Vec::new();
    for x in &m.mounts {
        let p = pct(x.used, x.total);
        lines.push(Line::from(vec![
            Span::styled(x.mount_point.clone(), bold(theme().accent)),
            Span::styled(
                format!("  {} · {}", x.device, x.fs),
                Style::new().fg(Color::Gray),
            ),
        ]));
        lines.push(bar(
            "",
            0,
            inner.width,
            p,
            format!(
                "{p:.0}%  {} {} · {} {} {}",
                fmt_bytes(x.used),
                tx.used_space,
                fmt_bytes(x.total - x.used),
                tx.free_of,
                fmt_bytes(x.total)
            ),
        ));
        lines.push(Line::raw(""));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn tab_gpu(f: &mut Frame, app: &mut App, area: Rect) {
    let tx = i18n::t();
    let gpus = &app.monitor.gpus;
    if gpus.is_empty() {
        f.render_widget(
            Paragraph::new(tx.gpu_none).block(titled_block(padded(tx.gpu))),
            area,
        );
        return;
    }
    let areas = Layout::vertical(gpus.iter().map(|_| Constraint::Fill(1))).split(area);
    for (g, a) in gpus.iter().zip(areas.iter()) {
        let w = 13;
        let width = a.width.saturating_sub(2);
        let state = match g.power_state.as_deref() {
            Some("suspended") => Span::styled(tx.gpu_state_idle, Style::new().fg(theme().muted)),
            Some("active") => Span::styled(tx.gpu_state_active, Style::new().fg(Color::Green)),
            Some(o) => Span::raw(o.to_string()),
            None => Span::raw("—"),
        };
        let mut l = vec![
            key_value(tx.vendor, g.vendor.clone(), w),
            key_value(tx.driver, g.driver.clone().unwrap_or_else(|| "—".into()), w),
            key_value(tx.pci, g.address.clone(), w),
            Line::from(vec![
                Span::styled(format!("{:<w$}", tx.state), Style::new().fg(theme().label)),
                state,
            ]),
        ];
        if let Some(u) = g.usage {
            l.push(bar(tx.usage, w, width, u as f64, format!("{u:.0}%")));
        }
        if let (Some(u), Some(t)) = (g.vram_used, g.vram_total) {
            l.push(bar(
                tx.vram,
                w,
                width,
                pct(u, t),
                format!("{} / {}", fmt_bytes(u), fmt_bytes(t)),
            ));
        }
        if let Some(fr) = g.clock_mhz {
            let max = g
                .max_clock_mhz
                .map(|m| format!(" / {m} MHz"))
                .unwrap_or_default();
            l.push(key_value(tx.frequency, format!("{fr} MHz{max}"), w));
        }
        if let Some(t) = g.temperature {
            l.push(key_temp(tx.temperature, t, w));
        }
        if let Some(p) = g.power_w {
            l.push(key_value(tx.power, format!("{p:.1} W"), w));
        }
        if g.vendor.contains("NVIDIA") && g.usage.is_none() && !g.is_suspended() {
            l.push(Line::styled(
                tx.gpu_querying,
                Style::new().fg(theme().muted),
            ));
        }
        if g.is_suspended() {
            l.push(Line::styled(
                tx.gpu_not_waking,
                Style::new()
                    .fg(theme().muted)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
        f.render_widget(
            Paragraph::new(l).block(titled_block(format!(" {} ", g.name))),
            *a,
        );
    }
}

fn tab_network(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let m = &app.monitor;
    let [chart_area, table_area] =
        Layout::vertical([Constraint::Length(9), Constraint::Min(4)]).areas(area);
    let [rx, tx] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(chart_area);
    let rx_now: f64 = m.interfaces.iter().map(|i| i.rx_bps).sum();
    let tx_now: f64 = m.interfaces.iter().map(|i| i.tx_bps).sum();
    chart(
        f,
        rx,
        format!(" ↓ {} {} ", t.download, fmt_rate(rx_now)),
        &m.hist_rx,
        None,
        Color::Green,
    );
    chart(
        f,
        tx,
        format!(" ↑ {} {} ", t.upload, fmt_rate(tx_now)),
        &m.hist_tx,
        None,
        Color::Magenta,
    );

    let rows = m.interfaces.iter().map(|i| {
        let is_active = !i.ips.is_empty();
        Row::new(vec![
            Cell::from(i.name.clone()).style(Style::new().fg(if is_active {
                theme().accent
            } else {
                theme().muted
            })),
            Cell::from(summarize_ips(&i.ips)),
            Cell::from(i.mac.clone()).style(Style::new().fg(Color::Gray)),
            Cell::from(fmt_rate(i.rx_bps)).style(Style::new().fg(Color::Green)),
            Cell::from(fmt_rate(i.tx_bps)).style(Style::new().fg(Color::Magenta)),
            Cell::from(fmt_bytes(i.rx_total)),
            Cell::from(fmt_bytes(i.tx_total)),
        ])
    });
    let t = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Fill(1),
            Constraint::Length(17),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(9),
            Constraint::Length(9),
        ],
    )
    .header(
        header_row([
            t.col_interface.to_string(),
            "IP".into(),
            "MAC".into(),
            format!("↓ {}", t.col_now),
            format!("↑ {}", t.col_now),
            format!("↓ {}", t.col_total),
            format!("↑ {}", t.col_total),
        ])
        .bottom_margin(1),
    )
    .block(titled_block(padded(t.title_interfaces)));
    f.render_widget(t, table_area);
}

/// Full IPv4 addresses and the rest summarized: "192.0.2.10/24 +3 v6".
fn summarize_ips(ips: &[String]) -> String {
    if ips.is_empty() {
        return "—".into();
    }
    let (v4, v6): (Vec<&String>, Vec<&String>) = ips.iter().partition(|ip| ip.contains('.'));
    let mut s = v4.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
    match (s.is_empty(), v6.len()) {
        (_, 0) => {}
        (true, 1) => s = v6[0].clone(),
        (true, n) => s = format!("{} +{} v6", v6[0], n - 1),
        (false, n) => s.push_str(&format!(" +{n} v6")),
    }
    s
}

/// Splits the command line into (path, program, arguments) to color it.
fn command_parts(p: &Process) -> (String, String, String) {
    if p.command.is_empty() {
        return (String::new(), format!("[{}]", p.name), String::new());
    }
    let (prog, args) = p.command.split_once(' ').unwrap_or((&p.command, ""));
    let (dir, base) = match prog.rsplit_once('/') {
        Some((d, b)) => (format!("{d}/"), b.to_string()),
        None => (String::new(), prog.to_string()),
    };
    // If the program does not look like the process name (e.g. "RDD Process"), show the name first.
    if !base.starts_with(&p.name) && !p.name.starts_with(&base) {
        return (String::new(), p.name.clone(), format!("  {}", p.command));
    }
    let args = if args.is_empty() {
        String::new()
    } else {
        format!(" {args}")
    };
    (dir, base, args)
}

fn tab_processes(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let total_ram = app.monitor.memory.total;
    let arrow = |o: ProcessSort| {
        if app.process_sort == o {
            if app.descending { " ↓" } else { " ↑" }
        } else {
            ""
        }
    };
    let header = header_row([
        format!("{}{}", t.col_pid, arrow(ProcessSort::Pid)),
        format!("{}{}", t.col_user, arrow(ProcessSort::User)),
        format!("{}{}", t.col_cpu, arrow(ProcessSort::Cpu)),
        format!("{}{}", t.col_mem, arrow(ProcessSort::Memory)),
        t.col_res.into(),
        t.state.into(),
        t.col_cpu_time.into(),
        format!("{}{}", t.col_command, arrow(ProcessSort::Name)),
    ]);

    let list = app.processes();
    let n = list.len();
    let rows: Vec<Row> = list
        .iter()
        .map(|p| {
            let mem_pct = pct(p.memory, total_ram);
            let (dir, prog, args) = command_parts(p);
            Row::new(vec![
                Cell::from(p.pid.to_string()).style(Style::new().fg(theme().muted)),
                Cell::from(p.user.clone()).style(Style::new().fg(Color::Gray)),
                Cell::from(format!("{:5.1}", p.cpu))
                    .style(Style::new().fg(pct_color(p.cpu as f64))),
                Cell::from(format!("{mem_pct:5.1}"))
                    .style(Style::new().fg(pct_color(mem_pct * 3.0))),
                Cell::from(fmt_bytes(p.memory)),
                Cell::from(process_state_text(p.state)).style(Style::new().fg(Color::Gray)),
                Cell::from(fmt_cpu_time(p.cpu_time)).style(Style::new().fg(Color::Gray)),
                Cell::from(Line::from(vec![
                    Span::styled(dir, Style::new().fg(theme().muted)),
                    Span::styled(prog, Style::new().add_modifier(Modifier::BOLD)),
                    Span::styled(args, Style::new().fg(Color::Gray)),
                ])),
            ])
        })
        .collect();

    let mut title = format!(" {} ({n}) ", t.tab_processes);
    if app.mine_only {
        title.push_str(&format!("· {} ", t.mine_only));
    }
    let mut block = titled_block(title);
    if app.mode == Mode::Searching || !app.search.is_empty() {
        let cursor = if app.mode == Mode::Searching {
            "▏"
        } else {
            ""
        };
        block = block.title(
            Line::from(vec![
                Span::styled(" / ", bold(Color::Yellow)),
                Span::styled(
                    format!("{}{cursor} ", app.search),
                    Style::new().fg(Color::Yellow),
                ),
            ])
            .alignment(Alignment::Right),
        );
    }

    let t = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Fill(1),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::new()
            .bg(theme().selection)
            .add_modifier(Modifier::BOLD),
    )
    .block(block);
    f.render_stateful_widget(t, area, &mut app.process_table);

    if n > area.height.saturating_sub(3) as usize {
        let mut sb = ScrollbarState::new(n).position(app.process_table.selected().unwrap_or(0));
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(theme().muted)),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut sb,
        );
    }
}

fn fmt_cpu_time(s: u64) -> String {
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h >= 100 {
        format!("{}d{}h", h / 24, h % 24)
    } else {
        format!("{h}:{m:02}:{s:02}")
    }
}

// ───────────────────────────── footer and dialogs ─────────────────────────────

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    if let Some((txt, error, _)) = &app.notice {
        let c = if *error { Color::Red } else { Color::Green };
        f.render_widget(
            Paragraph::new(Line::styled(format!(" {txt}"), Style::new().fg(c))),
            area,
        );
        return;
    }
    let t = i18n::t();
    let pairs: &[(&str, &str)] = match (&app.mode, app.tab) {
        (Mode::Searching, _) => &[
            ("Enter", t.footer_accept),
            ("Esc", t.footer_clear),
            ("↑↓", t.footer_move),
        ],
        (_, Tab::Processes) => &[
            ("↑↓", t.footer_move),
            ("/", t.footer_search),
            ("c m p n u", t.footer_sort),
            ("o", t.mine_only),
            ("t", t.footer_terminate),
            ("K", t.footer_kill),
            ("←→", t.footer_tab),
            ("?", t.footer_help),
            ("q", t.footer_quit),
        ],
        _ => &[
            ("←→/Tab", t.footer_tab),
            ("1-7", t.footer_go_to),
            (t.key_space, t.footer_pause),
            ("?", t.footer_help),
            ("q", t.footer_quit),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    for (k, d) in pairs {
        spans.push(Span::styled(
            *k,
            Style::new().fg(Color::Black).bg(theme().accent),
        ));
        spans.push(Span::styled(
            format!(" {d}  "),
            Style::new().fg(Color::Gray),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let (w, h) = (w.min(area.width), h.min(area.height));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_confirm(f: &mut Frame, pid: u32, name: &str, force: bool) {
    let t = i18n::t();
    let r = centered(f.area(), 58, 7);
    f.render_widget(Clear, r);
    let signal = if force {
        format!("SIGKILL {}", t.confirm_force)
    } else {
        "SIGTERM".to_string()
    };
    let l = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw(format!(" {} ", t.confirm_send)),
            Span::styled(signal, bold(Color::Red)),
            Span::raw(format!(" {} ", t.confirm_to)),
            Span::styled(
                format!("{name} ({pid})"),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::raw(""),
        Line::styled(
            format!(" {}", t.confirm_options),
            Style::new().fg(theme().muted),
        ),
    ];
    let block = titled_block(padded(t.confirm_title)).border_style(Style::new().fg(Color::Red));
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }).block(block), r);
}

fn draw_help(f: &mut Frame) {
    let t = i18n::t();
    let sections: [(&str, &[(&str, &str)]); 2] = [
        (
            t.help_general,
            &[
                ("← → / Tab / h l", t.help_switch_tab),
                ("1 … 7", t.help_go_to_tab),
                (t.key_space, t.help_pause),
                ("q / Esc", t.help_quit),
            ],
        ),
        (
            t.tab_processes,
            &[
                ("↑ ↓ / j k", t.help_move),
                ("/", t.help_search),
                ("c m p n u", t.help_sort),
                ("", t.help_reverse),
                ("o", t.help_mine_only),
                ("x", t.help_clear_filters),
                ("t / F9", t.help_terminate),
                ("K", t.help_kill),
            ],
        ),
    ];
    let mut l = Vec::new();
    for (title, rows) in sections {
        l.push(Line::styled(format!(" {title}"), bold(Color::Yellow)));
        for (k, d) in rows {
            l.push(Line::from(vec![
                Span::styled(format!("   {k:<18}"), Style::new().fg(theme().accent)),
                Span::raw(*d),
            ]));
        }
        l.push(Line::raw(""));
    }
    let r = centered(f.area(), 72, l.len() as u16 + 2);
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(l).block(titled_block(padded(t.help_title))),
        r,
    );
}
