//! Dibujo de la interfaz con ratatui.
//!
//! La pantalla se divide en: barra de pestañas (arriba), panel de imagen y resumen del
//! sistema (izquierda), contenido de la pestaña activa (derecha) y atajos de teclado (abajo).
//! Todas las funciones son de solo lectura sobre [`App`], salvo el estado de selección
//! de las tablas y el protocolo de imagen, que ratatui necesita mutables.

use std::collections::VecDeque;

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
use ratatui_image::{Resize, StatefulImage};

use crate::{
    app::{App, Modo, OrdenProc, Pestana},
    foto::Contenido,
    sistema::{HISTORIAL, Proceso, Temperatura, fmt_bytes, fmt_duracion, fmt_tasa},
};

const ACENTO: Color = Color::Cyan;
const TENUE: Color = Color::DarkGray;
const SEL_BG: Color = Color::Rgb(35, 55, 85);
const ETIQUETA: Color = Color::LightBlue;

// ───────────────────────────── utilidades ─────────────────────────────

fn bloque(titulo: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(TENUE))
        .title(Span::styled(
            titulo.into(),
            Style::new().fg(ACENTO).add_modifier(Modifier::BOLD),
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
        Span::styled(format!("{etiqueta:<ancho_etq$}"), Style::new().fg(ETIQUETA)),
        Span::styled("▕", Style::new().fg(TENUE)),
        Span::styled("█".repeat(lleno), Style::new().fg(color)),
        Span::styled(
            "░".repeat(ancho_barra - lleno),
            Style::new().fg(Color::Rgb(55, 55, 65)),
        ),
        Span::styled("▏", Style::new().fg(TENUE)),
        Span::raw(format!(" {texto}")),
    ])
}

fn clave_valor(k: &str, v: impl Into<String>, ancho: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<ancho$}"), Style::new().fg(ETIQUETA)),
        Span::raw(v.into()),
    ])
}

fn clave_temp(k: &str, t: f32, ancho: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<ancho$}"), Style::new().fg(ETIQUETA)),
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
    let alto_sis = (lineas_sis.len() as u16 + 2).min(izq.height.saturating_sub(6));
    let [foto, sis] =
        Layout::vertical([Constraint::Min(6), Constraint::Length(alto_sis)]).areas(izq);
    dibujar_foto(f, app, foto);
    f.render_widget(Paragraph::new(lineas_sis).block(bloque(" Sistema ")), sis);

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
                Span::styled(format!("{} ", i + 1), Style::new().fg(TENUE)),
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
            " ⏸ PAUSA ",
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
                .bg(ACENTO)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::new().fg(TENUE)));
    f.render_widget(tabs, area);
}

fn texto_centrado(f: &mut Frame, inner: Rect, texto: String, color: Color) {
    let alto = texto.lines().count() as u16 + 1;
    let y = inner.y + inner.height.saturating_sub(alto) / 2;
    f.render_widget(
        Paragraph::new(texto)
            .alignment(Alignment::Center)
            .style(Style::new().fg(color))
            .wrap(Wrap { trim: true }),
        Rect {
            y,
            height: inner.height - (y - inner.y),
            ..inner
        },
    );
}

fn dibujar_foto(f: &mut Frame, app: &mut App, area: Rect) {
    let pie = if app.fotos.cantidad() > 0 {
        " f: otra ".to_string()
    } else {
        format!(" tus fotos: {} ", crate::foto::carpeta_mostrada())
    };
    let block = bloque(app.fotos.titulo.clone())
        .title_bottom(Line::styled(pie, Style::new().fg(TENUE)).alignment(Alignment::Right));
    let inner = block.inner(area);
    f.render_widget(block, area);

    match &mut app.fotos.actual {
        Contenido::Error(e) => texto_centrado(f, inner, e.clone(), Color::Red),
        Contenido::Pixeles(sp) => dibujar_sprite(f, sp, inner),
        Contenido::Imagen(proto) => {
            let resize = Resize::Scale(None);
            let tam = proto.size_for(resize.clone(), inner.as_size());
            let r = Rect {
                x: inner.x + inner.width.saturating_sub(tam.width) / 2,
                y: inner.y + inner.height.saturating_sub(tam.height) / 2,
                width: tam.width.min(inner.width),
                height: tam.height.min(inner.height),
            };
            f.render_stateful_widget(StatefulImage::default().resize(resize), r, proto.as_mut());
            let _ = proto.last_encoding_result();
        }
    }
}

/// Dibuja el sprite escalado (vecino más cercano) y centrado en `area`,
/// usando `▀`/`▄` para meter dos píxeles por celda.
fn dibujar_sprite(f: &mut Frame, sp: &crate::foto::Sprite, area: Rect) {
    let s = sp.escala(area.width, area.height);
    if s <= 0.0 {
        return;
    }
    let ancho = ((sp.ancho as f64 * s).round() as u16).min(area.width);
    let alto_px = ((sp.alto as f64 * s).round() as u16).min(area.height * 2);
    let filas = alto_px.div_ceil(2);
    let x0 = area.x + (area.width - ancho) / 2;
    let y0 = area.y + (area.height - filas) / 2;
    let fuente = |t: u16, max: usize| ((t as f64 / s) as usize).min(max - 1);

    let buf = f.buffer_mut();
    for cy in 0..filas {
        for cx in 0..ancho {
            let sx = fuente(cx, sp.ancho);
            let arriba = sp.pixel(sx, fuente(cy * 2, sp.alto));
            let abajo = if cy * 2 + 1 < alto_px {
                sp.pixel(sx, fuente(cy * 2 + 1, sp.alto))
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

/// Bloque estilo fastfetch.
fn panel_sistema(app: &App) -> Vec<Line<'static>> {
    let m = &app.monitor;
    let i = &m.info;
    let w = 11;
    let mut l = vec![
        Line::from(vec![
            Span::styled(i.usuario.clone(), negrita(ACENTO)),
            Span::raw("@"),
            Span::styled(i.host.clone(), negrita(ACENTO)),
        ]),
        Line::styled(
            "─".repeat(i.usuario.chars().count() + i.host.chars().count() + 1),
            Style::new().fg(TENUE),
        ),
    ];
    if let Some(e) = &i.modelo {
        l.push(clave_valor("Equipo", e.clone(), w));
    }
    l.push(clave_valor("SO", i.so.clone(), w));
    l.push(clave_valor("Kernel", i.kernel.clone(), w));
    l.push(clave_valor("Encendido", fmt_duracion(m.uptime), w));
    if let Some(e) = &i.escritorio {
        l.push(clave_valor("Escritorio", e.clone(), w));
    }
    if let Some(s) = &i.shell {
        l.push(clave_valor("Shell", s.clone(), w));
    }
    if let Some(t) = &i.terminal {
        l.push(clave_valor("Terminal", t.clone(), w));
    }
    l.push(clave_valor(
        "CPU",
        format!("{} ({})", i.cpu_modelo, i.hilos),
        w,
    ));
    for g in &m.gpus {
        l.push(clave_valor(
            "GPU",
            format!("{} {}", g.fabricante, g.nombre),
            w,
        ));
    }
    l.push(clave_valor(
        "Memoria",
        format!(
            "{} / {}",
            fmt_bytes(m.memoria.usada()),
            fmt_bytes(m.memoria.total)
        ),
        w,
    ));
    if let Some(raiz) = m.montajes.iter().find(|x| x.punto == "/") {
        l.push(clave_valor(
            "Disco (/)",
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
            "Batería",
            format!("{}% ({})", b.porcentaje, b.estado),
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
    let block = bloque(" CPU ");
    let inner = block.inner(cpu);
    f.render_widget(block, cpu);
    let [l1, graf] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let mut extra = format!("{:5.1}%", m.cpu_total);
    if let Some(t) = m.temperatura_cpu() {
        extra.push_str(&format!("  {t:.0}°C"));
    }
    let freq = m.cpu_freq_mhz.iter().max().copied().unwrap_or(0);
    if freq > 0 {
        extra.push_str(&format!("  {:.2} GHz", freq as f64 / 1000.0));
    }
    f.render_widget(barra("Uso", 8, l1.width, m.cpu_total as f64, extra), l1);
    let v = ultimos(&m.hist_cpu, graf.width as usize);
    f.render_widget(
        Sparkline::default()
            .data(&v)
            .max(100)
            .style(Style::new().fg(ACENTO)),
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
            "Swap",
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
    f.render_widget(Paragraph::new(lineas).block(bloque(" Memoria ")), mem);

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
        Paragraph::new(lineas).block(bloque(" Almacenamiento ")),
        alm,
    );

    // GPU, red, batería
    let mut lineas = Vec::new();
    for g in &m.gpus {
        let estado = if g.suspendida() {
            Span::styled("en reposo", Style::new().fg(TENUE))
        } else {
            Span::styled("activa", Style::new().fg(Color::Green))
        };
        lineas.push(Line::from(vec![
            Span::styled(format!("{:<9}", "GPU"), Style::new().fg(ETIQUETA)),
            Span::raw(format!("{} · ", g.nombre)),
            estado,
        ]));
    }
    if activas.is_empty() {
        lineas.push(clave_valor("Red", "sin conexión", 9));
    }
    for i in &activas {
        lineas.push(Line::from(vec![
            Span::styled(format!("{:<9}", "Red"), Style::new().fg(ETIQUETA)),
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
            "Batería",
            9,
            otros.width.saturating_sub(2),
            b.porcentaje as f64,
            format!("{}% {}", b.porcentaje, b.estado),
            c,
        ));
    }
    f.render_widget(
        Paragraph::new(lineas).block(bloque(" GPU · Red · Energía ")),
        otros,
    );

    let lista = app.top(false, top.height.saturating_sub(3) as usize);
    tabla_mini(
        f,
        top,
        " Procesos con más CPU ",
        &lista,
        app.monitor.memoria.total,
    );
}

fn tabla_mini(f: &mut Frame, area: Rect, titulo: &str, lista: &[&Proceso], ram_total: u64) {
    let filas = lista.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::new().fg(TENUE)),
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
        ["PID", "Nombre", "CPU%", "MEM%", "RES", "Usuario"].map(String::from),
    ))
    .block(bloque(titulo.to_string()));
    f.render_widget(t, area);
}

fn tab_cpu(f: &mut Frame, app: &mut App, area: Rect) {
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
        clave_valor("Modelo", i.cpu_modelo.clone(), w),
        clave_valor(
            "Núcleos",
            format!("{fisicos} físicos · {} hilos", i.hilos),
            w,
        ),
        clave_valor(
            "Frecuencia",
            format!(
                "{:.2} – {:.2} GHz",
                fmin as f64 / 1000.0,
                fmax as f64 / 1000.0
            ),
            w,
        ),
        clave_valor(
            "Carga media",
            format!(
                "{:.2}  {:.2}  {:.2}   (1, 5, 15 min)",
                m.carga.0, m.carga.1, m.carga.2
            ),
            w,
        ),
        clave_valor("Arquitectura", i.arquitectura.clone(), w),
    ];
    if let Some(t) = m.temperatura_cpu() {
        l.push(clave_temp("Temperatura", t, w));
    }
    f.render_widget(Paragraph::new(l).block(bloque(" Procesador ")), info);

    grafica(
        f,
        graf,
        format!(" Uso total {:.1}% · últimos {HISTORIAL} s ", m.cpu_total),
        &m.hist_cpu,
        Some(100),
        ACENTO,
    );

    let [nucleos, temps] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(abajo);

    // Hilos en dos columnas.
    let block = bloque(" Por hilo ");
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
        .map(|t| {
            let etq: String = t.etiqueta.chars().take(ancho).collect();
            let mut s = vec![
                Span::styled(format!("{etq:<ancho$}"), Style::new().fg(Color::Gray)),
                Span::styled(
                    format!("{:>5.0}°C", t.grados),
                    Style::new().fg(color_temp(t.grados)),
                ),
            ];
            if t.critica.is_some_and(|c| t.grados >= c) {
                s.push(Span::styled(" !", Style::new().fg(Color::Red)));
            }
            Line::from(s)
        })
        .collect();
    f.render_widget(Paragraph::new(lineas).block(bloque(" Sensores ")), temps);
}

fn tab_ram(f: &mut Frame, app: &mut App, area: Rect) {
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
            "Usada",
            10,
            w,
            pct(mm.usada(), mm.total),
            format!("{} / {}", fmt_bytes(mm.usada()), fmt_bytes(mm.total)),
        ),
        barra_color(
            "Caché",
            10,
            w,
            pct(mm.cache + mm.buffers, mm.total),
            fmt_bytes(mm.cache + mm.buffers),
            Color::Blue,
        ),
        barra(
            "Swap",
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
    f.render_widget(Paragraph::new(lineas).block(bloque(" Memoria ")), barras);

    let [det, graf] =
        Layout::horizontal([Constraint::Length(30), Constraint::Min(10)]).areas(medio);
    let w = 13;
    let l = vec![
        clave_valor("Total", fmt_bytes(mm.total), w),
        clave_valor("Usada", fmt_bytes(mm.usada()), w),
        clave_valor("Disponible", fmt_bytes(mm.disponible), w),
        clave_valor("Libre", fmt_bytes(mm.libre), w),
        clave_valor("Caché", fmt_bytes(mm.cache), w),
        clave_valor("Buffers", fmt_bytes(mm.buffers), w),
        clave_valor("Compartida", fmt_bytes(mm.compartida), w),
        clave_valor("Swap total", fmt_bytes(mm.swap_total), w),
        clave_valor("Swap usada", fmt_bytes(mm.swap_usada()), w),
    ];
    f.render_widget(Paragraph::new(l).block(bloque(" Detalle ")), det);
    grafica(
        f,
        graf,
        format!(
            " Uso {:.0}% · últimos {HISTORIAL} s ",
            pct(mm.usada(), mm.total)
        ),
        &app.monitor.hist_ram,
        Some(100),
        Color::LightGreen,
    );

    let lista = app.top(true, top.height.saturating_sub(3) as usize);
    tabla_mini(f, top, " Procesos con más memoria ", &lista, mm.total);
}

fn tab_almacenamiento(f: &mut Frame, app: &mut App, area: Rect) {
    let m = &app.monitor;
    let alto_discos = m.discos.len() as u16 + 4;
    let [discos, parts] =
        Layout::vertical([Constraint::Length(alto_discos), Constraint::Min(4)]).areas(area);

    let filas = m.discos.iter().map(|d| {
        Row::new(vec![
            Cell::from(d.nombre.clone()).style(Style::new().fg(ACENTO)),
            Cell::from(d.modelo.clone().unwrap_or_else(|| "—".into())),
            Cell::from(d.tipo).style(Style::new().fg(Color::LightMagenta)),
            Cell::from(fmt_bytes(d.tamano)),
            match d.temperatura {
                Some(t) => Cell::from(format!("{t:.0}°C")).style(Style::new().fg(color_temp(t))),
                None => Cell::from("—").style(Style::new().fg(TENUE)),
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
                "Disco",
                "Modelo",
                "Tipo",
                "Tamaño",
                "Temp",
                "Lectura",
                "Escritura",
            ]
            .map(String::from),
        )
        .bottom_margin(1),
    )
    .block(bloque(" Discos físicos "));
    f.render_widget(t, discos);

    let block = bloque(" Particiones montadas ");
    let inner = block.inner(parts);
    f.render_widget(block, parts);
    let mut lineas = Vec::new();
    for x in &m.montajes {
        let p = pct(x.usado, x.total);
        lineas.push(Line::from(vec![
            Span::styled(x.punto.clone(), negrita(ACENTO)),
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
                "{p:.0}%  {} usados · {} libres de {}",
                fmt_bytes(x.usado),
                fmt_bytes(x.total - x.usado),
                fmt_bytes(x.total)
            ),
        ));
        lineas.push(Line::raw(""));
    }
    f.render_widget(Paragraph::new(lineas), inner);
}

fn tab_gpu(f: &mut Frame, app: &mut App, area: Rect) {
    let gpus = &app.monitor.gpus;
    if gpus.is_empty() {
        f.render_widget(
            Paragraph::new("No se encontró ninguna GPU").block(bloque(" GPU ")),
            area,
        );
        return;
    }
    let areas = Layout::vertical(gpus.iter().map(|_| Constraint::Fill(1))).split(area);
    for (g, a) in gpus.iter().zip(areas.iter()) {
        let w = 13;
        let ancho = a.width.saturating_sub(2);
        let estado = match g.estado.as_deref() {
            Some("suspended") => {
                Span::styled("En reposo (ahorrando energía)", Style::new().fg(TENUE))
            }
            Some("active") => Span::styled("Activa", Style::new().fg(Color::Green)),
            Some(o) => Span::raw(o.to_string()),
            None => Span::raw("—"),
        };
        let mut l = vec![
            clave_valor("Fabricante", g.fabricante.clone(), w),
            clave_valor("Driver", g.driver.clone().unwrap_or_else(|| "—".into()), w),
            clave_valor("PCI", g.direccion.clone(), w),
            Line::from(vec![
                Span::styled(format!("{:<w$}", "Estado"), Style::new().fg(ETIQUETA)),
                estado,
            ]),
        ];
        if let Some(u) = g.uso {
            l.push(barra("Uso", w, ancho, u as f64, format!("{u:.0}%")));
        }
        if let (Some(u), Some(t)) = (g.vram_usada, g.vram_total) {
            l.push(barra(
                "VRAM",
                w,
                ancho,
                pct(u, t),
                format!("{} / {}", fmt_bytes(u), fmt_bytes(t)),
            ));
        }
        if let Some(fr) = g.frecuencia_mhz {
            let max = g
                .frecuencia_max_mhz
                .map(|m| format!(" / {m} MHz"))
                .unwrap_or_default();
            l.push(clave_valor("Frecuencia", format!("{fr} MHz{max}"), w));
        }
        if let Some(t) = g.temperatura {
            l.push(clave_temp("Temperatura", t, w));
        }
        if let Some(p) = g.consumo_w {
            l.push(clave_valor("Consumo", format!("{p:.1} W"), w));
        }
        if g.fabricante.contains("NVIDIA") && g.uso.is_none() && !g.suspendida() {
            l.push(Line::styled(
                "Consultando nvidia-smi…",
                Style::new().fg(TENUE),
            ));
        }
        if g.suspendida() {
            l.push(Line::styled(
                "No se le consulta nada para no despertarla.",
                Style::new().fg(TENUE).add_modifier(Modifier::ITALIC),
            ));
        }
        f.render_widget(
            Paragraph::new(l).block(bloque(format!(" {} ", g.nombre))),
            *a,
        );
    }
}

fn tab_red(f: &mut Frame, app: &mut App, area: Rect) {
    let m = &app.monitor;
    let [graf, tabla] = Layout::vertical([Constraint::Length(9), Constraint::Min(4)]).areas(area);
    let [rx, tx] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(graf);
    let rx_ahora: f64 = m.interfaces.iter().map(|i| i.rx_bps).sum();
    let tx_ahora: f64 = m.interfaces.iter().map(|i| i.tx_bps).sum();
    grafica(
        f,
        rx,
        format!(" ↓ Descarga {} ", fmt_tasa(rx_ahora)),
        &m.hist_rx,
        None,
        Color::Green,
    );
    grafica(
        f,
        tx,
        format!(" ↑ Subida {} ", fmt_tasa(tx_ahora)),
        &m.hist_tx,
        None,
        Color::Magenta,
    );

    let filas = m.interfaces.iter().map(|i| {
        let activa = !i.ips.is_empty();
        Row::new(vec![
            Cell::from(i.nombre.clone()).style(Style::new().fg(if activa {
                ACENTO
            } else {
                TENUE
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
        cabecera(
            [
                "Interfaz",
                "IP",
                "MAC",
                "↓ ahora",
                "↑ ahora",
                "↓ total",
                "↑ total",
            ]
            .map(String::from),
        )
        .bottom_margin(1),
    )
    .block(bloque(" Interfaces "));
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
    let total_ram = app.monitor.memoria.total;
    let flecha = |o: OrdenProc| {
        if app.orden_proc == o {
            if app.desc { " ↓" } else { " ↑" }
        } else {
            ""
        }
    };
    let cab = cabecera([
        format!("PID{}", flecha(OrdenProc::Pid)),
        format!("Usuario{}", flecha(OrdenProc::Usuario)),
        format!("CPU%{}", flecha(OrdenProc::Cpu)),
        format!("MEM%{}", flecha(OrdenProc::Memoria)),
        "RES".into(),
        "Estado".into(),
        "T. CPU".into(),
        format!("Comando{}", flecha(OrdenProc::Nombre)),
    ]);

    let lista = app.procesos();
    let n = lista.len();
    let filas: Vec<Row> = lista
        .iter()
        .map(|p| {
            let mem_pct = pct(p.memoria, total_ram);
            let (dir, prog, args) = partes_comando(p);
            Row::new(vec![
                Cell::from(p.pid.to_string()).style(Style::new().fg(TENUE)),
                Cell::from(p.usuario.clone()).style(Style::new().fg(Color::Gray)),
                Cell::from(format!("{:5.1}", p.cpu))
                    .style(Style::new().fg(color_pct(p.cpu as f64))),
                Cell::from(format!("{mem_pct:5.1}"))
                    .style(Style::new().fg(color_pct(mem_pct * 3.0))),
                Cell::from(fmt_bytes(p.memoria)),
                Cell::from(p.estado.clone()).style(Style::new().fg(Color::Gray)),
                Cell::from(fmt_tiempo(p.tiempo)).style(Style::new().fg(Color::Gray)),
                Cell::from(Line::from(vec![
                    Span::styled(dir, Style::new().fg(TENUE)),
                    Span::styled(prog, Style::new().add_modifier(Modifier::BOLD)),
                    Span::styled(args, Style::new().fg(Color::Gray)),
                ])),
            ])
        })
        .collect();

    let mut titulo = format!(" Procesos ({n}) ");
    if app.solo_mios {
        titulo.push_str("· solo míos ");
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
    .row_highlight_style(Style::new().bg(SEL_BG).add_modifier(Modifier::BOLD))
    .block(block);
    f.render_stateful_widget(t, area, &mut app.tabla_proc);

    if n > area.height.saturating_sub(3) as usize {
        let mut sb = ScrollbarState::new(n).position(app.tabla_proc.selected().unwrap_or(0));
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().fg(TENUE)),
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
    let pares: &[(&str, &str)] = match (&app.modo, app.pestana) {
        (Modo::Buscando, _) => &[("Enter", "aceptar"), ("Esc", "borrar"), ("↑↓", "mover")],
        (_, Pestana::Procesos) => &[
            ("↑↓", "mover"),
            ("/", "buscar"),
            ("c m p n u", "ordenar"),
            ("o", "solo míos"),
            ("t", "terminar"),
            ("K", "matar"),
            ("←→", "pestaña"),
            ("?", "ayuda"),
            ("q", "salir"),
        ],
        _ => &[
            ("←→/Tab", "pestaña"),
            ("1-7", "ir a"),
            ("f", "otra foto"),
            ("espacio", "pausa"),
            ("?", "ayuda"),
            ("q", "salir"),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    for (k, d) in pares {
        spans.push(Span::styled(*k, Style::new().fg(Color::Black).bg(ACENTO)));
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
    let r = centrar(f.area(), 58, 7);
    f.render_widget(Clear, r);
    let senal = if forzar {
        "SIGKILL (forzar)"
    } else {
        "SIGTERM"
    };
    let l = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw(" ¿Enviar "),
            Span::styled(senal, negrita(Color::Red)),
            Span::raw(" a "),
            Span::styled(
                format!("{nombre} ({pid})"),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::raw(""),
        Line::styled(
            " s / Enter: sí      cualquier otra tecla: no",
            Style::new().fg(TENUE),
        ),
    ];
    let block = bloque(" Terminar proceso ").border_style(Style::new().fg(Color::Red));
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }).block(block), r);
}

fn dibujar_ayuda(f: &mut Frame) {
    let secciones: [(&str, &[(&str, &str)]); 2] = [
        (
            "General",
            &[
                ("← → / Tab / h l", "Cambiar de pestaña"),
                ("1 … 7", "Ir a una pestaña"),
                ("f / F", "Foto siguiente / anterior (o nuevo Pokémon)"),
                ("espacio", "Pausar / reanudar la actualización"),
                ("q / Esc", "Salir"),
            ],
        ),
        (
            "Procesos",
            &[
                ("↑ ↓ / j k", "Moverse (PgUp/PgDn, g/G)"),
                ("/", "Buscar por nombre, comando, PID o usuario"),
                (
                    "c m p n u",
                    "Ordenar por CPU, memoria, PID, nombre, usuario",
                ),
                ("", "(repetir la tecla invierte el orden)"),
                ("o", "Solo mis procesos"),
                ("x", "Quitar filtros"),
                ("t / F9", "Terminar (SIGTERM), pide confirmación"),
                ("K", "Matar (SIGKILL), pide confirmación"),
            ],
        ),
    ];
    let mut l = Vec::new();
    for (titulo, filas) in secciones {
        l.push(Line::styled(format!(" {titulo}"), negrita(Color::Yellow)));
        for (k, d) in filas {
            l.push(Line::from(vec![
                Span::styled(format!("   {k:<18}"), Style::new().fg(ACENTO)),
                Span::raw(*d),
            ]));
        }
        l.push(Line::raw(""));
    }
    let r = centrar(f.area(), 72, l.len() as u16 + 2);
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(l).block(bloque(" Ayuda — cualquier tecla para cerrar ")),
        r,
    );
}
