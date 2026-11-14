// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! chibchacúm — el buscador de clientes de Xiliux (el gemelo hacia AFUERA de mindalá).
//!
//! Sale a fuentes PÚBLICAS, encuentra entidades con una necesidad que Xiliux resuelve,
//! la categoriza a la oferta que encaja (`oferta`), extrae su canal de negocio público
//! (`contacto`) y la encola (`bd`). El razonador califica; la propuesta la envía Juan.
//! Diseño: `conocimiento/proyectos/chibchacum/DISENO.md`.

pub mod bd;
pub mod contacto;
pub mod modelo;
pub mod oferta;
pub mod propuesta;
pub mod red;
