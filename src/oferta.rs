// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! El mapa señal→OFERTA: dado el texto de una señal pública (una oferta de empleo, un
//! repo, un hilo de comunidad), dice a qué oferta de Xiliux encaja el prospecto y con
//! cuánta fuerza (palabras clave que casaron). Función PURA: no toca red ni disco, así
//! la prueba la valida contra el ARTEFACTO y no contra un arnés (D33).
//!
//! Es el gemelo de `categoria` de mindalá, pero categoriza hacia lo que VENDEMOS, no
//! hacia el proyecto que aprovecha una técnica. El juicio fino lo pone el razonador;
//! esto es el hint de primer corte.

/// Ninguna oferta casó con fuerza: el razonador decide si vale la pena.
pub const SIN_CLASIFICAR: &str = "sin_clasificar";

struct Regla {
    oferta: &'static str,
    /// Todo en minúscula; se casa por subcadena sobre el texto en minúscula.
    señales: &'static [&'static str],
}

const MAPA: &[Regla] = &[
    Regla {
        // Desarrollo Rust a medida: la entidad admite por escrito que necesita Rust.
        oferta: "desarrollo",
        señales: &[
            "rust developer",
            "rust engineer",
            "hiring rust",
            "rust backend",
            "backend engineer",
            "systems programming",
            "axum",
            "tokio",
            "actix",
            "sqlx",
            "microservice",
            "high-performance backend",
            "memory safety",
            "rewrite in rust",
        ],
    },
    Regla {
        // Cripto / PQC: migración post-cuántica o criptografía que hay que endurecer.
        oferta: "cripto",
        señales: &[
            "post-quantum",
            "post quantum",
            "pqc",
            "quantum-safe",
            "quantum safe",
            "ml-kem",
            "ml-dsa",
            "kyber",
            "dilithium",
            "crypto agility",
            "cryptographic migration",
            "key management",
            "end-to-end encryption",
            "hybrid encryption",
            "fips 203",
            "fips 204",
        ],
    },
    Regla {
        // Peritaje jurídico-técnico: cumplimiento de protección de datos. La identidad
        // doble de Juan (abogado + técnico). Señal: incidente/obligación de datos.
        oferta: "peritaje",
        señales: &[
            "habeas data",
            "ley 1581",
            "protección de datos",
            "proteccion de datos",
            "data protection",
            "privacy compliance",
            "gdpr",
            "data breach",
            "brecha de datos",
            "fuga de datos",
            "data protection officer",
            "oficial de protección de datos",
            "tratamiento de datos personales",
            "sanción sic",
            "superintendencia de industria",
        ],
    },
    Regla {
        // Herramientas de la tienda: el dolor que Xué (MCP), Chaquén (secretos) y
        // sin-caso-rojo (pruebas que discriminan) resuelven.
        oferta: "herramientas",
        señales: &[
            "secret scanning",
            "secret detection",
            "detect secrets",
            "leaked credentials",
            "leaked secret",
            "hardcoded secret",
            "gitleaks",
            "credential leak",
            "flaky test",
            "test coverage gap",
            "tests always pass",
            "mcp server",
            "model context protocol",
        ],
    },
];

/// La oferta que mejor casa con el texto y su puntaje (nº de señales distintas que
/// aparecen). Si ninguna casa, `(SIN_CLASIFICAR, 0)`. Empate → la primera del mapa
/// (orden deliberado: desarrollo es el mercado más amplio).
#[must_use]
pub fn clasificar_texto(texto: &str) -> (&'static str, usize) {
    let bajo = texto.to_lowercase();
    let mut mejor = (SIN_CLASIFICAR, 0usize);
    for regla in MAPA {
        let n = regla
            .señales
            .iter()
            .filter(|s| bajo.contains(**s))
            .count();
        if n > mejor.1 {
            mejor = (regla.oferta, n);
        }
    }
    mejor
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Aguja por oferta: un texto claro cae en su oferta.
    #[test]
    fn desarrollo_rust() {
        let (o, n) = clasificar_texto("We are hiring a Rust backend engineer (axum, tokio)");
        assert_eq!(o, "desarrollo");
        assert!(n >= 2, "puntaje {n}");
    }

    #[test]
    fn cripto_pqc() {
        let (o, _) = clasificar_texto("Planning our post-quantum migration to ML-KEM (FIPS 203)");
        assert_eq!(o, "cripto");
    }

    #[test]
    fn peritaje_proteccion_datos() {
        let (o, _) = clasificar_texto("Empresa sancionada por la SIC por violar la Ley 1581 (Habeas Data)");
        assert_eq!(o, "peritaje");
    }

    #[test]
    fn herramientas_secretos() {
        let (o, _) = clasificar_texto("We keep leaking hardcoded secrets; need secret scanning");
        assert_eq!(o, "herramientas");
    }

    // CONTROL (D33): texto sin señal de negocio NO se clasifica — no un falso positivo.
    #[test]
    fn ruido_no_clasifica() {
        let (o, n) = clasificar_texto("Beautiful sunset over the mountains, great coffee today");
        assert_eq!(o, SIN_CLASIFICAR);
        assert_eq!(n, 0);
    }

    // CONTROL (D33): NO confundir criptomoneda con criptografía. Un hilo de trading no
    // es un prospecto de PQC. (La lección del falso positivo `crypto` de mindalá.)
    #[test]
    fn criptomoneda_no_es_cripto() {
        let (o, _) = clasificar_texto("Bitcoin hits new high, crypto market rallies, altcoins pump");
        assert_ne!(o, "cripto", "criptomoneda no es un prospecto de criptografía");
    }
}
