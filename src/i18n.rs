//! Idioma de la interfaz y textos traducidos.
//!
//! El idioma se detecta del entorno con [`detectar`]. Los textos se obtienen con [`t`],
//! que devuelve la tabla del idioma activo.
//!
//! Para añadir un texto: crea el campo en [`Textos`] y dale valor en [`ES`] y [`EN`].
//! El compilador obliga a traducirlo en los dos idiomas.

use std::{env, sync::OnceLock};

/// Idiomas disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Es,
    En,
}

static IDIOMA: OnceLock<Idioma> = OnceLock::new();

/// Idioma activo: el del entorno, detectado la primera vez que se consulta.
pub fn actual() -> Idioma {
    *IDIOMA.get_or_init(detectar)
}

/// Textos del idioma activo.
pub fn t() -> &'static Textos {
    match actual() {
        Idioma::Es => &ES,
        Idioma::En => &EN,
    }
}

/// Idioma según las variables de entorno, en el orden de prioridad de gettext:
/// `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` y `LANG`. Español si el idioma es `es`;
/// inglés en cualquier otro caso (incluido el locale `C`/`POSIX`).
pub fn detectar() -> Idioma {
    detectar_con(|clave| env::var(clave).ok())
}

fn detectar_con(var: impl Fn(&str) -> Option<String>) -> Idioma {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| var(k).filter(|v| !v.is_empty()));
    // gettext ignora LANGUAGE si el locale es C/POSIX.
    let es_c = locale
        .as_deref()
        .is_none_or(|l| l == "C" || l == "POSIX" || l.starts_with("C."));
    let preferido = var("LANGUAGE")
        .filter(|v| !v.is_empty() && !es_c)
        .and_then(|v| v.split(':').next().map(str::to_string));
    match preferido.or(locale) {
        Some(v) if codigo(&v) == "es" => Idioma::Es,
        _ => Idioma::En,
    }
}

/// `"es_CO.UTF-8@euro"` → `"es"`.
fn codigo(locale: &str) -> String {
    locale
        .split(['_', '.', '@', '-'])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// Todos los textos de la interfaz en un idioma.
pub struct Textos {
    // Pestañas
    pub pestana_resumen: &'static str,
    pub pestana_cpu: &'static str,
    pub pestana_ram: &'static str,
    pub pestana_almacenamiento: &'static str,
    pub pestana_gpu: &'static str,
    pub pestana_red: &'static str,
    pub pestana_procesos: &'static str,
}

pub const ES: Textos = Textos {
    pestana_resumen: "Resumen",
    pestana_cpu: "CPU",
    pestana_ram: "RAM",
    pestana_almacenamiento: "Almacenamiento",
    pestana_gpu: "GPU",
    pestana_red: "Red",
    pestana_procesos: "Procesos",
};

pub const EN: Textos = Textos {
    pestana_resumen: "Overview",
    pestana_cpu: "CPU",
    pestana_ram: "Memory",
    pestana_almacenamiento: "Storage",
    pestana_gpu: "GPU",
    pestana_red: "Network",
    pestana_procesos: "Processes",
};

#[cfg(test)]
mod tests {
    use super::*;

    fn con(vars: &[(&str, &str)]) -> Idioma {
        detectar_con(|k| {
            vars.iter()
                .find(|(c, _)| *c == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn detecta_espanol_en_cualquier_pais() {
        assert_eq!(con(&[("LANG", "es_CO.UTF-8")]), Idioma::Es);
        assert_eq!(con(&[("LANG", "es_ES@euro")]), Idioma::Es);
        assert_eq!(con(&[("LANG", "es")]), Idioma::Es);
    }

    #[test]
    fn cualquier_otro_idioma_usa_ingles() {
        assert_eq!(con(&[("LANG", "en_US.UTF-8")]), Idioma::En);
        assert_eq!(con(&[("LANG", "fr_FR.UTF-8")]), Idioma::En);
        assert_eq!(con(&[("LANG", "C.UTF-8")]), Idioma::En);
        assert_eq!(con(&[]), Idioma::En);
    }

    #[test]
    fn respeta_la_prioridad_de_las_variables() {
        assert_eq!(
            con(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "es_MX.UTF-8")]),
            Idioma::Es
        );
        assert_eq!(
            con(&[("LC_MESSAGES", "es_MX.UTF-8"), ("LC_ALL", "en_GB.UTF-8")]),
            Idioma::En
        );
        assert_eq!(
            con(&[("LANG", "en_US.UTF-8"), ("LANGUAGE", "es:en")]),
            Idioma::Es
        );
        assert_eq!(
            con(&[("LANG", ""), ("LC_ALL", ""), ("LC_MESSAGES", "es_AR")]),
            Idioma::Es
        );
    }

    #[test]
    fn language_se_ignora_con_el_locale_c() {
        assert_eq!(con(&[("LANG", "C"), ("LANGUAGE", "es")]), Idioma::En);
    }
}
