//! Estado de la interfaz: pestaña activa, lista de procesos (búsqueda, orden y
//! selección), ventanas modales y traducción de teclas a acciones.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use serde::Deserialize;

use crate::{
    config::Config,
    gpu, i18n,
    sistema::{ErrorSenal, Monitor, Proceso},
};

/// Pestañas del panel derecho, en el orden en que se muestran (teclas `1`–`7`).
/// En la configuración se escriben en inglés y minúsculas (`"summary"`, `"processes"`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pestana {
    #[serde(rename = "summary")]
    Resumen,
    Cpu,
    Ram,
    #[serde(rename = "storage")]
    Almacenamiento,
    Gpu,
    #[serde(rename = "network")]
    Red,
    #[serde(rename = "processes")]
    Procesos,
}

impl Pestana {
    pub const TODAS: [Pestana; 7] = [
        Pestana::Resumen,
        Pestana::Cpu,
        Pestana::Ram,
        Pestana::Almacenamiento,
        Pestana::Gpu,
        Pestana::Red,
        Pestana::Procesos,
    ];
    /// Nombre de la pestaña en el idioma de la interfaz.
    pub fn titulo(self) -> &'static str {
        let t = i18n::t();
        match self {
            Pestana::Resumen => t.tab_summary,
            Pestana::Cpu => t.tab_cpu,
            Pestana::Ram => t.tab_ram,
            Pestana::Almacenamiento => t.tab_storage,
            Pestana::Gpu => t.tab_gpu,
            Pestana::Red => t.tab_network,
            Pestana::Procesos => t.tab_processes,
        }
    }
    fn indice(self) -> usize {
        Self::TODAS.iter().position(|p| *p == self).unwrap_or(0)
    }
}

/// Columna por la que se ordena la pestaña Procesos.
/// En la configuración se escriben en inglés y minúsculas (`"cpu"`, `"memory"`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrdenProc {
    Cpu,
    #[serde(rename = "memory")]
    Memoria,
    Pid,
    #[serde(rename = "name")]
    Nombre,
    #[serde(rename = "user")]
    Usuario,
}

impl OrdenProc {
    /// Números de mayor a menor (lo que más consume, arriba); texto de la A a la Z.
    fn descendente_por_defecto(self) -> bool {
        matches!(self, OrdenProc::Cpu | OrdenProc::Memoria)
    }
}

/// Qué recibe las teclas: la vista normal, la búsqueda o una ventana modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modo {
    Normal,
    Buscando,
    Ayuda,
    Confirmar {
        pid: u32,
        nombre: String,
        forzar: bool,
    },
}

pub struct App {
    /// Datos del sistema; se refrescan en cada [`App::tick`].
    pub monitor: Monitor,
    pub pestana: Pestana,
    pub modo: Modo,
    pub pausado: bool,
    pub salir: bool,

    pub tabla_proc: TableState,
    pub orden_proc: OrdenProc,
    pub desc: bool,
    pub busqueda: String,
    pub solo_mios: bool,
    /// Mostrar el logo encima del panel Sistema (`logo` en la configuración).
    pub logo: bool,
    sel_pid: Option<u32>,

    pub mensaje: Option<(String, bool, Instant)>,
}

impl App {
    /// Crea el estado inicial con la pestaña, el orden y el filtro de la configuración.
    pub fn new(monitor: Monitor, config: &Config) -> Self {
        let orden = config.processes.sort;
        let mut app = Self {
            monitor,
            pestana: Pestana::Resumen,
            modo: Modo::Normal,
            pausado: false,
            salir: false,
            tabla_proc: TableState::default().with_selected(0),
            orden_proc: orden,
            desc: orden.descendente_por_defecto(),
            busqueda: String::new(),
            solo_mios: config.processes.mine_only,
            logo: config.logo,
            sel_pid: None,
            mensaje: None,
        };
        // `ir_a` también activa la consulta de la GPU NVIDIA si se empieza en esa pestaña.
        app.ir_a(config.start_tab);
        app.sincronizar_seleccion();
        app
    }

    pub fn tick(&mut self) {
        if !self.pausado {
            self.monitor.actualizar();
            self.sincronizar_seleccion();
        }
        if self
            .mensaje
            .as_ref()
            .is_some_and(|(_, _, t)| t.elapsed().as_secs() >= 4)
        {
            self.mensaje = None;
        }
    }

    /// Procesos filtrados y ordenados como en la pestaña Procesos.
    pub fn procesos(&self) -> Vec<&Proceso> {
        let q = self.busqueda.to_lowercase();
        let yo = &self.monitor.info.usuario;
        let mut v: Vec<&Proceso> = self
            .monitor
            .procesos
            .iter()
            .filter(|p| !self.solo_mios || &p.usuario == yo)
            .filter(|p| {
                q.is_empty()
                    || p.nombre.to_lowercase().contains(&q)
                    || p.comando.to_lowercase().contains(&q)
                    || p.pid.to_string() == q
                    || p.usuario.to_lowercase() == q
            })
            .collect();
        v.sort_by(|a, b| {
            let o = match self.orden_proc {
                OrdenProc::Cpu => a.cpu.total_cmp(&b.cpu),
                OrdenProc::Memoria => a.memoria.cmp(&b.memoria),
                OrdenProc::Pid => a.pid.cmp(&b.pid),
                OrdenProc::Nombre => a.nombre.to_lowercase().cmp(&b.nombre.to_lowercase()),
                OrdenProc::Usuario => a.usuario.cmp(&b.usuario),
            };
            if self.desc { o.reverse() } else { o }.then(a.pid.cmp(&b.pid))
        });
        v
    }

    /// Top N procesos por CPU o memoria (para Resumen/CPU/RAM).
    pub fn top(&self, por_memoria: bool, n: usize) -> Vec<&Proceso> {
        let mut v: Vec<&Proceso> = self.monitor.procesos.iter().collect();
        if por_memoria {
            v.sort_by_key(|p| std::cmp::Reverse(p.memoria));
        } else {
            v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        }
        v.truncate(n);
        v
    }

    /// Mantiene seleccionado el mismo PID aunque la lista se reordene.
    fn sincronizar_seleccion(&mut self) {
        let lista = self.procesos();
        let pos = self
            .sel_pid
            .and_then(|pid| lista.iter().position(|p| p.pid == pid))
            .or_else(|| {
                (!lista.is_empty())
                    .then(|| self.tabla_proc.selected().unwrap_or(0).min(lista.len() - 1))
            });
        self.sel_pid = pos.map(|i| lista[i].pid);
        self.tabla_proc.select(pos);
    }

    /// Tras buscar, filtrar u ordenar: volver al primer proceso de la lista.
    fn al_inicio(&mut self) {
        self.sel_pid = None;
        self.tabla_proc = TableState::default().with_selected(0);
        self.sincronizar_seleccion();
    }

    fn mover(&mut self, delta: isize) {
        let lista = self.procesos();
        if lista.is_empty() {
            return;
        }
        let i = self.tabla_proc.selected().unwrap_or(0) as isize;
        let n = (i.saturating_add(delta)).clamp(0, lista.len() as isize - 1) as usize;
        self.sel_pid = Some(lista[n].pid);
        self.tabla_proc.select(Some(n));
    }

    fn seleccionado(&self) -> Option<&Proceso> {
        let pid = self.sel_pid?;
        self.monitor.procesos.iter().find(|p| p.pid == pid)
    }

    fn ir_a(&mut self, p: Pestana) {
        self.pestana = p;
        gpu::QUERY_NVIDIA.store(p == Pestana::Gpu, std::sync::atomic::Ordering::Relaxed);
    }

    fn ordenar(&mut self, o: OrdenProc) {
        if self.orden_proc == o {
            self.desc = !self.desc;
        } else {
            self.orden_proc = o;
            self.desc = o.descendente_por_defecto();
        }
        self.al_inicio();
    }

    fn avisar(&mut self, texto: impl Into<String>, error: bool) {
        self.mensaje = Some((texto.into(), error, Instant::now()));
    }

    pub fn tecla(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.salir = true;
            return;
        }
        match self.modo.clone() {
            Modo::Ayuda => self.modo = Modo::Normal,
            Modo::Confirmar {
                pid,
                nombre,
                forzar,
            } => {
                if matches!(
                    k.code,
                    KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter
                ) {
                    let t = i18n::t();
                    match self.monitor.senal(pid, forzar) {
                        Ok(()) => self.avisar(
                            format!(
                                "{} {} {nombre} ({pid})",
                                if forzar { "SIGKILL" } else { "SIGTERM" },
                                t.notice_sent_to
                            ),
                            false,
                        ),
                        Err(e) => {
                            let motivo = match e {
                                ErrorSenal::NoExiste => t.signal_no_such_process,
                                ErrorSenal::SinPermiso => t.signal_permission_denied,
                                ErrorSenal::NoSoportada => t.signal_unsupported,
                            };
                            self.avisar(
                                format!("{} {nombre}: {motivo}", t.notice_could_not_terminate),
                                true,
                            )
                        }
                    }
                }
                self.modo = Modo::Normal;
            }
            Modo::Buscando => match k.code {
                KeyCode::Esc => {
                    self.busqueda.clear();
                    self.modo = Modo::Normal;
                    self.al_inicio();
                }
                KeyCode::Enter => self.modo = Modo::Normal,
                KeyCode::Backspace => {
                    self.busqueda.pop();
                    self.al_inicio();
                }
                KeyCode::Up => self.mover(-1),
                KeyCode::Down => self.mover(1),
                KeyCode::Char(c) => {
                    self.busqueda.push(c);
                    self.al_inicio();
                }
                _ => {}
            },
            Modo::Normal => self.tecla_normal(k),
        }
    }

    fn tecla_normal(&mut self, k: KeyEvent) {
        let i = self.pestana.indice();
        let n = Pestana::TODAS.len();
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => self.salir = true,
            KeyCode::Char('?') => self.modo = Modo::Ayuda,
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.ir_a(Pestana::TODAS[(i + 1) % n])
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.ir_a(Pestana::TODAS[(i + n - 1) % n])
            }
            KeyCode::Char(c @ '1'..='7') => self.ir_a(Pestana::TODAS[c as usize - '1' as usize]),
            KeyCode::Char(' ') => self.pausado = !self.pausado,
            _ if self.pestana == Pestana::Procesos => self.tecla_procesos(k),
            _ => {}
        }
    }

    fn tecla_procesos(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.mover(-1),
            KeyCode::Down | KeyCode::Char('j') => self.mover(1),
            KeyCode::PageUp => self.mover(-15),
            KeyCode::PageDown => self.mover(15),
            KeyCode::Home | KeyCode::Char('g') => self.mover(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.mover(isize::MAX),
            KeyCode::Char('/') => self.modo = Modo::Buscando,
            KeyCode::Char('c') => self.ordenar(OrdenProc::Cpu),
            KeyCode::Char('m') => self.ordenar(OrdenProc::Memoria),
            KeyCode::Char('p') => self.ordenar(OrdenProc::Pid),
            KeyCode::Char('n') => self.ordenar(OrdenProc::Nombre),
            KeyCode::Char('u') => self.ordenar(OrdenProc::Usuario),
            KeyCode::Char('o') => {
                self.solo_mios = !self.solo_mios;
                self.al_inicio();
            }
            KeyCode::Char('x') => {
                self.busqueda.clear();
                self.solo_mios = false;
                self.al_inicio();
            }
            KeyCode::Char('t') | KeyCode::Char('K') | KeyCode::F(9) => {
                if let Some(p) = self.seleccionado() {
                    self.modo = Modo::Confirmar {
                        pid: p.pid,
                        nombre: p.nombre.clone(),
                        forzar: k.code == KeyCode::Char('K'),
                    };
                }
            }
            _ => {}
        }
    }
}
