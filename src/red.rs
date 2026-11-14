// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! La ÚNICA parte con red. Fuente R1: Hacker News «Who is hiring» vía la API de Algolia
//! (keyless, sin rate limit). Dos pasos: (1) hallar el hilo mensual del autor
//! `whoishiring`; (2) traer sus comentarios y pasarlos por la costura pura
//! `modelo::prospecto_de_comentario`. El parseo va aparte para probarlo sin red (D33).

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::modelo::{Prospecto, es_seeking_freelancer, limpiar_html, prospecto_de_comentario};
use crate::{contacto, oferta};

const ALGOLIA: &str = "https://hn.algolia.com/api/v1/search_by_date";
const UA: &str = "chibchacum/0.1 (+https://xiliux.com; prospección de negocio)";

fn cliente() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .context("no se pudo construir el cliente HTTP")
}

#[derive(Deserialize)]
struct RespStory {
    hits: Vec<HitStory>,
}
#[derive(Deserialize)]
struct HitStory {
    #[serde(rename = "objectID")]
    object_id: String,
    title: Option<String>,
}

/// Todos los `(objectID, título)` de las historias del JSON.
///
/// # Errores
/// Falla si el JSON no tiene la forma esperada (D30).
pub fn parsear_stories(json: &str) -> Result<Vec<(String, String)>> {
    let r: RespStory = serde_json::from_str(json).context("JSON de historia inesperado")?;
    Ok(r.hits
        .into_iter()
        .map(|h| (h.object_id, h.title.unwrap_or_default()))
        .collect())
}

/// El autor `whoishiring` publica TRES hilos/mes: «Who is hiring?» (EMPRESAS),
/// «Who wants to be hired?» (PERSONAS) y «Freelancer? Seeking freelancer?». Estos
/// predicados eligen el hilo correcto por título; los de PERSONAS se excluyen a nivel
/// de comentario (barandilla #1).
fn es_hilo_hiring(titulo_bajo: &str) -> bool {
    titulo_bajo.contains("who is hiring") && !titulo_bajo.contains("wants to be hired")
}
fn es_hilo_freelance(titulo_bajo: &str) -> bool {
    titulo_bajo.contains("freelancer")
}

/// El primer story cuyo título casa `quiere`, o `None`.
pub fn elegir_story(json: &str, quiere: fn(&str) -> bool) -> Result<Option<(String, String)>> {
    Ok(parsear_stories(json)?
        .into_iter()
        .find(|(_, t)| quiere(&t.to_lowercase())))
}

#[derive(Deserialize)]
struct RespCom {
    hits: Vec<HitCom>,
}
#[derive(Deserialize)]
struct HitCom {
    #[serde(rename = "objectID")]
    object_id: String,
    comment_text: Option<String>,
    created_at: Option<String>,
}

/// Convierte los comentarios de un hilo en prospectos (los que sirven; el filtro está
/// en `prospecto_de_comentario`). Es la COSTURA que recibe el TEXTO real de la API, la
/// misma que se prueba con un JSON sintético (D33).
///
/// # Errores
/// Falla si el JSON no tiene la forma esperada (D30).
pub fn parsear_comentarios(json: &str, fuente: &str, solo_freelance: bool) -> Result<Vec<Prospecto>> {
    let r: RespCom = serde_json::from_str(json).context("JSON de comentarios inesperado")?;
    Ok(r
        .hits
        .into_iter()
        .filter_map(|h| {
            let texto = h.comment_text?;
            // Freelance: solo la cara SEEKING FREELANCER (empresas); SEEKING WORK son
            // personas → se excluyen (barandilla #1).
            if solo_freelance && !es_seeking_freelancer(&texto) {
                return None;
            }
            prospecto_de_comentario(
                &h.object_id,
                &texto,
                h.created_at.as_deref().unwrap_or(""),
                fuente,
            )
        })
        .collect())
}

/// Trae prospectos de un hilo mensual de `whoishiring`: elige el hilo por `quiere`
/// (título), trae sus comentarios y los pasa por la costura (filtrando a SEEKING
/// FREELANCER si `solo_freelance`). Helper de `buscar_hn` y `buscar_hn_freelance`.
async fn buscar_hilo(
    query: &str,
    quiere: fn(&str) -> bool,
    fuente: &str,
    solo_freelance: bool,
    hits: u32,
) -> Result<Vec<Prospecto>> {
    let cli = cliente()?;
    let resp = cli
        .get(ALGOLIA)
        .query(&[
            ("tags", "story,author_whoishiring"),
            ("query", query),
            ("hitsPerPage", "20"), // varios meses: se filtra por título con `quiere`, se toma el 1º (más reciente).
        ])
        .send()
        .await
        .context("falló la petición del hilo a HN")?
        .error_for_status()
        .context("HN respondió con error al buscar el hilo")?;
    let cuerpo = resp.text().await.context("cuerpo del hilo ilegible")?;
    let (story_id, _t) =
        elegir_story(&cuerpo, quiere)?.context("no se encontró el hilo pedido")?;

    let resp = cli
        .get(ALGOLIA)
        .query(&[
            ("tags", format!("comment,story_{story_id}").as_str()),
            ("hitsPerPage", hits.to_string().as_str()),
        ])
        .send()
        .await
        .context("falló la petición de comentarios a HN")?
        .error_for_status()
        .context("HN respondió con error al traer comentarios")?;
    let cuerpo = resp.text().await.context("cuerpo de comentarios ilegible")?;
    parsear_comentarios(&cuerpo, fuente, solo_freelance)
}

/// Prospectos del hilo «Who is hiring» (empresas contratando; incluye contrato).
pub async fn buscar_hn(hits: u32) -> Result<Vec<Prospecto>> {
    buscar_hilo("Who is hiring", es_hilo_hiring, "hn-hiring", false, hits).await
}

/// Prospectos del hilo «Freelancer? Seeking freelancer?», solo la cara SEEKING
/// FREELANCER (EMPRESAS que buscan freelancer) — el objetivo de Xiliux.
pub async fn buscar_hn_freelance(hits: u32) -> Result<Vec<Prospecto>> {
    buscar_hilo("Freelancer seeking freelancer", es_hilo_freelance, "hn-freelance", true, hits).await
}

// --- Fuente R1.5: GitHub Search API (keyless, 10 req/min, UA obligatorio) ----
// Señal INDIRECTA (más débil que un aviso de contratación): una org con repos activos
// de cripto/Rust TIENE cripto/Rust que auditar, migrar o extender → candidata a los
// servicios de Xiliux. El razonador juzga si de verdad encaja.

const GH_API: &str = "https://api.github.com/search/repositories";

#[derive(Deserialize)]
struct RespGh {
    items: Vec<RepoGh>,
}
#[derive(Deserialize)]
struct RepoGh {
    full_name: String,
    description: Option<String>,
    homepage: Option<String>,
    owner: OwnerGh,
    pushed_at: Option<String>,
}
#[derive(Deserialize)]
struct OwnerGh {
    login: String,
    html_url: String,
}

/// Convierte la respuesta de la GitHub Search API en prospectos. `oferta` es la que
/// fijó la intención de la búsqueda (topic post-quantum → cripto); el puntaje sale de
/// clasificar la descripción (confianza). El canal público es el `homepage` de la org
/// (si es http) y su perfil; el correo no viene en esta API (lo resuelve R3).
///
/// # Errores
/// Falla si el JSON no tiene la forma esperada (D30).
pub fn parsear_github(json: &str, oferta: &str) -> Result<Vec<Prospecto>> {
    let r: RespGh = serde_json::from_str(json).context("JSON de GitHub inesperado")?;
    Ok(r
        .items
        .into_iter()
        .map(|repo| {
            let desc = repo.description.unwrap_or_default();
            let (_, puntaje) = oferta::clasificar_texto(&format!("{} {desc}", repo.full_name));
            let mut webs = Vec::new();
            if let Some(h) = repo.homepage.filter(|h| h.starts_with("http")) {
                webs.push(h);
            }
            webs.push(repo.owner.html_url);
            Prospecto {
                id: format!("github-{}", repo.full_name),
                fuente: "github".to_string(),
                oferta: oferta.to_string(),
                puntaje: puntaje as i64,
                entidad: repo.owner.login,
                senal: format!("{}: {desc}", repo.full_name)
                    .chars()
                    .take(280)
                    .collect(),
                correos: Vec::new(),
                webs,
                fecha: repo.pushed_at.unwrap_or_default(),
            }
        })
        .collect())
}

/// Busca repos por `topic` (y opcional `lenguaje`) y los trae como prospectos de la
/// `oferta` dada. Keyless: 10 req/min, User-Agent obligatorio (ya en `cliente`).
pub async fn buscar_github(
    topic: &str,
    lenguaje: Option<&str>,
    oferta: &str,
    por_pagina: u32,
) -> Result<Vec<Prospecto>> {
    let mut q = format!("topic:{topic}");
    if let Some(l) = lenguaje {
        q.push_str(&format!(" language:{l}"));
    }
    let resp = cliente()?
        .get(GH_API)
        .query(&[
            ("q", q.as_str()),
            ("sort", "updated"),
            ("order", "desc"),
            ("per_page", por_pagina.to_string().as_str()),
        ])
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .context("falló la petición a GitHub")?
        .error_for_status()
        .context("GitHub respondió con error (¿rate-limit de 10 req/min sin token?)")?;
    let cuerpo = resp.text().await.context("cuerpo de GitHub ilegible")?;
    parsear_github(&cuerpo, oferta)
}

// --- Fuente: RemoteOK (JSON público, keyless) -------------------------------
// La veta freelance VIVA: su tipo `contract` = empresa que quiere un freelancer.
// ToS de RemoteOK: exige atribución si se REPUBLICA su listado; aquí solo se LEE para
// prospección interna, no se republica. Se contacta el canal de negocio de la empresa.

/// ¿La oferta es de SOFTWARE? (para no traer contratos de ventas/diseño/HR). Señales
/// amplias de ingeniería; el mapa de `oferta` afina después a qué carril de Xiliux.
fn es_software(texto_bajo: &str) -> bool {
    [
        "developer", "engineer", "engineering", "development", "software", "backend",
        "frontend", "full-stack", "fullstack", "full stack", "programming", "devops",
        "sre", "rust", "python", "golang", " go ", "typescript", "javascript", "node",
        "react", "kubernetes", "api", "desarrollador", "programador",
    ]
    .iter()
    .any(|k| texto_bajo.contains(k))
}

/// Parsea el array de RemoteOK. El PRIMER elemento es texto legal (sin `company`) → se
/// salta. Con `solo_contrato`, retiene solo las ofertas de contrato/freelance. Robusto a
/// campos ausentes: se lee por clave, no con struct rígido (la API es heterogénea).
///
/// # Errores
/// Falla si el cuerpo no es un array JSON (D30).
pub fn parsear_remoteok(json: &str, solo_contrato: bool) -> Result<Vec<Prospecto>> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(json).context("JSON de RemoteOK inesperado (¿no es un array?)")?;
    let mut out = Vec::new();
    for v in arr {
        let campo = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
        let company = campo("company");
        if company.is_empty() {
            continue; // el elemento legal, o basura sin empresa
        }
        let position = campo("position");
        let tags: Vec<String> = v
            .get("tags")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_lowercase)).collect())
            .unwrap_or_default();
        let desc = campo("description");
        let texto = format!("{position} {} {desc}", tags.join(" "));
        let pos_tags = format!("{position} {}", tags.join(" ")).to_lowercase();
        if solo_contrato {
            // CONTRATO amplio (a veces solo se dice en la descripción)…
            let bajo = texto.to_lowercase();
            let es_contrato = bajo.contains("freelance") || bajo.contains("contract");
            // …pero SOFTWARE solo en puesto+tags: la descripción de un puesto de
            // ventas/HR trae palabras técnicas sueltas y colaría (medido).
            if !(es_contrato && es_software(&pos_tags)) {
                continue;
            }
        }
        let (oferta, puntaje) = oferta::clasificar_texto(&texto);
        let c = contacto::extraer_contacto(desc);
        let mut webs = c.webs;
        for k in ["apply_url", "url"] {
            let u = campo(k);
            if u.starts_with("http") && !webs.iter().any(|w| w == u) {
                webs.push(u.to_string());
            }
        }
        webs.truncate(4); // descripciones ricas traen decenas de enlaces; basta con unos
        if puntaje == 0 && c.correos.is_empty() && webs.is_empty() {
            continue; // sin oferta ni canal, no es prospecto (D20)
        }
        let id = v
            .get("id")
            .and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_i64().map(|n| n.to_string())))
            .unwrap_or_else(|| company.to_string());
        out.push(Prospecto {
            id: format!("remoteok-{id}"),
            fuente: "remoteok".to_string(),
            oferta: oferta.to_string(),
            puntaje: puntaje as i64,
            entidad: company.to_string(),
            senal: format!("{position} — {desc}").chars().take(280).collect(),
            correos: c.correos,
            webs,
            fecha: campo("date").to_string(),
        });
    }
    Ok(out)
}

/// Trae prospectos de RemoteOK. Con `solo_contrato`, solo ofertas de contrato/freelance.
pub async fn buscar_remoteok(solo_contrato: bool) -> Result<Vec<Prospecto>> {
    let cuerpo = cliente()?
        .get("https://remoteok.com/api")
        .send()
        .await
        .context("falló la petición a RemoteOK")?
        .error_for_status()
        .context("RemoteOK respondió con error")?
        .text()
        .await
        .context("cuerpo de RemoteOK ilegible")?;
    parsear_remoteok(&cuerpo, solo_contrato)
}

// --- Fuente: SIC (sanciones de protección de datos, Ley 1581) ----------------
// El encaje EXACTO del carril peritaje: una entidad sancionada por la SIC necesita YA
// asesoría jurídico-técnica en protección de datos. Fuente: las tablas HTML públicas
// `sic.gov.co/sanciones-proteccion-datos-personales-{AÑO}` (keyless, sin JS).
//
// TRAMPA MEDIDA (D30): `www.sic.gov.co` sirve una cadena TLS INCOMPLETA (solo el leaf,
// falta el intermedio GlobalSign RSA OV SSL CA 2018). Un cliente estándar falla en
// silencio → cero mudo. Se resuelve empotrando el intermedio y confiándolo como raíz —
// la verificación sigue ACTIVA (no se usa danger_accept_invalid_certs).

const CA_GLOBALSIGN: &str = include_str!("../certs/globalsign-rsa-ov-ssl-ca-2018.pem");

fn cliente_sic() -> Result<reqwest::Client> {
    let cert = reqwest::Certificate::from_pem(CA_GLOBALSIGN.as_bytes())
        .context("el certificado intermedio empotrado es inválido")?;
    reqwest::Client::builder()
        .user_agent(UA)
        .add_root_certificate(cert) // completa la cadena; verificación sigue activa
        .build()
        .context("no se pudo construir el cliente de la SIC")
}

/// Interiores de `<TAG …>…</TAG>` en orden (tolerante a atributos en la apertura).
fn bloques_html<'a>(html: &'a str, tag: &str) -> Vec<&'a str> {
    let abre = format!("<{tag}");
    let cierra = format!("</{tag}>");
    let mut out = Vec::new();
    let mut resto = html;
    while let Some(i) = resto.find(&abre) {
        let tras = &resto[i + abre.len()..];
        let Some(gt) = tras.find('>') else { break };
        let cuerpo = &tras[gt + 1..];
        match cuerpo.find(&cierra) {
            Some(j) => {
                out.push(&cuerpo[..j]);
                resto = &cuerpo[j + cierra.len()..];
            }
            None => break,
        }
    }
    out
}

/// Marcas de que un nombre es una ENTIDAD (no una persona natural). Barandilla #1:
/// solo entidades. Se yerra hacia excluir (D21): un nombre sin marca societaria se salta.
const MARCAS_ENTIDAD: &[&str] = &[
    "s.a.s", "s.a.", " sa", "sas", "ltda", "e.s.p", "esp", "eps", "s. en c", "& cia",
    "cia.", "compañ", "compani", "fundaci", "cooperativa", "asociaci", "corporaci",
    "banco", "seguros", "s.a ",
];

fn es_entidad(nombre: &str) -> bool {
    let n = format!(" {} ", nombre.to_lowercase());
    MARCAS_ENTIDAD.iter().any(|m| n.contains(m))
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(64)
        .collect()
}

/// Parsea una página de sanciones de la SIC: cada `<tr>` con ≥2 `<td>` cuya 1ª celda es
/// una ENTIDAD → un prospecto de la oferta `peritaje`. El TÍTULO (qué violó) va en la
/// señal — oro para la propuesta. PURA: se prueba con una tabla sintética (D33).
#[must_use]
pub fn parsear_sic(html: &str) -> Vec<Prospecto> {
    let mut out = Vec::new();
    let mut vistos = std::collections::BTreeSet::new();
    for fila in bloques_html(html, "tr") {
        let celdas: Vec<String> = bloques_html(fila, "td")
            .iter()
            .map(|c| limpiar_html(c).split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        if celdas.len() < 2 {
            continue; // encabezado o fila vacía
        }
        let entidad = celdas[0].clone();
        if entidad.is_empty() || !es_entidad(&entidad) {
            continue;
        }
        let fecha = celdas.get(1).cloned().unwrap_or_default();
        let senal: String = celdas[1..].join(" · ").chars().take(280).collect();
        let (_, puntaje) = oferta::clasificar_texto(&celdas.join(" "));
        let id = format!("sic-{}", slug(&format!("{entidad}-{fecha}")));
        if !vistos.insert(id.clone()) {
            continue;
        }
        out.push(Prospecto {
            id,
            fuente: "sic".to_string(),
            oferta: "peritaje".to_string(), // por construcción: toda sanción SIC es de datos
            puntaje: puntaje as i64,
            entidad,
            senal,
            correos: Vec::new(),
            webs: Vec::new(), // la tabla no da contacto → canal se resuelve a mano (R3)
            fecha,
        });
    }
    out
}

/// Trae las entidades sancionadas por la SIC en los `años` dados.
pub async fn buscar_sic(años: &[u32]) -> Result<Vec<Prospecto>> {
    let cli = cliente_sic()?;
    let mut out = Vec::new();
    for año in años {
        let url = format!("https://www.sic.gov.co/sanciones-proteccion-datos-personales-{año}");
        let html = cli
            .get(&url)
            .send()
            .await
            .with_context(|| format!("falló GET {url} (¿cadena TLS de la SIC?)"))?
            .error_for_status()
            .with_context(|| format!("la SIC respondió con error para {año}"))?
            .text()
            .await
            .with_context(|| format!("cuerpo de la SIC {año} ilegible"))?;
        out.extend(parsear_sic(&html));
    }
    Ok(out)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sic_parsea_entidad_y_salta_persona() {
        let html = r#"<table>
          <tr><th>SANCIONADO</th><th>FECHA</th><th>TÍTULO</th></tr>
          <tr><td>MICROBANK COLOMBIA S.A.S.</td><td>2024-05-10</td>
              <td>Violación al deber de seguridad de datos personales (Ley 1581)</td>
              <td>Res. 12345</td><td>EN FIRME</td></tr>
          <tr><td>Juan Pérez</td><td>2024-06-01</td><td>algo</td><td>Res. 999</td><td>EN FIRME</td></tr>
        </table>"#;
        let ps = parsear_sic(html);
        assert_eq!(ps.len(), 1, "solo la entidad, no la persona: {ps:?}");
        assert_eq!(ps[0].entidad, "MICROBANK COLOMBIA S.A.S.");
        assert_eq!(ps[0].oferta, "peritaje");
        assert!(ps[0].puntaje >= 1, "el título de datos personales clasifica peritaje");
        assert!(ps[0].id.starts_with("sic-"));
        assert!(ps[0].senal.contains("Ley 1581"), "el TÍTULO va en la señal");
    }

    #[test]
    fn remoteok_salta_legal_y_filtra_contrato() {
        let json = r#"[
            {"legal":"See remoteok.com/api for terms"},
            {"id":"1","company":"Acme","position":"Rust Engineer (Contract)","tags":["rust","contract"],
             "description":"axum backend, apply hi@acme.io","apply_url":"https://acme.io/apply","date":"2026-09-20"},
            {"id":"2","company":"BigCo","position":"Full-time Backend","tags":["python","full-time"],
             "description":"perm role team@bigco.com","apply_url":"https://bigco.com","date":"2026-09-20"},
            {"id":"3","company":"AdCo","position":"Freelance Creative Director","tags":["design","contract"],
             "description":"branding, jobs@adco.com","apply_url":"https://adco.com","date":"2026-09-20"}
        ]"#;
        let ps = parsear_remoteok(json, true).unwrap();
        // solo Acme: legal saltado, BigCo fuera (full-time), AdCo fuera (contrato NO-software).
        assert_eq!(ps.len(), 1, "solo contrato+software: {ps:?}");
        assert_eq!(ps[0].entidad, "Acme");
        assert_eq!(ps[0].oferta, "desarrollo");
        assert_eq!(ps[0].correos, vec!["hi@acme.io"]);
        assert!(ps[0].id.starts_with("remoteok-"));
        // sin filtro, entran los tres.
        assert_eq!(parsear_remoteok(json, false).unwrap().len(), 3);
    }

    #[test]
    fn parsear_github_arma_prospecto() {
        let json = r#"{"items":[{"full_name":"acme/pqc-lib","description":"post-quantum crypto in rust",
            "homepage":"https://acme.io","owner":{"login":"acme","html_url":"https://github.com/acme"},
            "pushed_at":"2026-09-20"}]}"#;
        let ps = parsear_github(json, "cripto").unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].oferta, "cripto");
        assert_eq!(ps[0].entidad, "acme");
        assert!(ps[0].webs.iter().any(|w| w.contains("acme.io")));
        assert!(ps[0].id.starts_with("github-"));
    }

    // BARANDILLA #1 (D33): elige «Who is hiring» (empresas) y RECHAZA «Who wants to be
    // hired» (personas). El caso rojo —el hilo de personas primero— NO debe colarse.
    #[test]
    fn elegir_story_hiring_no_personas() {
        let json = r#"{"hits":[
            {"objectID":"111","title":"Ask HN: Who wants to be hired? (September 2026)"},
            {"objectID":"999","title":"Ask HN: Who is hiring? (September 2026)"}
        ]}"#;
        let (id, _) = elegir_story(json, es_hilo_hiring).unwrap().unwrap();
        assert_eq!(id, "999", "toma el de empresas, no el de personas");
        // Y el hilo freelance se elige por su título.
        let jf = r#"{"hits":[{"objectID":"777","title":"Ask HN: Freelancer? Seeking freelancer? (Sep 2026)"}]}"#;
        assert_eq!(elegir_story(jf, es_hilo_freelance).unwrap().unwrap().0, "777");
    }

    #[test]
    fn elegir_story_solo_personas_da_none() {
        let json = r#"{"hits":[{"objectID":"111","title":"Ask HN: Who wants to be hired?"}]}"#;
        assert!(elegir_story(json, es_hilo_hiring).unwrap().is_none());
    }

    // La costura recibe el JSON real de Algolia y produce prospectos por la vía real
    // (D33): un comentario de Rust con contacto entra; uno de charla NO.
    #[test]
    fn parsear_comentarios_por_la_via_real() {
        let json = r#"{"hits":[
            {"objectID":"1","comment_text":"Beta Labs | Rust backend engineer | remote | jobs@beta.dev","created_at":"2026-09-02"},
            {"objectID":"2","comment_text":"Great thread, good luck everyone!","created_at":"2026-09-02"}
        ]}"#;
        let ps = parsear_comentarios(json, "hn-hiring", false).unwrap();
        assert_eq!(ps.len(), 1, "solo el útil: {ps:?}");
        assert_eq!(ps[0].id, "1");
        assert_eq!(ps[0].fuente, "hn-hiring");
        assert_eq!(ps[0].correos, vec!["jobs@beta.dev"]);
    }

    // Freelance (D33): con solo_freelance, entra SEEKING FREELANCER (empresa) y se
    // excluye SEEKING WORK (persona).
    #[test]
    fn parsear_comentarios_freelance_solo_empresas() {
        let json = r#"{"hits":[
            {"objectID":"a","comment_text":"SEEKING FREELANCER | Acme | Rust contract | jobs@acme.io","created_at":"2026-09-02"},
            {"objectID":"b","comment_text":"SEEKING WORK | Rust dev, 5y | me@gmail.com","created_at":"2026-09-02"}
        ]}"#;
        let ps = parsear_comentarios(json, "hn-freelance", true).unwrap();
        assert_eq!(ps.len(), 1, "solo la empresa: {ps:?}");
        assert_eq!(ps[0].id, "a");
        assert_eq!(ps[0].fuente, "hn-freelance");
    }

    #[test]
    fn json_malformado_falla_ruidoso() {
        assert!(parsear_comentarios("no soy json", "hn-hiring", false).is_err());
    }
}
