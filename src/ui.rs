//! Dibujo de la interfaz con ratatui.
//!
//! La pantalla se divide en: barra de pestañas (arriba), resumen del sistema (izquierda,
//! con el logo encima si se activó en la configuración), contenido de la pestaña activa (derecha) y atajos de teclado (abajo).
//! Todas las funciones son de solo lectura sobre [`App`], salvo el estado de selección
//! de las tablas, que ratatui necesita mutable.

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
    app::{App, Modo, OrdenProc, Pestana},
    config::Theme,
    i18n,
    logo::Sprite,
    sistema::{
        EstadoBateria, EstadoProceso, HISTORIAL, Proceso, Temperatura, fmt_bytes, fmt_duracion,
        fmt_tasa,
    },
};

static TEMA: OnceLock<Theme> = OnceLock::new();

/// Fija los colores de la interfaz. Se llama una vez al arrancar, antes del primer dibujo.
pub fn fijar_tema(tema: Theme) {
    let _ = TEMA.set(tema);
}

fn tema() -> &'static Theme {
    TEMA.get_or_init(Theme::default)
}

// ───────────────────────────── utilidades ─────────────────────────────

fn bloque(titulo: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(tema().muted))
        .title(Span::styled(
            titulo.into(),
            Style::new().fg(tema().accent).add_modifier(Modifier::BOLD),
        ))
}

fn negrita(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

fn color_pct(p: f64) -> Color {
    if p < 60.0 {
        Color::Green
    } else if p < 85.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn color_temp(t: f32) -> Color {
    if t < 60.0 {
        Color::Green
    } else if t < 80.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// `etiqueta ▕██████░░░░▏ texto` en una sola línea, con el color según el porcentaje.
fn barra(etiqueta: &str, ancho_etq: usize, ancho: u16, pct: f64, texto: String) -> Line<'static> {
    barra_color(etiqueta, ancho_etq, ancho, pct, texto, color_pct(pct))
}

fn barra_color(
    etiqueta: &str,
    ancho_etq: usize,
    ancho: u16,
    pct: f64,
    texto: String,
    color: Color,
) -> Line<'static> {
    let pct = pct.clamp(0.0, 100.0);
    let ancho_barra = (ancho as usize)
        .saturating_sub(ancho_etq + texto.chars().count() + 3)
        .max(4);
    let lleno = ((pct / 100.0) * ancho_barra as f64).round() as usize;
    Line::from(vec![
        Span::styled(
            format!("{etiqueta:<ancho_etq$}"),
            Style::new().fg(tema().label),
        ),
        Span::styled("▕", Style::new().fg(tema().muted)),
        Span::styled("█".repeat(lleno), Style::new().fg(color)),
        Span::styled(
            "░".repeat(ancho_barra - lleno),
            Style::new().fg(Color::Rgb(55, 55, 65)),
        ),
        Span::styled("▏", Style::new().fg(tema().muted)),
        Span::raw(format!(" {texto}")),
    ])
}

fn clave_valor(k: &str, v: impl Into<String>, ancho: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<ancho$}"), Style::new().fg(tema().label)),
        Span::raw(v.into()),
    ])
}

fn clave_temp(k: &str, t: f32, ancho: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<ancho$}"), Style::new().fg(tema().label)),
        Span::styled(format!("{t:.0} °C"), Style::new().fg(color_temp(t))),
    ])
}

/// Últimos `n` valores del historial (lo más reciente queda a la derecha).
fn ultimos(h: &VecDeque<u64>, n: usize) -> Vec<u64> {
    h.iter().skip(h.len().saturating_sub(n)).copied().collect()
}

fn grafica(
    f: &mut Frame,
    area: Rect,
    titulo: String,
    datos: &VecDeque<u64>,
    max: Option<u64>,
    color: Color,
) {
    let block = bloque(titulo);
    let v = ultimos(datos, block.inner(area).width as usize);
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

fn cabecera<const N: usize>(c: [String; N]) -> Row<'static> {
    Row::new(c).style(negrita(Color::Yellow))
}

// ───────────────────────────── principal ─────────────────────────────

pub fn dibujar(f: &mut Frame, app: &mut App) {
    let [arriba, cuerpo, pie] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .areas(f.area());

    dibujar_pestanas(f, app, arriba);

    let ancho_izq = (cuerpo.width * 34 / 100)
        .clamp(36, 64)
        .min(cuerpo.width / 2);
    let [izq, der] =
        Layout::horizontal([Constraint::Length(ancho_izq), Constraint::Min(30)]).areas(cuerpo);

    let lineas_sis = panel_sistema(app);
    // Sin logo, el panel Sistema ocupa toda la columna izquierda.
    let sis = if app.logo {
        let alto_sis = (lineas_sis.len() as u16 + 2).min(izq.height.saturating_sub(6));
        let [logo, sis] =
            Layout::vertical([Constraint::Min(6), Constraint::Length(alto_sis)]).areas(izq);
        dibujar_logo(f, logo);
        sis
    } else {
        izq
    };
    f.render_widget(
        Paragraph::new(lineas_sis).block(bloque(marco(i18n::t().title_system))),
        sis,
    );

    match app.pestana {
        Pestana::Resumen => tab_resumen(f, app, der),
        Pestana::Cpu => tab_cpu(f, app, der),
        Pestana::Ram => tab_ram(f, app, der),
        Pestana::Almacenamiento => tab_almacenamiento(f, app, der),
        Pestana::Gpu => tab_gpu(f, app, der),
        Pestana::Red => tab_red(f, app, der),
        Pestana::Procesos => tab_procesos(f, app, der),
    }

    dibujar_pie(f, app, pie);

    match &app.modo {
        Modo::Ayuda => dibujar_ayuda(f),
        Modo::Confirmar {
            pid,
            nombre,
            forzar,
        } => dibujar_confirmar(f, *pid, nombre, *forzar),
        _ => {}
    }
}

fn dibujar_pestanas(f: &mut Frame, app: &App, area: Rect) {
    let titulos: Vec<Line> = Pestana::TODAS
        .iter()
        .enumerate()
        .map(|(i, p)| {
            Line::from(vec![
                Span::styled(format!("{} ", i + 1), Style::new().fg(tema().muted)),
                Span::raw(p.titulo()),
            ])
        })
        .collect();
    let sel = Pestana::TODAS
        .iter()
        .position(|p| *p == app.pestana)
        .unwrap_or(0);

    let info = &app.monitor.info;
    let mut derecha = vec![Span::styled(
        format!(" {}@{} ", info.usuario, info.host),
        Style::new().fg(Color::Gray),
    )];
    if app.pausado {
        derecha.push(Span::styled(
            format!(" ⏸ {} ", i18n::t().paused),
            Style::new().fg(Color::Black).bg(Color::Yellow),
        ));
        derecha.push(Span::raw(" "));
    }
    let block = bloque(" memtop ").title(Line::from(derecha).alignment(Alignment::Right));
    let tabs = Tabs::new(titulos)
        .select(sel)
        .block(block)
        .style(Style::new().fg(Color::Gray))
        .highlight_style(
            Style::new()
                .fg(Color::Black)
                .bg(tema().accent)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::new().fg(tema().muted)));
    f.render_widget(tabs, area);
}

fn dibujar_logo(f: &mut Frame, area: Rect) {
    static LOGO: LazyLock<Sprite> = LazyLock::new(Sprite::logo);
    let block = bloque(" memtop ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    dibujar_sprite(f, &LOGO, inner);
}

/// Dibuja el sprite escalado (vecino más cercano) y centrado en `area`,
/// usando `▀`/`▄` para meter dos píxeles por celda.
fn dibujar_sprite(f: &mut Frame, sp: &Sprite, area: Rect) {
    let s = sp.scale(area.width, area.height);
    if s <= 0.0 {
        return;
    }
    let ancho = ((sp.width as f64 * s).round() as u16).min(area.width);
    let alto_px = ((sp.height as f64 * s).round() as u16).min(area.height * 2);
    let filas = alto_px.div_ceil(2);
    let x0 = area.x + (area.width - ancho) / 2;
    let y0 = area.y + (area.height - filas) / 2;
    let fuente = |t: u16, max: usize| ((t as f64 / s) as usize).min(max - 1);

    let buf = f.buffer_mut();
    for cy in 0..filas {
        for cx in 0..ancho {
            let sx = fuente(cx, sp.width);
            let arriba = sp.pixel(sx, fuente(cy * 2, sp.height));
            let abajo = if cy * 2 + 1 < alto_px {
                sp.pixel(sx, fuente(cy * 2 + 1, sp.height))
            } else {
                None
            };
            let (simbolo, estilo) = match (arriba, abajo) {
                (None, None) => continue,
                (Some(a), None) => ("▀", Style::new().fg(a)),
                (None, Some(b)) => ("▄", Style::new().fg(b)),
                (Some(a), Some(b)) => ("▀", Style::new().fg(a).bg(b)),
            };
            if let Some(celda) = buf.cell_mut((x0 + cx, y0 + cy)) {
                celda.set_symbol(simbolo).set_style(estilo);
            }
        }
    }
}

/// `"Memoria"` → `" Memoria "`: los títulos de los bloques llevan un espacio a cada lado.
fn marco(texto: &str) -> String {
    format!(" {texto} ")
}

fn estado_proceso(e: EstadoProceso) -> &'static str {
    let t = i18n::t();
    match e {
        EstadoProceso::Ejecutando => t.proc_running,
        EstadoProceso::Durmiendo => t.proc_sleeping,
        EstadoProceso::Inactivo => t.proc_idle,
        EstadoProceso::EsperaDisco => t.proc_disk_wait,
        EstadoProceso::Detenido => t.proc_stopped,
        EstadoProceso::Zombi => t.proc_zombie,
        EstadoProceso::Otro => t.proc_other,
    }
}

fn estado_bateria(e: EstadoBateria) -> &'static str {
    let t = i18n::t();
    match e {
        EstadoBateria::Cargando => t.battery_charging,
        EstadoBateria::Descargando => t.battery_discharging,
        EstadoBateria::Completa => t.battery_full,
        EstadoBateria::SinCargar => t.battery_not_charging,
        EstadoBateria::Desconocido => t.battery_unknown,
    }
}

/// Bloque estilo fastfetch.
fn panel_sistema(app: &App) -> Vec<Line<'static>> {
    let t = i18n::t();
    let m = &app.monitor;
    let i = &m.info;
    let w = 11;
    let mut l = vec![
        Line::from(vec![
            Span::styled(i.usuario.clone(), negrita(tema().accent)),
            Span::raw("@"),
            Span::styled(i.host.clone(), negrita(tema().accent)),
        ]),
        Line::styled(
            "─".repeat(i.usuario.chars().count() + i.host.chars().count() + 1),
            Style::new().fg(tema().muted),
        ),
    ];
    if let Some(e) = &i.modelo {
        l.push(clave_valor(t.machine, e.clone(), w));
    }
    l.push(clave_valor(t.os, i.so.clone(), w));
    l.push(clave_valor(t.kernel, i.kernel.clone(), w));
    l.push(clave_valor(t.uptime, fmt_duracion(m.uptime), w));
    if let Some(e) = &i.escritorio {
        l.push(clave_valor(t.desktop, e.clone(), w));
    }
    if let Some(s) = &i.shell {
        l.push(clave_valor(t.shell, s.clone(), w));
    }
    if let Some(term) = &i.terminal {
        l.push(clave_valor(t.terminal, term.clone(), w));
    }
    l.push(clave_valor(
        t.cpu,
        format!("{} ({})", i.cpu_modelo, i.hilos),
        w,
    ));
    for g in &m.gpus {
        l.push(clave_valor(t.gpu, format!("{} {}", g.vendor, g.name), w));
    }
    l.push(clave_valor(
        t.memory,
        format!(
            "{} / {}",
            fmt_bytes(m.memoria.usada()),
            fmt_bytes(m.memoria.total)
        ),
        w,
    ));
    if let Some(raiz) = m.montajes.iter().find(|x| x.punto == "/") {
        l.push(clave_valor(
            t.root_disk,
            format!(
                "{} / {} ({})",
                fmt_bytes(raiz.usado),
                fmt_bytes(raiz.total),
                raiz.fs
            ),
            w,
        ));
    }
    if let Some(b) = &m.bateria {
        l.push(clave_valor(
            t.battery,
            format!("{}% ({})", b.porcentaje, estado_bateria(b.estado)),
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

// ───────────────────────────── pestañas ─────────────────────────────

fn tab_resumen(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let m = &app.monitor;
    let activas: Vec<_> = m
        .interfaces
        .iter()
        .filter(|i| i.fisica && !i.ips.is_empty())
        .collect();
    let alto_alm = m.montajes.len().max(1) as u16 + 2;
    let alto_otros =
        (m.gpus.len() + activas.len().max(1) + usize::from(m.bateria.is_some())) as u16 + 2;
    let [cpu, mem, alm, otros, top] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(alto_alm),
        Constraint::Length(alto_otros),
        Constraint::Min(4),
    ])
    .areas(area);

    // CPU: barra + gráfica
    let block = bloque(marco(t.cpu));
    let inner = block.inner(cpu);
    f.render_widget(block, cpu);
    let [l1, graf] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let mut extra = format!("{:5.1}%", m.cpu_total);
    if let Some(grados) = m.temperatura_cpu() {
        extra.push_str(&format!("  {grados:.0}°C"));
    }
    let freq = m.cpu_freq_mhz.iter().max().copied().unwrap_or(0);
    if freq > 0 {
        extra.push_str(&format!("  {:.2} GHz", freq as f64 / 1000.0));
    }
    f.render_widget(barra(t.usage, 8, l1.width, m.cpu_total as f64, extra), l1);
    let v = ultimos(&m.hist_cpu, graf.width as usize);
    f.render_widget(
        Sparkline::default()
            .data(&v)
            .max(100)
            .style(Style::new().fg(tema().accent)),
        graf,
    );

    // Memoria
    let mm = &m.memoria;
    let w = mem.width.saturating_sub(2);
    let lineas = vec![
        barra(
            "RAM",
            8,
            w,
            pct(mm.usada(), mm.total),
            format!("{} / {}", fmt_bytes(mm.usada()), fmt_bytes(mm.total)),
        ),
        barra(
            t.swap,
            8,
            w,
            pct(mm.swap_usada(), mm.swap_total),
            format!(
                "{} / {}",
                fmt_bytes(mm.swap_usada()),
                fmt_bytes(mm.swap_total)
            ),
        ),
    ];
    f.render_widget(Paragraph::new(lineas).block(bloque(marco(t.memory))), mem);

    // Almacenamiento
    let ancho_etq = m
        .montajes
        .iter()
        .map(|x| x.punto.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 18)
        + 1;
    let lineas: Vec<Line> = m
        .montajes
        .iter()
        .map(|x| {
            barra(
                &x.punto,
                ancho_etq,
                alm.width.saturating_sub(2),
                pct(x.usado, x.total),
                format!("{} / {}", fmt_bytes(x.usado), fmt_bytes(x.total)),
            )
        })
        .collect();
    f.render_widget(
        Paragraph::new(lineas).block(bloque(marco(t.tab_storage))),
        alm,
    );

    // GPU, red, batería
    let mut lineas = Vec::new();
    for g in &m.gpus {
        let estado = if g.is_suspended() {
            Span::styled(t.gpu_idle, Style::new().fg(tema().muted))
        } else {
            Span::styled(t.gpu_active, Style::new().fg(Color::Green))
        };
        lineas.push(Line::from(vec![
            Span::styled(format!("{:<9}", t.gpu), Style::new().fg(tema().label)),
            Span::raw(format!("{} · ", g.name)),
            estado,
        ]));
    }
    if activas.is_empty() {
        lineas.push(clave_valor(t.network, t.network_disconnected, 9));
    }
    for i in &activas {
        lineas.push(Line::from(vec![
            Span::styled(format!("{:<9}", t.network), Style::new().fg(tema().label)),
            Span::raw(format!("{:<12}", i.nombre)),
            Span::styled(
                format!("↓ {:<13}", fmt_tasa(i.rx_bps)),
                Style::new().fg(Color::Green),
            ),
            Span::styled(
                format!("↑ {}", fmt_tasa(i.tx_bps)),
                Style::new().fg(Color::Magenta),
            ),
        ]));
    }
    if let Some(b) = &m.bateria {
        // En la batería, más es mejor: el color va al revés.
        let c = if b.porcentaje > 40 {
            Color::Green
        } else if b.porcentaje > 15 {
            Color::Yellow
        } else {
            Color::Red
        };
        lineas.push(barra_color(
            t.battery,
            9,
            otros.width.saturating_sub(2),
            b.porcentaje as f64,
            format!("{}% {}", b.porcentaje, estado_bateria(b.estado)),
            c,
        ));
    }
    f.render_widget(
        Paragraph::new(lineas).block(bloque(marco(t.title_gpu_network_power))),
        otros,
    );

    let lista = app.top(false, top.height.saturating_sub(3) as usize);
    tabla_mini(
        f,
        top,
        &marco(t.title_top_cpu),
        &lista,
        app.monitor.memoria.total,
    );
}

fn tabla_mini(f: &mut Frame, area: Rect, titulo: &str, lista: &[&Proceso], ram_total: u64) {
    let t = i18n::t();
    let filas = lista.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::new().fg(tema().muted)),
            Cell::from(p.nombre.clone()),
            Cell::from(format!("{:5.1}", p.cpu)).style(Style::new().fg(color_pct(p.cpu as f64))),
            Cell::from(format!("{:5.1}", pct(p.memoria, ram_total)))
                .style(Style::new().fg(Color::LightGreen)),
            Cell::from(fmt_bytes(p.memoria)),
            Cell::from(p.usuario.clone()).style(Style::new().fg(Color::Gray)),
        ])
    });
    let t = Table::new(
        filas,
        [
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(cabecera(
        [
            t.col_pid, t.col_name, t.col_cpu, t.col_mem, t.col_res, t.col_user,
        ]
        .map(String::from),
    ))
    .block(bloque(titulo.to_string()));
    f.render_widget(t, area);
}

fn tab_cpu(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let m = &app.monitor;
    let i = &m.info;
    let [info, graf, abajo] = Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(5),
    ])
    .areas(area);

    let fmin = m.cpu_freq_mhz.iter().min().copied().unwrap_or(0);
    let fmax = m.cpu_freq_mhz.iter().max().copied().unwrap_or(0);
    let w = 14;
    let fisicos = i
        .nucleos_fisicos
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());
    let mut l = vec![
        clave_valor(t.model, i.cpu_modelo.clone(), w),
        clave_valor(
            t.cores,
            format!("{fisicos} {} · {} {}", t.physical, i.hilos, t.threads),
            w,
        ),
        clave_valor(
            t.frequency,
            format!(
                "{:.2} – {:.2} GHz",
                fmin as f64 / 1000.0,
                fmax as f64 / 1000.0
            ),
            w,
        ),
        clave_valor(
            t.load_average,
            format!(
                "{:.2}  {:.2}  {:.2}   (1, 5, 15 min)",
                m.carga.0, m.carga.1, m.carga.2
            ),
            w,
        ),
        clave_valor(t.architecture, i.arquitectura.clone(), w),
    ];
    if let Some(grados) = m.temperatura_cpu() {
        l.push(clave_temp(t.temperature, grados, w));
    }
    f.render_widget(
        Paragraph::new(l).block(bloque(marco(t.title_processor))),
        info,
    );

    grafica(
        f,
        graf,
        format!(
            " {} {:.1}% · {} {HISTORIAL} s ",
            t.total_usage, m.cpu_total, t.last
        ),
        &m.hist_cpu,
        Some(100),
        tema().accent,
    );

    let [nucleos, temps] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(abajo);

    // Hilos en dos columnas.
    let block = bloque(marco(t.title_per_thread));
    let inner = block.inner(nucleos);
    f.render_widget(block, nucleos);
    let n = m.cpu_nucleos.len();
    let mitad = n.div_ceil(2);
    let [c1, c2] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner);
    for (col, rango) in [(c1, 0..mitad), (c2, mitad..n)] {
        let lineas: Vec<Line> = rango
            .map(|k| {
                let u = m.cpu_nucleos[k] as f64;
                let ghz = m.cpu_freq_mhz.get(k).copied().unwrap_or(0) as f64 / 1000.0;
                barra(
                    &format!("{k:>2}"),
                    3,
                    col.width.saturating_sub(1),
                    u,
                    format!("{u:3.0}% {ghz:.1}G"),
                )
            })
            .collect();
        f.render_widget(Paragraph::new(lineas), col);
    }

    let mut ts: Vec<&Temperatura> = m.temperaturas.iter().collect();
    ts.sort_by(|a, b| a.etiqueta.cmp(&b.etiqueta));
    let ancho = temps.width.saturating_sub(10) as usize;
    let lineas: Vec<Line> = ts
        .iter()
        .map(|sensor| {
            let etq: String = sensor.etiqueta.chars().take(ancho).collect();
            let mut s = vec![
                Span::styled(format!("{etq:<ancho$}"), Style::new().fg(Color::Gray)),
                Span::styled(
                    format!("{:>5.0}°C", sensor.grados),
                    Style::new().fg(color_temp(sensor.grados)),
                ),
            ];
            if sensor.critica.is_some_and(|c| sensor.grados >= c) {
                s.push(Span::styled(" !", Style::new().fg(Color::Red)));
            }
            Line::from(s)
        })
        .collect();
    f.render_widget(
        Paragraph::new(lineas).block(bloque(marco(t.title_sensors))),
        temps,
    );
}

fn tab_ram(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let mm = &app.monitor.memoria;
    let [barras, medio, top] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(11),
        Constraint::Min(5),
    ])
    .areas(area);

    let w = barras.width.saturating_sub(2);
    let lineas = vec![
        barra(
            t.used,
            10,
            w,
            pct(mm.usada(), mm.total),
            format!("{} / {}", fmt_bytes(mm.usada()), fmt_bytes(mm.total)),
        ),
        barra_color(
            t.cache,
            10,
            w,
            pct(mm.cache + mm.buffers, mm.total),
            fmt_bytes(mm.cache + mm.buffers),
            Color::Blue,
        ),
        barra(
            t.swap,
            10,
            w,
            pct(mm.swap_usada(), mm.swap_total),
            format!(
                "{} / {}",
                fmt_bytes(mm.swap_usada()),
                fmt_bytes(mm.swap_total)
            ),
        ),
    ];
    f.render_widget(
        Paragraph::new(lineas).block(bloque(marco(t.memory))),
        barras,
    );

    let [det, graf] =
        Layout::horizontal([Constraint::Length(30), Constraint::Min(10)]).areas(medio);
    let w = 13;
    let l = vec![
        clave_valor(t.total, fmt_bytes(mm.total), w),
        clave_valor(t.used, fmt_bytes(mm.usada()), w),
        clave_valor(t.available, fmt_bytes(mm.disponible), w),
        clave_valor(t.free, fmt_bytes(mm.libre), w),
        clave_valor(t.cache, fmt_bytes(mm.cache), w),
        clave_valor(t.buffers, fmt_bytes(mm.buffers), w),
        clave_valor(t.shared, fmt_bytes(mm.compartida), w),
        clave_valor(t.swap_total, fmt_bytes(mm.swap_total), w),
        clave_valor(t.swap_used, fmt_bytes(mm.swap_usada()), w),
    ];
    f.render_widget(Paragraph::new(l).block(bloque(marco(t.title_details))), det);
    grafica(
        f,
        graf,
        format!(
            " {} {:.0}% · {} {HISTORIAL} s ",
            t.usage,
            pct(mm.usada(), mm.total),
            t.last
        ),
        &app.monitor.hist_ram,
        Some(100),
        Color::LightGreen,
    );

    let lista = app.top(true, top.height.saturating_sub(3) as usize);
    tabla_mini(f, top, &marco(t.title_top_memory), &lista, mm.total);
}

fn tab_almacenamiento(f: &mut Frame, app: &mut App, area: Rect) {
    let tx = i18n::t();
    let m = &app.monitor;
    let alto_discos = m.discos.len() as u16 + 4;
    let [discos, parts] =
        Layout::vertical([Constraint::Length(alto_discos), Constraint::Min(4)]).areas(area);

    let filas = m.discos.iter().map(|d| {
        Row::new(vec![
            Cell::from(d.nombre.clone()).style(Style::new().fg(tema().accent)),
            Cell::from(d.modelo.clone().unwrap_or_else(|| "—".into())),
            Cell::from(d.tipo).style(Style::new().fg(Color::LightMagenta)),
            Cell::from(fmt_bytes(d.tamano)),
            match d.temperatura {
                Some(t) => Cell::from(format!("{t:.0}°C")).style(Style::new().fg(color_temp(t))),
                None => Cell::from("—").style(Style::new().fg(tema().muted)),
            },
            Cell::from(fmt_tasa(d.lectura_bps)).style(Style::new().fg(Color::Green)),
            Cell::from(fmt_tasa(d.escritura_bps)).style(Style::new().fg(Color::Magenta)),
        ])
    });
    let t = Table::new(
        filas,
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
        cabecera(
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
    .block(bloque(marco(tx.title_disks)));
    f.render_widget(t, discos);

    let block = bloque(marco(tx.title_partitions));
    let inner = block.inner(parts);
    f.render_widget(block, parts);
    let mut lineas = Vec::new();
    for x in &m.montajes {
        let p = pct(x.usado, x.total);
        lineas.push(Line::from(vec![
            Span::styled(x.punto.clone(), negrita(tema().accent)),
            Span::styled(
                format!("  {} · {}", x.dispositivo, x.fs),
                Style::new().fg(Color::Gray),
            ),
        ]));
        lineas.push(barra(
            "",
            0,
            inner.width,
            p,
            format!(
                "{p:.0}%  {} {} · {} {} {}",
                fmt_bytes(x.usado),
                tx.used_space,
                fmt_bytes(x.total - x.usado),
                tx.free_of,
                fmt_bytes(x.total)
            ),
        ));
        lineas.push(Line::raw(""));
    }
    f.render_widget(Paragraph::new(lineas), inner);
}

fn tab_gpu(f: &mut Frame, app: &mut App, area: Rect) {
    let tx = i18n::t();
    let gpus = &app.monitor.gpus;
    if gpus.is_empty() {
        f.render_widget(
            Paragraph::new(tx.gpu_none).block(bloque(marco(tx.gpu))),
            area,
        );
        return;
    }
    let areas = Layout::vertical(gpus.iter().map(|_| Constraint::Fill(1))).split(area);
    for (g, a) in gpus.iter().zip(areas.iter()) {
        let w = 13;
        let ancho = a.width.saturating_sub(2);
        let estado = match g.power_state.as_deref() {
            Some("suspended") => Span::styled(tx.gpu_state_idle, Style::new().fg(tema().muted)),
            Some("active") => Span::styled(tx.gpu_state_active, Style::new().fg(Color::Green)),
            Some(o) => Span::raw(o.to_string()),
            None => Span::raw("—"),
        };
        let mut l = vec![
            clave_valor(tx.vendor, g.vendor.clone(), w),
            clave_valor(tx.driver, g.driver.clone().unwrap_or_else(|| "—".into()), w),
            clave_valor(tx.pci, g.address.clone(), w),
            Line::from(vec![
                Span::styled(format!("{:<w$}", tx.state), Style::new().fg(tema().label)),
                estado,
            ]),
        ];
        if let Some(u) = g.usage {
            l.push(barra(tx.usage, w, ancho, u as f64, format!("{u:.0}%")));
        }
        if let (Some(u), Some(t)) = (g.vram_used, g.vram_total) {
            l.push(barra(
                tx.vram,
                w,
                ancho,
                pct(u, t),
                format!("{} / {}", fmt_bytes(u), fmt_bytes(t)),
            ));
        }
        if let Some(fr) = g.clock_mhz {
            let max = g
                .max_clock_mhz
                .map(|m| format!(" / {m} MHz"))
                .unwrap_or_default();
            l.push(clave_valor(tx.frequency, format!("{fr} MHz{max}"), w));
        }
        if let Some(t) = g.temperature {
            l.push(clave_temp(tx.temperature, t, w));
        }
        if let Some(p) = g.power_w {
            l.push(clave_valor(tx.power, format!("{p:.1} W"), w));
        }
        if g.vendor.contains("NVIDIA") && g.usage.is_none() && !g.is_suspended() {
            l.push(Line::styled(tx.gpu_querying, Style::new().fg(tema().muted)));
        }
        if g.is_suspended() {
            l.push(Line::styled(
                tx.gpu_not_waking,
                Style::new().fg(tema().muted).add_modifier(Modifier::ITALIC),
            ));
        }
        f.render_widget(Paragraph::new(l).block(bloque(format!(" {} ", g.name))), *a);
    }
}

fn tab_red(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let m = &app.monitor;
    let [graf, tabla] = Layout::vertical([Constraint::Length(9), Constraint::Min(4)]).areas(area);
    let [rx, tx] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(graf);
    let rx_ahora: f64 = m.interfaces.iter().map(|i| i.rx_bps).sum();
    let tx_ahora: f64 = m.interfaces.iter().map(|i| i.tx_bps).sum();
    grafica(
        f,
        rx,
        format!(" ↓ {} {} ", t.download, fmt_tasa(rx_ahora)),
        &m.hist_rx,
        None,
        Color::Green,
    );
    grafica(
        f,
        tx,
        format!(" ↑ {} {} ", t.upload, fmt_tasa(tx_ahora)),
        &m.hist_tx,
        None,
        Color::Magenta,
    );

    let filas = m.interfaces.iter().map(|i| {
        let activa = !i.ips.is_empty();
        Row::new(vec![
            Cell::from(i.nombre.clone()).style(Style::new().fg(if activa {
                tema().accent
            } else {
                tema().muted
            })),
            Cell::from(resumen_ips(&i.ips)),
            Cell::from(i.mac.clone()).style(Style::new().fg(Color::Gray)),
            Cell::from(fmt_tasa(i.rx_bps)).style(Style::new().fg(Color::Green)),
            Cell::from(fmt_tasa(i.tx_bps)).style(Style::new().fg(Color::Magenta)),
            Cell::from(fmt_bytes(i.rx_total)),
            Cell::from(fmt_bytes(i.tx_total)),
        ])
    });
    let t = Table::new(
        filas,
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
        cabecera([
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
    .block(bloque(marco(t.title_interfaces)));
    f.render_widget(t, tabla);
}

/// IPv4 completas y el resto resumido: "192.0.2.10/24 +3 v6".
fn resumen_ips(ips: &[String]) -> String {
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

/// Separa la línea de comandos en (ruta, programa, argumentos) para colorearla.
fn partes_comando(p: &Proceso) -> (String, String, String) {
    if p.comando.is_empty() {
        return (String::new(), format!("[{}]", p.nombre), String::new());
    }
    let (prog, args) = p.comando.split_once(' ').unwrap_or((&p.comando, ""));
    let (dir, base) = match prog.rsplit_once('/') {
        Some((d, b)) => (format!("{d}/"), b.to_string()),
        None => (String::new(), prog.to_string()),
    };
    // Si el programa no se parece al nombre del proceso (p. ej. "RDD Process"), mostrar el nombre primero.
    if !base.starts_with(&p.nombre) && !p.nombre.starts_with(&base) {
        return (String::new(), p.nombre.clone(), format!("  {}", p.comando));
    }
    let args = if args.is_empty() {
        String::new()
    } else {
        format!(" {args}")
    };
    (dir, base, args)
}

fn tab_procesos(f: &mut Frame, app: &mut App, area: Rect) {
    let t = i18n::t();
    let total_ram = app.monitor.memoria.total;
    let flecha = |o: OrdenProc| {
        if app.orden_proc == o {
            if app.desc { " ↓" } else { " ↑" }
        } else {
            ""
        }
    };
    let cab = cabecera([
        format!("{}{}", t.col_pid, flecha(OrdenProc::Pid)),
        format!("{}{}", t.col_user, flecha(OrdenProc::Usuario)),
        format!("{}{}", t.col_cpu, flecha(OrdenProc::Cpu)),
        format!("{}{}", t.col_mem, flecha(OrdenProc::Memoria)),
        t.col_res.into(),
        t.state.into(),
        t.col_cpu_time.into(),
        format!("{}{}", t.col_command, flecha(OrdenProc::Nombre)),
    ]);

    let lista = app.procesos();
    let n = lista.len();
    let filas: Vec<Row> = lista
        .iter()
        .map(|p| {
            let mem_pct = pct(p.memoria, total_ram);
            let (dir, prog, args) = partes_comando(p);
            Row::new(vec![
                Cell::from(p.pid.to_string()).style(Style::new().fg(tema().muted)),
                Cell::from(p.usuario.clone()).style(Style::new().fg(Color::Gray)),
                Cell::from(format!("{:5.1}", p.cpu))
                    .style(Style::new().fg(color_pct(p.cpu as f64))),
                Cell::from(format!("{mem_pct:5.1}"))
                    .style(Style::new().fg(color_pct(mem_pct * 3.0))),
                Cell::from(fmt_bytes(p.memoria)),
                Cell::from(estado_proceso(p.estado)).style(Style::new().fg(Color::Gray)),
                Cell::from(fmt_tiempo(p.tiempo)).style(Style::new().fg(Color::Gray)),
                Cell::from(Line::from(vec![
                    Span::styled(dir, Style::new().fg(tema().muted)),
                    Span::styled(prog, Style::new().add_modifier(Modifier::BOLD)),
                    Span::styled(args, Style::new().fg(Color::Gray)),
                ])),
            ])
        })
        .collect();

    let mut titulo = format!(" {} ({n}) ", t.tab_processes);
    if app.solo_mios {
        titulo.push_str(&format!("· {} ", t.mine_only));
    }
    let mut block = bloque(titulo);
    if app.modo == Modo::Buscando || !app.busqueda.is_empty() {
        let cursor = if app.modo == Modo::Buscando {
            "▏"
        } else {
            ""
        };
        block = block.title(
            Line::from(vec![
                Span::styled(" / ", negrita(Color::Yellow)),
                Span::styled(
                    format!("{}{cursor} ", app.busqueda),
                    Style::new().fg(Color::Yellow),
                ),
            ])
            .alignment(Alignment::Right),
        );
    }

    let t = Table::new(
        filas,
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
    .header(cab)
    .row_highlight_style(
        Style::new()
            .bg(tema().selection)
            .add_modifier(Modifier::BOLD),
    )
    .block(block);
    f.render_stateful_widget(t, area, &mut app.tabla_proc);

    if n > area.height.saturating_sub(3) as usize {
        let mut sb = ScrollbarState::new(n).position(app.tabla_proc.selected().unwrap_or(0));
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(tema().muted)),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut sb,
        );
    }
}

fn fmt_tiempo(s: u64) -> String {
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h >= 100 {
        format!("{}d{}h", h / 24, h % 24)
    } else {
        format!("{h}:{m:02}:{s:02}")
    }
}

// ───────────────────────────── pie y ventanas ─────────────────────────────

fn dibujar_pie(f: &mut Frame, app: &App, area: Rect) {
    if let Some((txt, error, _)) = &app.mensaje {
        let c = if *error { Color::Red } else { Color::Green };
        f.render_widget(
            Paragraph::new(Line::styled(format!(" {txt}"), Style::new().fg(c))),
            area,
        );
        return;
    }
    let t = i18n::t();
    let pares: &[(&str, &str)] = match (&app.modo, app.pestana) {
        (Modo::Buscando, _) => &[
            ("Enter", t.footer_accept),
            ("Esc", t.footer_clear),
            ("↑↓", t.footer_move),
        ],
        (_, Pestana::Procesos) => &[
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
    for (k, d) in pares {
        spans.push(Span::styled(
            *k,
            Style::new().fg(Color::Black).bg(tema().accent),
        ));
        spans.push(Span::styled(
            format!(" {d}  "),
            Style::new().fg(Color::Gray),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn centrar(area: Rect, w: u16, h: u16) -> Rect {
    let (w, h) = (w.min(area.width), h.min(area.height));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn dibujar_confirmar(f: &mut Frame, pid: u32, nombre: &str, forzar: bool) {
    let t = i18n::t();
    let r = centrar(f.area(), 58, 7);
    f.render_widget(Clear, r);
    let senal = if forzar {
        format!("SIGKILL {}", t.confirm_force)
    } else {
        "SIGTERM".to_string()
    };
    let l = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw(format!(" {} ", t.confirm_send)),
            Span::styled(senal, negrita(Color::Red)),
            Span::raw(format!(" {} ", t.confirm_to)),
            Span::styled(
                format!("{nombre} ({pid})"),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::raw(""),
        Line::styled(
            format!(" {}", t.confirm_options),
            Style::new().fg(tema().muted),
        ),
    ];
    let block = bloque(marco(t.confirm_title)).border_style(Style::new().fg(Color::Red));
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }).block(block), r);
}

fn dibujar_ayuda(f: &mut Frame) {
    let t = i18n::t();
    let secciones: [(&str, &[(&str, &str)]); 2] = [
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
    for (titulo, filas) in secciones {
        l.push(Line::styled(format!(" {titulo}"), negrita(Color::Yellow)));
        for (k, d) in filas {
            l.push(Line::from(vec![
                Span::styled(format!("   {k:<18}"), Style::new().fg(tema().accent)),
                Span::raw(*d),
            ]));
        }
        l.push(Line::raw(""));
    }
    let r = centrar(f.area(), 72, l.len() as u16 + 2);
    f.render_widget(Clear, r);
    f.render_widget(Paragraph::new(l).block(bloque(marco(t.help_title))), r);
}
