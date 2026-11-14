// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Arma el BORRADOR de propuesta a partir de un prospecto y su oferta. PURO (sin red ni
//! disco): recibe el `Prospecto` y devuelve el markdown. El razonador lo personaliza; y
//! **el envío es de Juan** — por eso el borrador lleva el aviso en la cabecera (barandilla
//! #2 del diseño). El «cómo» del pitch por oferta vive aquí, en un sitio, no repetido.

use crate::contacto;
use crate::modelo::Prospecto;

/// El pitch de cada oferta de Xiliux: qué se vende y por qué es distinto. Corto; el
/// razonador lo adapta al prospecto.
fn pitch(oferta: &str) -> &'static str {
    match oferta {
        "desarrollo" => "Desarrollo backend en **Rust** (axum + SQLx/SQLite): un solo binario \
            que corre offline, sin el zoo de dependencias de npm/JVM, con los errores atrapados \
            en tiempo de compilación. Entrego producto, no un `node_modules` que mantener.",
        "cripto" => "Criptografía en **Rust puro**: X-Wing (híbrido ML-KEM-768 + X25519) y \
            ML-DSA (FIPS 204), validados byte-a-byte contra referencias independientes (arnés \
            diferencial). Migración post-cuántica y auditoría de la cripto que ya corre.",
        "herramientas" => "Herramientas de desarrollo que corren offline y sin telemetría: \
            detección de secretos (Chaquén), pruebas que DISCRIMINAN de verdad (sin-caso-rojo), \
            y un servidor MCP (Xué). Un binario cada una, sin cuenta ni nube.",
        "peritaje" => "Asesoría jurídico-técnica en **protección de datos (Ley 1581)**: abogado \
            **y** programador — el criterio jurídico con la profundidad técnica como prueba. \
            Diagnóstico del incidente, plan de remediación verificable, y respaldo pericial \
            ante la SIC. No teoría de PPT: se mide y se demuestra.",
        _ => "Software soberano en Rust: un binario que funciona offline, sin telemetría, \
            sin dependencias que mantener entre visitas.",
    }
}

/// El borrador markdown para un prospecto. Lleva la cabecera de BORRADOR (Juan revisa y
/// envía), el contexto del prospecto, el pitch de la oferta, la señal detectada, y un
/// hueco para la personalización del razonador (D20: hueco explícito, no relleno).
#[must_use]
pub fn redactar(p: &Prospecto) -> String {
    let contacto = match contacto::canal_de_negocio(&p.correos, &p.webs) {
        Some(canal) => canal,
        None if !p.webs.is_empty() => {
            format!("— resolver dominio a mano (solo vía agregador: {})", p.webs[0])
        }
        None => "— sin canal público (resolver a mano)".to_string(),
    };
    format!(
        "# BORRADOR de propuesta — {entidad}\n\n\
         > ⚠️ BORRADOR generado por chibchacúm. **Revísalo y envíalo TÚ.** El motor no envía.\n\n\
         - **Entidad:** {entidad}\n\
         - **Oferta:** {oferta}\n\
         - **Canal público:** {contacto}\n\
         - **Fuente/señal:** {fuente} — {senal}\n\n\
         ## Pitch ({oferta})\n\n{pitch}\n\n\
         ## Mensaje (personaliza antes de enviar)\n\n\
         Hola, equipo de {entidad}:\n\n\
         [El razonador ata aquí la señal detectada con el pitch — por qué ESTA oferta les \
         sirve, en 3-4 líneas, sin sonar a plantilla. Firmar como Juan Carlos Isaza / Xiliux.]\n",
        entidad = p.entidad,
        oferta = p.oferta,
        contacto = contacto,
        fuente = p.fuente,
        senal = p.senal,
        pitch = pitch(&p.oferta),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn prospecto() -> Prospecto {
        Prospecto {
            id: "1".into(),
            fuente: "hn-hiring".into(),
            oferta: "cripto".into(),
            puntaje: 2,
            entidad: "Acme".into(),
            senal: "post-quantum migration".into(),
            correos: vec!["hi@acme.io".into()],
            webs: vec!["https://acme.io".into()],
            fecha: "2026-09-28".into(),
        }
    }

    #[test]
    fn borrador_lleva_aviso_y_pitch_de_la_oferta() {
        let md = redactar(&prospecto());
        assert!(md.contains("Revísalo y envíalo TÚ"), "el aviso de no-auto-envío");
        assert!(md.contains("Acme"));
        assert!(md.contains("ML-KEM-768"), "el pitch de cripto: {md}");
        assert!(md.contains("hi@acme.io"));
    }

    // Discrimina: la oferta cambia el pitch (D33 — un borrador ciego daría el mismo).
    #[test]
    fn el_pitch_cambia_con_la_oferta() {
        let mut p = prospecto();
        p.oferta = "desarrollo".into();
        let md = redactar(&p);
        assert!(md.contains("axum"), "pitch de desarrollo");
        assert!(!md.contains("ML-KEM-768"), "no el de cripto");
    }

    #[test]
    fn sin_contacto_lo_dice() {
        let mut p = prospecto();
        p.correos.clear();
        p.webs.clear();
        assert!(redactar(&p).contains("sin canal público"));
    }
}
