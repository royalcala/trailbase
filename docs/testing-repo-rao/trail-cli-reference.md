# TrailBase CLI — Referencia de Comandos

> Extraído de `crates/cli/src/args.rs` · TrailBase main branch

---

## Flags globales

```bash
trail --data-dir <path>    # directorio de datos (default: traildepot/)
trail --public-url <url>   # URL pública para auth emails y OAuth redirects
trail --version            # imprime la versión instalada
```

---

## `trail run` — Inicia el servidor HTTP

```bash
trail run [opciones]
```

| Flag | Descripción | Default |
|------|-------------|---------|
| `--address <host:port>` | Dirección donde escucha el servidor HTTP | `localhost:4000` |
| `--admin-address <host:port>` | Sirve el panel admin en una dirección separada | — |
| `--public-dir <path>` | Directorio de assets estáticos servidos en la raíz HTTP | — |
| `--spa` | Activa fallback a `index.html` para rutas sin extensión (SPAs) | `false` |
| `--dev` | CORS permisivo + cookies cross-origin para dev con servidor externo | `false` |
| `--demo` | Redacta PII de los logs | `false` |
| `--cors-allowed-origins <origins>` | Lista de orígenes permitidos | `*` |
| `--runtime-root-fs <path>` | FS sandboxed para el runtime WASM | — |
| `--runtime-threads <n>` | Número de threads para el runtime WASM | `#cpus` |
| `--geoip-db-path <path>` | Ruta a base de datos MaxmindDB para geo-localización de IPs | — |
| `--stderr-logging` | Envía logs a stderr | `false` |

---

## `trail schema` — Exporta JSON Schema de una tabla

```bash
trail schema <nombre_api> --mode <insert|select|update>
```

Los tres modos corresponden a diferentes casos de uso:
- `insert` — campos requeridos para crear un registro
- `select` — campos que devuelve la API al leer
- `update` — campos permitidos al actualizar

**Ejemplo:**
```bash
trail schema tenants --mode select
trail schema properties --mode insert
```

Úsalo con `quicktype` para generar tipos TypeScript:
```bash
trail schema tenants --mode select | quicktype --lang typescript -o src/types/Tenants.ts
```

---

## `trail openapi` — Exporta la definición OpenAPI

```bash
trail openapi print                      # imprime el JSON OpenAPI en stdout
trail openapi run [--address host:port]  # levanta Swagger UI (requiere feature "swagger")
```

---

## `trail migration` — Crea archivo de migración vacío

```bash
trail migration [sufijo] [db]
```

Genera un archivo con nombre `U<timestamp>__<sufijo>.sql` en `traildepot/migrations/main/` (o en la DB indicada).

**Ejemplo:**
```bash
trail migration add_properties_table
# → traildepot/migrations/main/U1715000000__add_properties_table.sql
```

---

## `trail admin` — Gestiona usuarios administradores

```bash
trail admin list                    # lista todos los admins
trail admin promote <email|uuid>    # promueve usuario a admin
trail admin demote  <email|uuid>    # degrada admin a usuario normal
```

---

## `trail user` — Gestiona usuarios (incluye admins)

```bash
trail user add <email> <password>                     # crea usuario verificado
trail user delete <email|uuid>                        # elimina usuario
trail user change-password <email|uuid> <nueva>       # cambia contraseña
trail user change-email <email|uuid> <nuevo_email>    # cambia email
trail user verify <email|uuid> [true|false]           # cambia estado de verificación
trail user invalidate-session <email|uuid>            # cierra sesión (invalida tokens)
trail user mint-token <email|uuid>                    # genera auth token
trail user import --auth0-json <path> [--dry-run]     # importa usuarios desde Auth0
```

> **Diferencia con `trail admin`:** `trail user` puede operar sobre usuarios admin también; `trail admin` solo maneja el rol.

---

## `trail email` — Envía emails programáticamente

```bash
trail email \
  --to      destinatario@ejemplo.com \
  --subject "Asunto del mensaje" \
  --body    "Cuerpo del email"
```

Requiere que el SMTP esté configurado en `config.textproto`.

---

## `trail components` — Gestiona WASM components

```bash
trail components list           # lista componentes first-party disponibles
trail components installed      # lista componentes actualmente instalados
trail components add <ref>      # instala un componente (path .wasm/.zip, URL HTTPS, o nombre)
trail components remove <ref>   # desinstala un componente
trail components update         # actualiza todos los first-party instalados
```

`<ref>` puede ser:
- Ruta local: `./mi-handler.wasm` o `./bundle.zip`
- URL HTTPS: `https://ejemplo.com/handler.wasm`
- Nombre de componente first-party: `trailbase/some-component`

---

## Referencia rápida

```
trail --version
trail run --dev
trail schema <api> --mode insert|select|update
trail openapi print
trail migration <sufijo>
trail admin list|promote|demote
trail user add|delete|change-password|change-email|verify|invalidate-session|mint-token|import
trail email --to --subject --body
trail components list|installed|add|remove|update
```
