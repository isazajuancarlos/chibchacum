<!--
SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
SPDX-License-Identifier: MIT
-->
# chibchacum — el buscador de clientes de Xiliux

Chibchacúm era el dios muisca **protector de los comerciantes**. Esta herramienta busca
a quién venderle el trabajo de Xiliux: sale a fuentes PÚBLICAS, encuentra entidades con
una necesidad que Xiliux resuelve, la categoriza a la oferta que encaja, extrae su canal
de negocio público, y —en rebanadas futuras— redacta la propuesta. **El envío es de Juan.**

Es el gemelo hacia AFUERA de mindalá: mindalá trae TÉCNICAS para mejorar nuestros
productos; chibchacúm trae PROSPECTOS para venderlos.

## Uso

```sh
# Clasificar una señal suelta (a qué oferta encaja).
chibchacum clasificar "We are hiring a Rust backend engineer, axum microservices"

# R1 · descubrir prospectos del hilo «Who is hiring» de HN (keyless) y encolarlos.
chibchacum buscar --fuente hn --hits 200

# R1.5 · descubrir por GitHub (carril cripto/PQC): orgs con repos de una veta.
chibchacum buscar --fuente github --topic post-quantum --lenguaje rust --oferta cripto

# R2 · el digest y la calificación (el razonador lee y decide).
chibchacum cola                        # prospectos por calificar (contacto primero)
chibchacum cola --oferta cripto        # filtra por oferta
chibchacum ver <id>                    # la ficha completa
chibchacum veredicto <id> --calza --nota "encaja PQC"   # o --descarta

# R4 · redactar el BORRADOR de propuesta (a propuestas/<id>.md). El envío es tuyo.
chibchacum redactar <id>

# R5 · lo redactado, listo para que ENVÍES TÚ.
chibchacum pendientes-envio
```

La memoria vive en `chibchacum.db` (SQLite, `--bd` para cambiar la ruta): dedup por `id`,
no se re-encola ni se re-contacta lo ya visto.

## Ofertas

`desarrollo` (Rust a medida) · `cripto` (PQC/quipu) · `herramientas` (tienda). Peritaje
diferido.

## Las tres barandillas

1. **Señales públicas de negocio, no datos personales** (se contacta el canal de la
   ENTIDAD, no un inbox personal).
2. **El motor redacta; Juan envía** — nunca auto-envía.
3. **El plano legal (Ley 1581) lo lleva Juan.**

La salida (prospectos/propuestas) es COMERCIAL → `comercial/`, no la lista de ingeniería.

## Estado

R1 HECHO (fuente HN). Diseño y roadmap: `conocimiento/proyectos/chibchacum/DISENO.md`.
Rust (D38), keyless, dep-free en lo posible, MIT.
