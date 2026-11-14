// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! La memoria de chibchacúm (SQLite): dedup por `id` para no re-encolar ni re-contactar
//! lo ya visto, y el digest de lo pendiente de calificar. Mismo patrón que mindalá.

use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::modelo::Prospecto;

static MIGRACIONES: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Abre (o crea) la BD y aplica las migraciones.
pub async fn abrir(ruta: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(ruta)
        .with_context(|| format!("ruta de BD inválida: {ruta}"))?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .context("no se pudo abrir la BD")?;
    MIGRACIONES
        .run(&pool)
        .await
        .context("fallaron las migraciones")?;
    Ok(pool)
}

/// Cuántos prospectos nuevos se insertaron y cuántos ya estaban (dedup).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resumen {
    pub nuevos: usize,
    pub revistos: usize,
}

/// Encola los prospectos, deduplicando por `id` (`INSERT OR IGNORE`). No pisa el
/// `estado` de uno ya calificado: un prospecto visto no se re-abre.
pub async fn encolar(pool: &SqlitePool, ps: &[Prospecto], hoy: &str) -> Result<Resumen> {
    let mut r = Resumen::default();
    for p in ps {
        let filas = sqlx::query(
            "INSERT OR IGNORE INTO prospectos \
             (id, fuente, oferta, puntaje, entidad, senal, correos, webs, fecha, primero_visto) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        )
        .bind(&p.id)
        .bind(&p.fuente)
        .bind(&p.oferta)
        .bind(p.puntaje)
        .bind(&p.entidad)
        .bind(&p.senal)
        .bind(p.correos.join("\n"))
        .bind(p.webs.join("\n"))
        .bind(&p.fecha)
        .bind(hoy)
        .execute(pool)
        .await?
        .rows_affected();
        if filas == 1 {
            r.nuevos += 1;
        } else {
            r.revistos += 1;
        }
    }
    Ok(r)
}

/// Una fila del digest de la cola.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaCola {
    pub id: String,
    pub oferta: String,
    pub puntaje: i64,
    pub entidad: String,
    pub correos: String,
    pub webs: String,
}

/// El digest: prospectos PENDIENTES de calificar (`estado IS NULL`). Con contacto
/// primero (los accionables), luego por puntaje. Filtra por oferta si se pide.
pub async fn cola(pool: &SqlitePool, oferta: Option<&str>) -> Result<Vec<FilaCola>> {
    // SQL en literales `const` (no `format!`): sqlx 0.9 rechaza el SQL dinámico.
    const TODOS: &str = "SELECT id, oferta, puntaje, entidad, correos, webs FROM prospectos \
        WHERE estado IS NULL \
        ORDER BY (correos != '' OR webs != '') DESC, puntaje DESC, entidad ASC";
    const POR_OFERTA: &str = "SELECT id, oferta, puntaje, entidad, correos, webs FROM prospectos \
        WHERE estado IS NULL AND oferta = ?1 \
        ORDER BY (correos != '' OR webs != '') DESC, puntaje DESC, entidad ASC";
    let rows = match oferta {
        Some(o) => sqlx::query(POR_OFERTA).bind(o).fetch_all(pool).await?,
        None => sqlx::query(TODOS).fetch_all(pool).await?,
    };
    Ok(rows
        .into_iter()
        .map(|row| FilaCola {
            id: row.get("id"),
            oferta: row.get("oferta"),
            puntaje: row.get("puntaje"),
            entidad: row.get("entidad"),
            correos: row.get("correos"),
            webs: row.get("webs"),
        })
        .collect())
}

/// Reconstruye un `Prospecto` desde una fila (correos/webs vienen unidos por '\n').
fn fila_a_prospecto(row: &sqlx::sqlite::SqliteRow) -> Prospecto {
    let dividir = |s: String| {
        s.lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    Prospecto {
        id: row.get("id"),
        fuente: row.get("fuente"),
        oferta: row.get("oferta"),
        puntaje: row.get("puntaje"),
        entidad: row.get("entidad"),
        senal: row.get("senal"),
        correos: dividir(row.get("correos")),
        webs: dividir(row.get("webs")),
        fecha: row.get("fecha"),
    }
}

/// Un prospecto por su `id`, o `None` si no está.
pub async fn ver(pool: &SqlitePool, id: &str) -> Result<Option<Prospecto>> {
    let row = sqlx::query(
        "SELECT id, fuente, oferta, puntaje, entidad, senal, correos, webs, fecha \
         FROM prospectos WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(fila_a_prospecto))
}

/// Sienta el veredicto del razonador sobre un prospecto (`calza`/`descartado`/
/// `redactado`), con su porqué. Si `oferta` es `Some`, CORRIGE la oferta (el mapa da un
/// hint tosco; el razonador la ajusta al calificar). Falla si el `id` no existe (D30).
pub async fn veredicto(
    pool: &SqlitePool,
    id: &str,
    estado: &str,
    nota: &str,
    oferta: Option<&str>,
) -> Result<()> {
    let filas = match oferta {
        Some(o) => {
            sqlx::query("UPDATE prospectos SET estado = ?2, nota = ?3, oferta = ?4 WHERE id = ?1")
                .bind(id)
                .bind(estado)
                .bind(nota)
                .bind(o)
                .execute(pool)
                .await?
                .rows_affected()
        }
        None => sqlx::query("UPDATE prospectos SET estado = ?2, nota = ?3 WHERE id = ?1")
            .bind(id)
            .bind(estado)
            .bind(nota)
            .execute(pool)
            .await?
            .rows_affected(),
    };
    anyhow::ensure!(filas == 1, "no hay prospecto con id={id}");
    Ok(())
}

/// Los prospectos en un `estado` dado (para R5: los `redactado`, listos para que Juan
/// envíe). Contacto primero.
pub async fn listar_por_estado(pool: &SqlitePool, estado: &str) -> Result<Vec<Prospecto>> {
    let rows = sqlx::query(
        "SELECT id, fuente, oferta, puntaje, entidad, senal, correos, webs, fecha \
         FROM prospectos WHERE estado = ?1 \
         ORDER BY (correos != '' OR webs != '') DESC, puntaje DESC",
    )
    .bind(estado)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(fila_a_prospecto).collect())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn bd_temporal() -> SqlitePool {
        abrir("sqlite::memory:").await.unwrap()
    }

    fn prospecto(id: &str, oferta: &str, correo: &str) -> Prospecto {
        Prospecto {
            id: id.into(),
            fuente: "hn-hiring".into(),
            oferta: oferta.into(),
            puntaje: 2,
            entidad: "Acme".into(),
            senal: "hiring rust".into(),
            correos: if correo.is_empty() { vec![] } else { vec![correo.into()] },
            webs: vec![],
            fecha: "2026-09-01".into(),
        }
    }

    #[tokio::test]
    async fn encolar_deduplica_por_id() {
        let pool = bd_temporal().await;
        let ps = [prospecto("1", "desarrollo", "a@x.com"), prospecto("2", "cripto", "b@y.com")];
        let r1 = encolar(&pool, &ps, "2026-09-28").await.unwrap();
        assert_eq!(r1.nuevos, 2);
        // re-encolar: 0 nuevos (dedup por id)
        let r2 = encolar(&pool, &ps, "2026-09-29").await.unwrap();
        assert_eq!(r2, Resumen { nuevos: 0, revistos: 2 });
    }

    #[tokio::test]
    async fn cola_prioriza_los_con_contacto() {
        let pool = bd_temporal().await;
        let ps = [
            prospecto("sin", "desarrollo", ""),   // sin contacto
            prospecto("con", "desarrollo", "hi@z.com"), // con contacto
        ];
        encolar(&pool, &ps, "2026-09-28").await.unwrap();
        let d = cola(&pool, None).await.unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].id, "con", "el que tiene contacto va primero: {d:?}");
        // filtro por oferta
        assert_eq!(cola(&pool, Some("cripto")).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn veredicto_saca_de_cola_y_ver_lo_trae() {
        let pool = bd_temporal().await;
        encolar(&pool, &[prospecto("x", "cripto", "a@b.com")], "2026-09-28").await.unwrap();
        // ver reconstruye el prospecto (correos split).
        let p = ver(&pool, "x").await.unwrap().unwrap();
        assert_eq!(p.correos, vec!["a@b.com"]);
        // veredicto calza → sale de la cola, entra en listar_por_estado.
        veredicto(&pool, "x", "calza", "encaja PQC", Some("cripto")).await.unwrap();
        assert!(cola(&pool, None).await.unwrap().is_empty(), "calificado sale de la cola");
        assert_eq!(listar_por_estado(&pool, "calza").await.unwrap().len(), 1);
        // la oferta se corrigió a cripto.
        assert_eq!(ver(&pool, "x").await.unwrap().unwrap().oferta, "cripto");
        // caso rojo (D30): veredicto sobre id inexistente falla.
        assert!(veredicto(&pool, "no-existe", "calza", "", None).await.is_err());
    }
}
