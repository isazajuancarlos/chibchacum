// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! El `Prospecto` y la costura PURA que convierte un comentario de «Who is hiring» en
//! uno: limpia el HTML, clasifica la oferta (`oferta`), extrae el canal público
//! (`contacto`) y saca el nombre de la entidad. Sin red — la red vive en `red`, esta
//! costura es la MISMA que ejercen las pruebas (D28/D33).

use crate::{contacto, oferta};

/// Un prospecto: una entidad con una necesidad que Xiliux resuelve, y por dónde
/// contactarla. `id` es la clave de dedup (para HN, el `objectID` del comentario).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prospecto {
    pub id: String,
    pub fuente: String,
    pub oferta: String,
    pub puntaje: i64,
    pub entidad: String,
    pub senal: String,
    pub correos: Vec<String>,
    pub webs: Vec<String>,
    pub fecha: String,
}

/// Quita etiquetas `<...>` y decodifica las entidades que HN usa en `comment_text`.
#[must_use]
pub fn limpiar_html(s: &str) -> String {
    // quitar etiquetas
    let mut sin_tags = String::with_capacity(s.len());
    let mut dentro = false;
    for c in s.chars() {
        match c {
            '<' => dentro = true,
            '>' => dentro = false,
            _ if !dentro => sin_tags.push(c),
            _ => {}
        }
    }
    sin_tags
        .replace("<p>", " ")
        .replace("&#x2F;", "/")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// Segmentos que NO son el nombre de la empresa: metadatos que algunas ofertas de HN
/// ponen primero. Si un segmento empieza por uno de estos, se salta.
const ETIQUETAS_META: &[&str] = &[
    "location", "remote", "onsite", "on-site", "hybrid", "salary", "comp", "compensation",
    "role", "position", "stack", "tech", "url", "apply", "contact", "email", "website",
    "about", "team", "who we are", "full-time", "part-time", "we are ", "we're ",
    "seeking freelancer", "seeking work", "seeking", "rate", "budget", "duration",
];

/// El nombre de la entidad. Las ofertas de HN suelen empezar por `Empresa | rol | …`,
/// pero no siempre: algunas ponen `Location: …` o `We're hiring at X` primero. Heurística:
/// quita el prefijo `(We're|We are|Hiring) at `, y devuelve el primer segmento (por `|`,
/// salto o guion largo) que NO empiece por una etiqueta de metadatos. Acotado a 60.
#[must_use]
pub fn entidad_de(texto_limpio: &str) -> String {
    let t = texto_limpio.trim();
    let t = ["We're hiring at ", "We are hiring at ", "Hiring at ", "We're hiring: ", "Hiring: "]
        .iter()
        .find_map(|p| t.strip_prefix(p))
        .unwrap_or(t);
    for seg in t.split(['|', '\n', '—', '–']) {
        let s = seg.trim().trim_start_matches([':', '-', '•', '*']).trim();
        if s.is_empty() {
            continue;
        }
        let bajo = s.to_lowercase();
        if ETIQUETAS_META.iter().any(|e| bajo.starts_with(e)) {
            continue;
        }
        return s.chars().take(60).collect::<String>().trim().to_string();
    }
    // Todo eran etiquetas: cae al primer segmento, acotado.
    t.split(['|', '\n'])
        .next()
        .unwrap_or("")
        .chars()
        .take(60)
        .collect::<String>()
        .trim()
        .to_string()
}

/// En el hilo «Freelancer? Seeking freelancer?», un comentario es de una EMPRESA que
/// busca freelancer si empieza por `SEEKING FREELANCER`. `SEEKING WORK` es una PERSONA
/// ofreciéndose → se excluye (barandilla #1: no datos personales de individuos).
#[must_use]
pub fn es_seeking_freelancer(texto_html: &str) -> bool {
    let t = limpiar_html(texto_html).to_uppercase();
    let t = t.trim_start();
    t.starts_with("SEEKING FREELANCER") || (t.contains("SEEKING FREELANCER") && !t.contains("SEEKING WORK"))
}

/// Convierte un comentario (id, texto HTML, fecha, fuente) en un `Prospecto`, o `None`
/// si no sirve. **Sirve** = casa una oferta (puntaje>0) O trae contacto público; si no
/// es ni lo uno ni lo otro, no es un prospecto (no se rellena, D20).
#[must_use]
pub fn prospecto_de_comentario(
    id: &str,
    texto_html: &str,
    fecha: &str,
    fuente: &str,
) -> Option<Prospecto> {
    let limpio = limpiar_html(texto_html);
    let (oferta, puntaje) = oferta::clasificar_texto(&limpio);
    // El contacto se extrae del HTML CRUDO: la URL de negocio suele vivir en el
    // atributo `href="…"`, que `limpiar_html` borraría con la etiqueta. Los correos son
    // texto plano en ambos, así que el crudo los pilla igual.
    let contacto = contacto::extraer_contacto(texto_html);

    // Filtro: sin oferta que case Y sin contacto, no hay prospecto.
    if puntaje == 0 && contacto.vacio() {
        return None;
    }
    Some(Prospecto {
        id: id.to_string(),
        fuente: fuente.to_string(),
        oferta: oferta.to_string(),
        puntaje: puntaje as i64,
        entidad: entidad_de(&limpio),
        senal: limpio.chars().take(280).collect::<String>().trim().to_string(),
        correos: contacto.correos,
        webs: contacto.webs,
        fecha: fecha.to_string(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const COMENTARIO_HN: &str = "Acme Corp (YC S21) | Senior <b>Rust</b> Engineer | Remote \
        | We need a Rust backend engineer for our axum services. Apply: \
        <a href=\"https://acme.io/jobs\">jobs</a> or email jobs&#x2F;at&#x2F;acme.io. \
        Contact: hiring@acme.io";

    #[test]
    fn comentario_de_hiring_produce_prospecto_util() {
        let p = prospecto_de_comentario("42", COMENTARIO_HN, "2026-09-01", "hn-hiring").unwrap();
        assert_eq!(p.oferta, "desarrollo", "Rust/axum → desarrollo");
        assert!(p.puntaje >= 2);
        assert_eq!(p.entidad, "Acme Corp (YC S21)");
        assert!(p.correos.contains(&"hiring@acme.io".to_string()), "{:?}", p.correos);
        assert!(p.webs.iter().any(|w| w.contains("acme.io")), "{:?}", p.webs);
    }

    // CONTROL (D33): un comentario de charla, sin oferta ni contacto, NO es prospecto.
    #[test]
    fn comentario_de_ruido_no_es_prospecto() {
        let ruido = "Congrats on the launch! This looks great, wishing you the best.";
        assert!(prospecto_de_comentario("7", ruido, "2026-09-01", "hn-hiring").is_none());
    }

    // Un comentario con contacto PERO sin oferta que case sigue siendo prospecto débil
    // (puntaje 0) — el razonador decide; no se descarta un contacto real.
    #[test]
    fn contacto_sin_oferta_sigue_siendo_prospecto() {
        let t = "We build fintech things. Reach us: talent@fin.co";
        let p = prospecto_de_comentario("9", t, "2026-09-01", "hn-hiring").unwrap();
        assert_eq!(p.oferta, oferta::SIN_CLASIFICAR);
        assert_eq!(p.correos, vec!["talent@fin.co"]);
    }

    #[test]
    fn limpiar_html_decodifica() {
        assert_eq!(limpiar_html("a&#x2F;b &amp; <b>c</b>"), "a/b & c");
    }

    #[test]
    fn seeking_freelancer_discrimina_empresa_de_persona() {
        assert!(es_seeking_freelancer("SEEKING FREELANCER | Acme needs a Rust dev | jobs@acme.io"));
        // SEEKING WORK = una persona ofreciéndose → NO (barandilla #1).
        assert!(!es_seeking_freelancer("SEEKING WORK | 5 yrs Rust | me@gmail.com"));
        // Un comentario cualquiera tampoco.
        assert!(!es_seeking_freelancer("Great thread!"));
    }

    #[test]
    fn entidad_salta_metadatos() {
        // Formato estándar: empresa primero.
        assert_eq!(entidad_de("Acme Corp | Senior Rust | Remote"), "Acme Corp");
        // Ruido real: empieza por Location → salta al primer no-metadato.
        assert_eq!(entidad_de("Location: London, UK | Beta Ltd | Backend"), "Beta Ltd");
        // "We're hiring at X — …" → X.
        assert_eq!(entidad_de("We're hiring at Langfuse — now part of ClickHouse"), "Langfuse");
    }
}
