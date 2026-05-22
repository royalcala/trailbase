# Guía: IP de cliente + GeoIP (MaxMind) en TrailBase

Esta guía explica:

- Cómo obtiene TrailBase la IP del cliente.
- Cómo habilitar enriquecimiento GeoIP con MaxMindDB.
- Qué alcance tiene realmente (qué sí y qué no hace).
- Cómo usar las funciones geoip en SQL (vistas, logs, endpoints).
- Cómo leer la IP del request dentro de un endpoint WASM (Rust / TypeScript).

## 1) Qué hace TrailBase por defecto

TrailBase captura la IP del cliente en cada request y la guarda en logs.

Fuentes de IP (prioridad):

1. `X-Forwarded-For` (rightmost, según la librería usada)
2. `X-Real-IP`
3. Otros headers comunes de edge/proxy (`Fly`, `CF`, `CloudFront`, etc.)
4. IP del socket (`remote_addr`) si no hay headers

Esa IP termina en `_logs.client_ip`.

## 2) MaxMindDB: para qué sirve

Si cargas una base MaxMind (`.mmdb`), TrailBase puede convertir IP a:

- País (`geoip_country`)
- Ciudad (`geoip_city_name` / `geoip_city_json`)

El Admin de logs usa esto para mostrar:

- `client_geoip_cc` (country code)
- `client_geoip_city` (city JSON)

Importante: si la DB no carga, TrailBase sigue funcionando, solo sin enriquecimiento GeoIP.

## 3) Configuración

### Opción A: pasando ruta explícita

```bash
trail run \
  --data-dir ./traildepot \
  --geoip-db-path ./traildepot/GeoLite2-City.mmdb
```

### Opción B: ruta por defecto

Si no pasas `--geoip-db-path`, intenta cargar:

- `<data_dir>/GeoLite2-Country.mmdb`

## 4) Dónde conseguir la base

MaxMind ofrece GeoLite2 (requiere cuenta/licencia de uso). Normalmente usarás:

- `GeoLite2-Country.mmdb` (más liviana)
- `GeoLite2-City.mmdb` (más detalle)

## 5) Requisito clave detrás de proxy

Si usas nginx/caddy/ingress, debes enviar correctamente `X-Forwarded-For`.

Ejemplo nginx:

```nginx
location / {
  proxy_pass http://127.0.0.1:4000/;
  proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

Si no lo haces, la IP registrada/rate-limited puede ser la del proxy y no la del cliente.

## 6) Alcance real (qué sí / qué no)

Sí:

- Logging de IP por request.
- Rate limiting por IP en auth (si configuras `server.auth_ip_rate_limit`).
- Enriquecimiento de logs/stats con país/ciudad cuando hay MMDB.
- Funciones SQL geoip disponibles en conexiones SQLite de TrailBase.

No:

- No mete geolocalización automáticamente en tus tablas de negocio.
- No reemplaza antifraude (solo es señal adicional).
- No corrige headers de proxy por ti.

## 7) ¿Se puede usar dentro de endpoints?

Sí.

Las funciones geoip están registradas como funciones SQL de SQLite, así que puedes usarlas en:

- Query SQL que corras desde TrailBase.
- Endpoints custom que ejecuten SQL (por ejemplo en guests/wasm).
- Views o consultas administrativas que lean logs.

Funciones disponibles:

- `geoip_country(ip_text)`
- `geoip_city_name(ip_text)`
- `geoip_city_json(ip_text)`

### Ejemplos SQL directos

```sql
SELECT geoip_country('8.8.8.8');
SELECT geoip_city_name('8.8.8.8');
SELECT geoip_city_json('8.8.8.8');
```

Sobre logs:

```sql
SELECT
  client_ip,
  geoip_country(client_ip) AS cc,
  geoip_city_name(client_ip) AS city
FROM _logs
ORDER BY id DESC
LIMIT 50;
```

## 8) Obtener la IP dentro de un endpoint WASM

En endpoints WASM el request completo llega con todos sus headers HTTP. TrailBase no inyecta la IP en ningún campo especial — **debes leerla de los headers** de la misma forma que lo hace el servidor internamente.

### Rust

```rust
use std::net::IpAddr;

fn extract_client_ip(req: &http::Request<impl http_body::Body>) -> Option<String> {
    let headers = req.headers();
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').last())   // rightmost = más cercano al cliente
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
}

// Dentro del handler:
let ip = extract_client_ip(&req).unwrap_or_default();
let country: String = conn
    .query_row("SELECT geoip_country(?1)", [&ip], |r| r.get(0))
    .unwrap_or_default();
```

### TypeScript / JavaScript

```typescript
export async function handler(req: Request): Promise<Response> {
  const ip =
    req.headers.get("x-forwarded-for")?.split(",").at(-1)?.trim() ??
    req.headers.get("x-real-ip") ??
    "";

  // Usar la IP como parámetro en SQL vía TrailBase client
  const result = await trailbase.query(
    "SELECT geoip_country(?1) AS country",
    [ip]
  );

  return Response.json({ ip, country: result[0]?.country ?? null });
}
```

> **Importante:** `_USER_`, `_ROW_` y `_REQ_` de los access rules **no** contienen la IP. Esos placeholders solo existen en las access rules del Record API, no en endpoints WASM. En WASM tienes acceso completo al request.

### Contexto inyectado por TrailBase

Además de los headers normales, TrailBase añade un header `__context` con información del usuario autenticado:

```json
{
  "kind": "Http",
  "registered_path": "/api/my-endpoint",
  "path_params": { "id": "123" },
  "user": {
    "id": "...",
    "email": "user@example.com",
    "csrf_token": "..."
  }
}
```

La IP **no aparece** en ese contexto — úsala siempre desde los headers HTTP estándar.

## 9) Patrón recomendado para endpoints

Si tu endpoint recibe IP explícita:

```sql
SELECT geoip_country(:ip) AS country;
```

Si quieres usar la IP ya registrada en logs:

```sql
SELECT
  client_ip,
  geoip_country(client_ip) AS country
FROM _logs
WHERE id = :id;
```

Buenas prácticas:

- No confíes ciegamente en IP para seguridad crítica.
- Usa GeoIP como señal adicional (riesgo, analytics, UX regional).
- Guarda cache de resultados si haces lookup masivo.

## 9) Verificación rápida end-to-end

1. Arranca con MMDB:

```bash
trail run --data-dir ./traildepot --geoip-db-path ./traildepot/GeoLite2-City.mmdb
```

2. Genera tráfico a tu API.

3. Revisa Admin Logs (deberías ver columna GeoIP enriquecida si hay match).

4. Ejecuta una query SQL de prueba:

```sql
SELECT geoip_country('89.160.20.112');
```

Si no devuelve nada:

- revisa que el `.mmdb` exista y sea legible,
- confirma que la IP sea pública (privadas/locales no siempre mapean),
- verifica formato de IP.

## 10) Privacidad y cumplimiento

- IP puede ser dato personal en varias jurisdicciones.
- Define retención de logs (`server.logs_retention_sec`).
- Considera anonimización o truncado para casos sensibles.
- En demos, TrailBase tiene modo `--demo` para redacción de PII en logs del admin.

## 11) Referencias de implementación

- `crates/core/src/extract/ip.rs`
- `crates/core/src/logging.rs`
- `crates/core/src/server/mod.rs`
- `crates/core/src/server/init.rs`
- `crates/extension/src/geoip.rs`
- `crates/core/src/admin/logs/list_logs.rs`
- `crates/core/src/admin/logs/stats.rs`
- `docs/src/content/docs/documentation/production.mdx`
