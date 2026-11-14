// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Extrae el CANAL DE NEGOCIO público de un texto (un comentario de «Who is hiring»,
//! la descripción de un repo): correos y webs que la entidad PUBLICÓ para que la
//! contacten. Función PURA, sin red y **sin dependencia de regex** (dep-free): escaneo
//! de caracteres, como el resto del stack.
//!
//! Barandilla #1 del diseño: esto recoge el contacto que la propia entidad publicó —no
//! compila datos personales cruzando fuentes—. Y el envío lo hace Juan (#2).

/// Lo que se pudo extraer de un texto: correos y webs, deduplicados y en orden de
/// aparición. Vacío si no hay nada — ausencia, no relleno (D20).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contacto {
    pub correos: Vec<String>,
    pub webs: Vec<String>,
}

impl Contacto {
    #[must_use]
    pub fn vacio(&self) -> bool {
        self.correos.is_empty() && self.webs.is_empty()
    }
}

/// Proveedores de correo PERSONAL: un `careers@gmail.com` no existe. Si el único
/// contacto de un "prospecto" es uno de estos, es una PERSONA, no una entidad —
/// barandilla #1: se contacta el canal de negocio de la entidad, no un inbox personal.
const DOMINIOS_PERSONALES: &[&str] = &[
    "gmail.com", "googlemail.com", "proton.me", "protonmail.com", "pm.me", "outlook.com",
    "hotmail.com", "live.com", "msn.com", "yahoo.com", "ymail.com", "icloud.com", "me.com",
    "aol.com", "gmx.com", "mail.com", "yandex.com", "yandex.ru",
];

fn es_dominio_personal(dominio: &str) -> bool {
    let d = dominio.to_ascii_lowercase();
    DOMINIOS_PERSONALES.iter().any(|p| d == *p)
}

/// Hosts que NO son el sitio de la empresa: agregadores de empleo, ATS, redes. Una URL
/// de estos no sirve para enviarle una propuesta a la ENTIDAD.
const AGREGADORES: &[&str] = &[
    "remoteok.com", "remoteok.io", "news.ycombinator.com", "ycombinator.com",
    "notion.site", "notion.so", "twitter.com", "x.com", "medium.com", "substack.com",
    "linkedin.com", "lever.co", "greenhouse.io", "ashbyhq.com", "workable.com",
    "bit.ly", "weworkremotely.com", "himalayas.app", "github.io",
];

fn host_de(url: &str) -> String {
    url.to_ascii_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_string()
}

fn es_agregador(url: &str) -> bool {
    let h = host_de(url);
    AGREGADORES.iter().any(|a| h == *a || h.ends_with(&format!(".{a}")))
}

/// R3 — el MEJOR canal directo para contactar a la entidad: un correo de negocio, o si
/// no la web PROPIA de la empresa (no un agregador/ATS). `None` si solo hay agregadores:
/// entonces el dominio se resuelve a mano (ausencia honesta, no relleno; D20).
#[must_use]
pub fn canal_de_negocio(correos: &[String], webs: &[String]) -> Option<String> {
    if let Some(c) = correos.first() {
        return Some(c.clone()); // un correo es el canal directo por excelencia
    }
    webs.iter().find(|w| !es_agregador(w)).cloned()
}

/// Un char válido en la parte LOCAL de un correo (antes de la `@`).
fn char_local(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
}
/// Un char válido en el DOMINIO (después de la `@`, o en un host).
fn char_dominio(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-')
}

/// Extrae correos y webs de `texto`. Un correo exige `local@dominio.tld` (el dominio
/// tiene al menos un punto con algo después); una web, `http(s)://host…` hasta el
/// primer espacio o delimitador. Deduplica conservando el orden.
#[must_use]
pub fn extraer_contacto(texto: &str) -> Contacto {
    let mut correos = Vec::new();
    let mut webs = Vec::new();
    let chars: Vec<char> = texto.chars().collect();

    // --- correos: alrededor de cada '@' ---
    for (i, &c) in chars.iter().enumerate() {
        if c != '@' {
            continue;
        }
        // hacia atrás: parte local
        let mut ini = i;
        while ini > 0 && char_local(chars[ini - 1]) {
            ini -= 1;
        }
        // hacia adelante: dominio
        let mut fin = i + 1;
        while fin < chars.len() && char_dominio(chars[fin]) {
            fin += 1;
        }
        if ini == i || fin == i + 1 {
            continue; // sin local o sin dominio
        }
        let local: String = chars[ini..i].iter().collect();
        let dominio: String = chars[i + 1..fin].iter().collect();
        // un dominio real: tiene un punto, y algo (>=2) tras el último punto (TLD).
        let dominio = dominio.trim_matches('.');
        if let Some((_, tld)) = dominio.rsplit_once('.')
            && tld.len() >= 2
            && !local.is_empty()
            && !es_dominio_personal(dominio) // barandilla #1: no inboxes personales
        {
            let correo = format!("{local}@{dominio}").to_lowercase();
            if !correos.contains(&correo) {
                correos.push(correo);
            }
        }
    }

    // --- webs: cada http(s):// hasta el primer separador ---
    for marca in ["https://", "http://"] {
        let mut resto = texto;
        while let Some(p) = resto.find(marca) {
            let desde = &resto[p + marca.len()..];
            let fin = desde
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>' | ')' | ']' | '|'))
                .unwrap_or(desde.len());
            let host_y_ruta = desde[..fin].trim_end_matches(['.', ',', ';', ':', ')']);
            // el host (antes del primer '/') debe tener un punto: es un dominio real.
            let host = host_y_ruta.split('/').next().unwrap_or("");
            if host.contains('.') && host.chars().all(char_dominio) && !host.is_empty() {
                let web = format!("{marca}{host_y_ruta}");
                if !webs.contains(&web) {
                    webs.push(web);
                }
            }
            resto = &desde[fin..];
        }
    }

    Contacto { correos, webs }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn aguja_correo_y_web() {
        let t = "We're hiring! Apply: jobs@acme.io or see https://acme.io/careers for details.";
        let c = extraer_contacto(t);
        assert_eq!(c.correos, vec!["jobs@acme.io"]);
        assert_eq!(c.webs, vec!["https://acme.io/careers"]);
        assert!(!c.vacio());
    }

    #[test]
    fn dedup_conserva_orden() {
        let t = "mail hi@x.com, again hi@x.com and https://x.com https://x.com";
        let c = extraer_contacto(t);
        assert_eq!(c.correos, vec!["hi@x.com"]);
        assert_eq!(c.webs, vec!["https://x.com"]);
    }

    // CONTROL (D33): sin dirección real, no inventa contacto.
    #[test]
    fn control_sin_contacto_queda_vacio() {
        let c = extraer_contacto("Email us on our website to apply. Great team, remote-first.");
        assert!(c.vacio(), "{c:?}");
    }

    // CONTROL (D33): `@handle` de red social y correo sin TLD NO son correos.
    #[test]
    fn control_arroba_sin_dominio_valido() {
        let c = extraer_contacto("ping @founder on X, or user@localhost internal only");
        assert!(c.correos.is_empty(), "ni handle ni user@localhost: {c:?}");
    }

    // CONTROL (D33): un http sin host con punto (localhost) no es web de negocio.
    #[test]
    fn control_localhost_no_es_web() {
        let c = extraer_contacto("run it at http://localhost:8080 to test");
        assert!(c.webs.is_empty(), "localhost no es canal de negocio: {c:?}");
    }

    // BARANDILLA #1 (D33): un correo de proveedor PERSONAL no es canal de negocio.
    // Aguja: careers@empresa.com entra. Control: alguien@gmail.com NO.
    #[test]
    fn correo_personal_no_es_canal_de_negocio() {
        let c = extraer_contacto("careers@estuary.dev is us; a candidate wrote from jane@gmail.com");
        assert_eq!(c.correos, vec!["careers@estuary.dev"], "gmail excluido: {c:?}");
    }

    // R3 (D33): el correo manda; si no, la web PROPIA; un agregador solo NO cuenta.
    #[test]
    fn canal_de_negocio_prefiere_correo_luego_web_propia() {
        // correo gana.
        assert_eq!(
            canal_de_negocio(&["hi@acme.io".into()], &["https://remoteok.com/x".into()]),
            Some("hi@acme.io".into())
        );
        // sin correo: la web propia, no el agregador.
        assert_eq!(
            canal_de_negocio(&[], &["https://remoteok.com/x".into(), "https://acme.io".into()]),
            Some("https://acme.io".into())
        );
        // CONTROL: solo agregadores → None (resolver a mano).
        assert_eq!(canal_de_negocio(&[], &["https://remoteok.com/jobs/123".into()]), None);
        assert_eq!(canal_de_negocio(&[], &["https://lever.co/acme".into()]), None);
    }
}
