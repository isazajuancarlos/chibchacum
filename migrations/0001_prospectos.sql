-- SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
-- SPDX-License-Identifier: MIT
--
-- Los PROSPECTOS: una entidad con una necesidad que Xiliux resuelve, y por dónde
-- contactarla. La `id` es la clave de dedup (para HN, el objectID del comentario), así
-- una re-corrida no vuelve a encolar lo ya visto. `estado` NULL = pendiente de
-- calificar (ausencia, no relleno; D20). Correos y webs van serializados con '\n'.

CREATE TABLE IF NOT EXISTS prospectos (
    id            TEXT PRIMARY KEY,       -- clave de dedup (objectID de HN, …)
    fuente        TEXT NOT NULL,          -- 'hn-hiring' | 'github' | …
    oferta        TEXT NOT NULL,          -- desarrollo | cripto | herramientas | sin_clasificar
    puntaje       INTEGER NOT NULL,       -- señales de la oferta que casaron
    entidad       TEXT NOT NULL,          -- nombre de la empresa/org (best-effort)
    senal         TEXT NOT NULL,          -- el fragmento de la señal (recorte)
    correos       TEXT NOT NULL,          -- canal público, uno por línea ('' si nada)
    webs          TEXT NOT NULL,          -- canal público, uno por línea ('' si nada)
    fecha         TEXT NOT NULL,          -- fecha de la señal
    primero_visto TEXT NOT NULL,          -- ISO-8601, primera corrida que lo trajo
    estado        TEXT,                   -- NULL = pendiente de calificar (D20)
    nota          TEXT                    -- porqué del veredicto (D5)
);

CREATE INDEX IF NOT EXISTS idx_prospectos_pendiente ON prospectos (estado) WHERE estado IS NULL;
CREATE INDEX IF NOT EXISTS idx_prospectos_oferta    ON prospectos (oferta);
