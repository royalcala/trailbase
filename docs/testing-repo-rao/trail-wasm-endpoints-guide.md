# Guía: Custom Endpoints con WASM en TrailBase

Esta guía explica:

- Qué son los custom endpoints WASM y para qué sirven.
- Cómo funciona internamente (cómo TrailBase los carga).
- Cómo crear un proyecto desde cero en Rust y en TypeScript.
- Qué APIs tiene disponibles el guest (DB, KV, HTTP fetch, Jobs, IP).
- El flujo completo de build → deploy.
- Limitaciones conocidas.

---

## 1) ¿Qué es un custom endpoint WASM?

TrailBase incluye un sistema web (Record APIs, Auth, Admin) listo para usar, pero a veces necesitas lógica personalizada: validaciones complejas, integraciones con APIs externas, cálculos, cron jobs internos, etc.

Para eso existen los **WASM endpoints**: módulos que compilan a WebAssembly Component Model (WASI p2) y que TrailBase carga en startup, registrando sus rutas HTTP junto al resto del servidor.

Características clave:

- Lenguaje: **Rust** (recomendado) o **TypeScript/JavaScript** (vía `jco componentize`).
- El módulo corre **dentro del proceso de TrailBase** (no es un proceso aparte).
- Tiene acceso a la base de datos SQLite de TrailBase vía una API interna.
- Puede definir rutas HTTP y/o cron jobs.
- Se recarga con `SIGHUP` (sin reiniciar el proceso completo).

---

## 2) Cómo los carga TrailBase

TrailBase busca automáticamente todos los archivos `*.wasm` dentro de:

```
<data-dir>/wasm/
```

Por ejemplo si usas `--data-dir ./traildepot`, el directorio sería:

```
traildepot/wasm/component.wasm
traildepot/wasm/otro-modulo.wasm
```

Puedes tener **múltiples módulos** a la vez. Cada uno puede registrar sus propias rutas y jobs.

En código Rust (simplificado):

```rust
// crates/core/src/wasm/mod.rs
let components_path = data_dir.root().join("wasm");
let components = find_wasm_components(&components_path);
// ... inicializa cada uno y registra sus rutas en el router
```

Las rutas del guest se montan bajo el prefijo `/api/` del servidor TrailBase.

---

## 3) Estructura de un proyecto WASM

### Opción A — Rust

```
mi-endpoint/
├── Cargo.toml
├── Makefile
└── src/
    └── lib.rs
```

**Cargo.toml** mínimo:

```toml
[package]
name = "mi-endpoint"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]   # imprescindible

[dependencies]
serde_json = "1"
trailbase-wasm = { version = "..." }
```

**src/lib.rs** mínimo:

```rust
use trailbase_wasm::http::{HttpRoute, routing};
use trailbase_wasm::{Guest, export};

struct Endpoints;

impl Guest for Endpoints {
    fn http_handlers() -> Vec<HttpRoute> {
        vec![
            routing::get("/hello", async |_req| {
                "Hello from WASM!\n"
            }),
        ]
    }

    fn job_handlers() -> Vec<trailbase_wasm::job::Job> {
        vec![]
    }
}

export!(Endpoints);
```

**Makefile** para buildear y copiar:

```makefile
TRAILDEPOT := traildepot
TRAILBIN   ?= trail

run: guest
	$(TRAILBIN) --data-dir=$(TRAILDEPOT) run

guest: $(TRAILDEPOT)/wasm/component.wasm

$(TRAILDEPOT)/wasm/component.wasm: ../../target/wasm32-wasip2/release/mi_endpoint.wasm
	mkdir -p $(TRAILDEPOT)/wasm && cp $< $@

../../target/wasm32-wasip2/release/mi_endpoint.wasm: src/*.rs
	cargo build --target wasm32-wasip2 --release

.PHONY: guest run
```

Para compilar y arrancar:

```bash
make          # compila + copia
make run      # arranca TrailBase
```

---

### Opción B — TypeScript

```
mi-endpoint-ts/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── Makefile
└── src/
    ├── index.ts       # lógica
    └── component.ts   # re-exporta para jco
```

**src/index.ts** mínimo:

```typescript
import { defineConfig } from "trailbase-wasm";
import { HttpHandler, HttpRequest } from "trailbase-wasm/http";

export default defineConfig({
  httpHandlers: [
    HttpHandler.get("/hello", (_req: HttpRequest) => {
      return "Hello from WASM TS!\n";
    }),
  ],
  jobHandlers: [],
});
```

**src/component.ts** (siempre igual):

```typescript
import e from "./index";
export const { initEndpoint, incomingHandler, sqliteFunctionEndpoint } = e;
```

**vite.config.ts**:

```typescript
import { defineConfig } from "vite";

export default defineConfig({
  build: {
    outDir: "./dist",
    minify: false,
    lib: {
      entry: "./src/component.ts",
      name: "runtime",
      fileName: "index",
      formats: ["es"],
    },
    rollupOptions: {
      external: (source) =>
        source.startsWith("wasi:") || source.startsWith("trailbase:"),
    },
  },
});
```

**package.json** scripts clave:

```json
{
  "scripts": {
    "build": "vite build && npm run build:wasm",
    "build:wasm": "jco componentize dist/index.js -w node_modules/trailbase-wasm/wit -o dist/component.wasm",
    "dev": "node --experimental-strip-types hot-reload.ts"
  }
}
```

**Makefile**:

```makefile
TRAILDEPOT := traildepot
TRAILBIN   ?= trail

run: guest
	$(TRAILBIN) --data-dir=$(TRAILDEPOT) run --dev

guest: $(TRAILDEPOT)/wasm/component.wasm

$(TRAILDEPOT)/wasm/component.wasm: dist/component.wasm
	mkdir -p $(TRAILDEPOT)/wasm && cp $< $@

dist/component.wasm: src/*.ts
	pnpm i && pnpm build

clean:
	rm -rf dist traildepot

.PHONY: guest run clean
```

---

## 4) APIs disponibles en el guest

### 4.1 HTTP handler — objeto `Request`

En Rust (`trailbase_wasm::http::Request`):

| Método | Descripción |
|--------|-------------|
| `req.header("nombre")` | Leer un header (→ `Option<&HeaderValue>`) |
| `req.query_param("nombre")` | Leer query string param |
| `req.query_parse::<T>()` | Deserializar query string completa a struct |
| `req.path_param("nombre")` | Leer segmento de path dinámico |
| `req.method()` | Método HTTP |
| `req.url()` | URL completa |
| `req.user()` | Usuario autenticado (`Option<&User>`) |
| `req.body().json::<T>().await` | Deserializar body JSON |

En TypeScript (`HttpRequest`):

```typescript
req.getHeader("nombre")       // string | undefined
req.getQueryParam("nombre")   // string | undefined
req.getPathParam("nombre")    // string | undefined
req.json()                    // object | null (body JSON)
req.user                      // { id, email, csrf_token } | undefined
```

### 4.2 Base de datos (db)

El guest puede hacer queries a la base de datos SQLite de TrailBase. Las queries corren en la misma conexión del servidor.

**Rust:**

```rust
use trailbase_wasm::db::{Value, escape, query};

// SELECT
let rows = query("SELECT id, name FROM mytable WHERE id = ?1", [Value::Integer(42)])
    .await
    .map_err(internal)?;

// Escapar nombres de tabla (no parametrizables)
let count = query(
    format!("SELECT COUNT(*) FROM {}", escape(table_name)),
    []
).await?;

// Transacciones
use trailbase_wasm::db::Transaction;
let mut tx = Transaction::begin()?;
tx.execute("INSERT INTO events (name) VALUES (?1)", &[Value::Text("click".into())])?;
tx.commit()?;
```

**TypeScript:**

```typescript
import { escape, query } from "trailbase-wasm/db";

const rows = await query("SELECT id, name FROM mytable WHERE id = ?1", [42]);
const count = await query(`SELECT COUNT(*) FROM ${escape(tableName)}`, []);
```

> **Nota de seguridad:** Usa siempre parámetros posicionales (`?1`, `?2`...) para valores del usuario. Solo usa `escape()` para nombres de tabla/columna (no para valores).

### 4.3 Key-Value store (kv)

Un almacén clave-valor en memoria persistente entre reloads, accesible desde el guest.

**Rust:**

```rust
use trailbase_wasm::kv::Store;

let mut store = Store::open()?;
store.set("contador", b"42")?;
let val = store.get("contador")?;   // Option<Vec<u8>>
store.delete("contador")?;
let exists = store.exists("contador")?;
```

### 4.4 HTTP fetch (hacia afuera)

Para llamar APIs externas desde el guest.

**Rust:**

```rust
use trailbase_wasm::fetch;

let bytes = fetch::get("https://api.example.com/data").await?;
let json: serde_json::Value = serde_json::from_slice(&bytes)?;
```

```rust
use trailbase_wasm::fetch::{fetch, Request};

let response = fetch(
    Request::builder()
        .uri("https://api.example.com/endpoint")
        .method("POST")
        .body(serde_json::to_vec(&payload)?.into())
        .unwrap()
).await?;
```

**TypeScript:**

```typescript
const res = await fetch("https://api.example.com/data");
const data = await res.json();
```

### 4.5 Timers / sleep

**Rust:**

```rust
use trailbase_wasm::time::{Duration, Timer};

Timer::after(Duration::from_millis(500)).wait().await;
```

**TypeScript:**

```typescript
await sleep(500); // ms
```

### 4.6 Jobs (cron tasks)

Se definen junto a los handlers HTTP en `job_handlers()`.

**Rust:**

```rust
use trailbase_wasm::job::Job;

fn job_handlers() -> Vec<Job> {
    vec![
        Job::minutely("sync-cache", async || {
            println!("sync!");
        }),
        Job::hourly("cleanup", async || { /* ... */ }),
        Job::daily("report", async || { /* ... */ }),
        // Custom cron (segundos opcionales):
        Job::new("custom", "0 9 * * 1-5", async || { /* lunes-viernes 9am */ })
            .expect("cron válido"),
    ]
}
```

Specs válidas: `@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`, o formato cron de 6-7 campos (con segundos).

**TypeScript:**

```typescript
import { JobHandler } from "trailbase-wasm/job";

jobHandlers: [
  JobHandler.minutely("sync", () => console.log("sync!")),
  JobHandler.hourly("cleanup", async () => { /* ... */ }),
]
```

---

## 5) Leer la IP del cliente en un endpoint WASM

Los access rules del Record API **no** incluyen la IP. Pero en un endpoint WASM el request llega completo con todos sus headers.

**Rust:**

```rust
routing::get("/mi-endpoint", async |req| {
    let ip = req
        .header("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').last())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.header("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_default();

    // Combinar con geoip via SQL
    let rows = query(
        "SELECT geoip_country(?1) AS cc",
        [Value::Text(ip.clone())]
    ).await?;

    Ok(format!("IP: {ip}, CC: {:?}\n", rows[0][0]))
})
```

**TypeScript:**

```typescript
HttpHandler.get("/mi-endpoint", async (req: HttpRequest) => {
  const ip =
    req.getHeader("x-forwarded-for")?.split(",").at(-1)?.trim() ??
    req.getHeader("x-real-ip") ??
    "";

  const rows = await query("SELECT geoip_country(?1) AS cc", [ip]);
  return JSON.stringify({ ip, cc: rows[0][0] });
}),
```

---

## 6) Usuario autenticado en el endpoint

TrailBase inyecta automáticamente la info del usuario autenticado (si el request trae token válido):

**Rust:**

```rust
routing::get("/perfil", async |req| {
    let user = req.user().ok_or_else(|| {
        HttpError::status(StatusCode::UNAUTHORIZED)
    })?;

    Ok(format!("Hola {}!\n", user.email))
})
```

**TypeScript:**

```typescript
HttpHandler.get("/perfil", (req: HttpRequest) => {
  if (!req.user) {
    return new HttpResponse("No autorizado", { status: 401 });
  }
  return `Hola ${req.user.email}!`;
}),
```

El objeto `user` contiene:

```typescript
{
  id: string,          // UUID en hex
  email: string,
  csrf_token: string,
}
```

---

## 7) Flujo completo de desarrollo

### Con Rust

```bash
# 1. Primera vez: instalar target
rustup target add wasm32-wasip2

# 2. Compilar
cargo build --target wasm32-wasip2 --release

# 3. Copiar a traildepot/wasm/
cp target/wasm32-wasip2/release/mi_endpoint.wasm traildepot/wasm/component.wasm

# 4. Arrancar TrailBase
trail --data-dir traildepot run

# 5. Recargar componente sin reiniciar (en otro terminal)
kill -HUP $(pgrep trail)
```

Con el Makefile del ejemplo: `make` hace pasos 2+3, `make run` los 4.

### Con TypeScript

```bash
# 1. Instalar deps (una vez)
pnpm install

# 2. Build completo (vite + jco componentize)
pnpm build

# 3. Copiar a traildepot/wasm/
cp dist/component.wasm traildepot/wasm/component.wasm

# 4. Arrancar
trail --data-dir traildepot run --dev
```

Modo desarrollo con hot-reload (reconstruye y manda SIGHUP automáticamente):

```bash
pnpm dev
```

---

## 8) Múltiples módulos WASM

Puedes tener varios archivos `.wasm` en `traildepot/wasm/`. TrailBase los carga todos:

```
traildepot/wasm/
├── auth-hooks.wasm
├── payments.wasm
└── notifications.wasm
```

Cada uno registra sus propias rutas (deben ser únicas entre módulos).

---

## 9) Limitaciones conocidas

| Limitación | Detalle |
|------------|---------|
| Rutas no hot-reloadables | Añadir/quitar rutas requiere reinicio completo del servidor |
| SIGHUP recarga toda la lógica | Pero las rutas ya registradas permanecen hasta restart |
| Sin acceso directo a filesystem host | El guest solo accede a un FS virtual (sandbox) |
| Sin acceso a env vars del host | Por diseño: usa el KV store o la DB para configuración |
| macOS + Winch | Winch JIT desactivado en macOS por limitaciones del sistema |
| TypeScript: bundle con vite primero | El paso `jco componentize` solo entiende un JS bundle, no TS directo |
| IP en access rules | No disponible. Solo en endpoints WASM vía headers HTTP |
