// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! CLI de chibchacúm.
//!
//! - `clasificar <texto>`  — a qué oferta encaja una señal (hint).
//! - `buscar --fuente hn`  — R1: descubre prospectos del hilo «Who is hiring» y los encola.
//! - `cola [--oferta X]`   — el digest de prospectos pendientes de calificar.
//!
//! El motor descubre, categoriza y (en rebanadas futuras) redacta; **el envío es de
//! Juan**. Rebanadas próximas: `veredicto`, `contacto`, `redactar`, cola de envío.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use chibchacum::{bd, contacto, oferta, propuesta, red};

#[derive(Parser)]
#[command(name = "chibchacum", version, about = "El buscador de clientes de Xiliux")]
struct Cli {
    /// Ruta del archivo SQLite (la memoria de chibchacúm).
    #[arg(long, default_value = "chibchacum.db")]
    bd: String,
    #[command(subcommand)]
    orden: Orden,
}

#[derive(Subcommand)]
enum Orden {
    /// Dice a qué OFERTA de Xiliux encaja el texto de una señal (hint de primer corte).
    Clasificar {
        /// El texto de la señal.
        texto: Vec<String>,
    },
    /// [R1] Descubre prospectos de una fuente pública y los encola (dedup).
    Buscar {
        /// La fuente: `sic` (entidades sancionadas por la SIC por Ley 1581 — el encaje
        /// EXACTO del peritaje), `remoteok` (contrato/freelance de software), `hn`
        /// (Who is hiring), `hn-freelance` (seco) o `github` (repos por topic, cripto).
        #[arg(long, default_value = "sic")]
        fuente: String,
        /// (hn) Cuántos comentarios traer del hilo.
        #[arg(long, default_value = "200")]
        hits: u32,
        /// (github) Topic a buscar (ej. `post-quantum`, `cryptography`).
        #[arg(long)]
        topic: Option<String>,
        /// (github) Lenguaje (ej. `rust`).
        #[arg(long)]
        lenguaje: Option<String>,
        /// (github) Oferta que fija la intención de la búsqueda.
        #[arg(long, default_value = "cripto")]
        oferta: String,
        /// (sic) Años a barrer, separados por coma (ej. `2023,2024,2025`).
        #[arg(long, default_value = "2023,2024,2025")]
        años: String,
    },
    /// [R2] El digest: prospectos PENDIENTES de calificar, los que tienen contacto arriba.
    Cola {
        /// Filtra por oferta (desarrollo | cripto | herramientas).
        #[arg(long)]
        oferta: Option<String>,
    },
    /// [R2] Muestra la ficha completa de un prospecto (para calificarlo).
    Ver {
        /// El `id` del prospecto (de la cola).
        id: String,
    },
    /// [R2] Sienta el veredicto del razonador: `--calza` o `--descarta`, con su porqué.
    Veredicto {
        /// El `id` del prospecto.
        id: String,
        /// Encaja: candidato a propuesta.
        #[arg(long, conflicts_with = "descarta")]
        calza: bool,
        /// No encaja: sale de la cola.
        #[arg(long)]
        descarta: bool,
        /// El porqué (queda en la memoria, D5).
        #[arg(long, default_value = "")]
        nota: String,
        /// Corrige la oferta (el mapa da un hint tosco; el razonador la ajusta).
        #[arg(long)]
        oferta: Option<String>,
    },
    /// [R4] Redacta el BORRADOR de propuesta de un prospecto (a `comercial/propuestas/`).
    /// El motor redacta; **el envío es tuyo.**
    Redactar {
        /// El `id` del prospecto (idealmente ya `--calza`).
        id: String,
    },
    /// [R5] Lista los prospectos con propuesta REDACTADA, listos para que Juan envíe.
    PendientesEnvio {},
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.orden {
        Orden::Clasificar { texto } => {
            let (o, n) = oferta::clasificar_texto(&texto.join(" "));
            if o == oferta::SIN_CLASIFICAR {
                println!("sin clasificar (ninguna oferta casó).");
            } else {
                println!("oferta: {o}  (señales que casaron: {n})");
            }
        }

        Orden::Buscar { fuente, hits, topic, lenguaje, oferta, años } => {
            let pool = bd::abrir(&cli.bd).await?;
            let prospectos = match fuente.as_str() {
                "hn" => red::buscar_hn(hits).await?,
                "hn-freelance" => red::buscar_hn_freelance(hits).await?,
                "remoteok" => red::buscar_remoteok(true).await?,
                "sic" => {
                    let años: Vec<u32> = años.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if años.is_empty() {
                        bail!("`--años` vacío o inválido (ej. 2023,2024,2025).");
                    }
                    red::buscar_sic(&años).await?
                }
                "github" => {
                    let topic = topic.ok_or_else(|| {
                        anyhow::anyhow!("`--fuente github` necesita `--topic <X>`")
                    })?;
                    red::buscar_github(&topic, lenguaje.as_deref(), &oferta, 50).await?
                }
                otra => bail!("fuente desconocida: «{otra}». Hoy: hn | github."),
            };
            let hoy = fecha_hoy();
            let r = bd::encolar(&pool, &prospectos, &hoy).await?;
            let con_contacto = prospectos
                .iter()
                .filter(|p| !p.correos.is_empty() || !p.webs.is_empty())
                .count();
            println!(
                "fuente «{fuente}»: {} prospectos ({} nuevos, {} revistos) · {} con contacto público.",
                prospectos.len(),
                r.nuevos,
                r.revistos,
                con_contacto
            );
            println!("→ el digest: `chibchacum cola`. El envío lo haces tú.");
        }

        Orden::Cola { oferta } => {
            let pool = bd::abrir(&cli.bd).await?;
            let filas = bd::cola(&pool, oferta.as_deref()).await?;
            if filas.is_empty() {
                println!("cola vacía.");
            } else {
                println!("{} prospecto(s) por calificar:\n", filas.len());
                for f in filas {
                    let ct = if f.correos.is_empty() && f.webs.is_empty() {
                        "— sin contacto".to_string()
                    } else {
                        let mut v = Vec::new();
                        if !f.correos.is_empty() {
                            v.push(f.correos.replace('\n', ", "));
                        }
                        if !f.webs.is_empty() {
                            v.push(f.webs.replace('\n', ", "));
                        }
                        v.join(" · ")
                    };
                    println!("  {}  [{}] {}  ({})", f.id, f.oferta, f.entidad, f.puntaje);
                    println!("      {ct}");
                }
            }
        }

        Orden::Ver { id } => {
            let pool = bd::abrir(&cli.bd).await?;
            match bd::ver(&pool, &id).await? {
                None => bail!("no hay prospecto con id={id}"),
                Some(p) => {
                    println!("id:       {}", p.id);
                    println!("fuente:   {}", p.fuente);
                    println!("oferta:   {}  (puntaje {})", p.oferta, p.puntaje);
                    println!("entidad:  {}", p.entidad);
                    let c = |v: &[String]| if v.is_empty() { "—".into() } else { v.join(", ") };
                    println!("correos:  {}", c(&p.correos));
                    println!("webs:     {}", c(&p.webs));
                    println!("señal:    {}", p.senal);
                }
            }
        }

        Orden::Veredicto { id, calza, descarta, nota, oferta } => {
            if calza == descarta {
                bail!("elige uno: `--calza` o `--descarta`.");
            }
            let estado = if calza { "calza" } else { "descartado" };
            let pool = bd::abrir(&cli.bd).await?;
            bd::veredicto(&pool, &id, estado, &nota, oferta.as_deref()).await?;
            println!("prospecto {id}: {estado}.");
        }

        Orden::Redactar { id } => {
            let pool = bd::abrir(&cli.bd).await?;
            let p = bd::ver(&pool, &id).await?.ok_or_else(|| anyhow::anyhow!("no hay prospecto con id={id}"))?;
            let md = propuesta::redactar(&p);
            let dir = std::path::Path::new("propuestas");
            std::fs::create_dir_all(dir).ok();
            let nombre: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
            let ruta = dir.join(format!("{nombre}.md"));
            std::fs::write(&ruta, &md).with_context(|| format!("no se pudo escribir {}", ruta.display()))?;
            bd::veredicto(&pool, &id, "redactado", "borrador escrito", None).await?;
            println!("borrador → {}", ruta.display());
            println!("(revísalo y envíalo TÚ; el motor no envía).");
        }

        Orden::PendientesEnvio {} => {
            let pool = bd::abrir(&cli.bd).await?;
            let ps = bd::listar_por_estado(&pool, "redactado").await?;
            if ps.is_empty() {
                println!("nada redactado. Califica (`veredicto --calza`) y redacta (`redactar <id>`).");
            } else {
                println!("{} propuesta(s) REDACTADA(s), listas para que envíes TÚ:\n", ps.len());
                for p in ps {
                    let canal = contacto::canal_de_negocio(&p.correos, &p.webs)
                        .unwrap_or_else(|| "— resolver dominio a mano".to_string());
                    println!("  [{}] {}  →  {}", p.oferta, p.entidad, canal);
                }
            }
        }
    }
    Ok(())
}

/// Fecha de hoy en ISO-8601 (UTC, solo día). Sin dependencia de `chrono`: la deriva del
/// reloj del sistema. Solo se usa como marca de `primero_visto`.
fn fecha_hoy() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // días desde epoch → YYYY-MM-DD por conversión civil (algoritmo de Howard Hinnant).
    let dias = (s / 86_400) as i64;
    let (y, m, d) = civil_desde_dias(dias);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Convierte días-desde-epoch a (año, mes, día). Algoritmo de Hinnant (dominio público).
fn civil_desde_dias(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
