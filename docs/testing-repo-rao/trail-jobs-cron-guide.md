# TrailBase Jobs y Cron: limites, alcances y guia de uso

## 1. Resumen rapido

TrailBase si incluye scheduler de jobs con expresiones cron.
Hoy existen dos caminos de uso:

1. Jobs del sistema (built-in), configurables desde `config.textproto`.
2. Jobs custom registrados por componentes WASM (TypeScript o Rust).

Tambien hay API/admin UI para:

- listar jobs,
- ver proxima corrida,
- ver ultimo resultado,
- ejecutar un job manualmente.

## 2. Arquitectura (como esta implementado)

### 2.1 Nucleo del scheduler

El scheduler esta en `crates/core/src/scheduler.rs`.
Cada job mantiene:

- `id`,
- `name`,
- `schedule` (cron),
- callback async,
- estado de ejecucion (`running`, ultima corrida, error opcional).

Ejecucion:

- El job calcula la siguiente fecha con `schedule.upcoming(Utc).next()`.
- Duerme hasta ese instante.
- Ejecuta callback.
- Repite.

### 2.2 Inicializacion

Los jobs del sistema se construyen al crear `AppState` usando config (`build_job_registry_from_config`).

Los jobs WASM se registran al instalar rutas/handlers del runtime WASM (`install_routes_and_jobs`).

### 2.3 Operacion y observabilidad

Admin API:

- `GET /api/_admin/jobs` lista estado de jobs.
- `POST /api/_admin/job/run` ejecuta por id.

Cada item devuelve:

- `enabled`,
- `next` (epoch sec),
- `latest`: `(start_ts, duration_ms, error?)`.

## 3. Alcance actual (que SI cubre)

### 3.1 Soporte cron

Acepta:

- aliases: `@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`.
- cron de 6 o 7 campos.

Orden esperado de campos:

- `second minute hour day-of-month month day-of-week [year]`

Importante: incluye **segundos** (no solo 5 campos).

### 3.2 Jobs del sistema incluidos

IDs disponibles en `SystemJobId`:

- `BACKUP` (default `@daily`, viene deshabilitado por default).
- `HEARTBEAT` (default `17 * * * * * *`).
- `LOG_CLEANER` (default `@hourly`).
- `AUTH_CLEANER` (default `@hourly`).
- `QUERY_OPTIMIZER` (default `@daily`).
- `FILE_DELETIONS` (default `@hourly`).

### 3.3 Jobs custom WASM

Se pueden registrar handlers custom desde guest:

- TypeScript: `jobHandlers` en `defineConfig`.
- Rust: `fn job_handlers() -> Vec<Job>`.

## 4. Limites actuales (que NO cubre o debes considerar)

1. No se ve coordinacion distribuida/leader-election en el scheduler.
   Si corres varias replicas del servidor, cada replica puede ejecutar el mismo job.

2. No hay pipeline de retries/backoff por job en el scheduler.
   Si falla, se guarda error en `latest`, pero no hay reintento automatico configurable por politica.

3. Persistencia de historial limitada.
   El estado expuesto es principalmente la ultima corrida (`latest`), no un historial completo de auditoria de ejecuciones.

4. Cron invalido desactiva efectivamente ese job.
   En startup/config reload se loguea error y no se agenda.

5. Los overrides en `config.textproto` aplican a jobs del sistema.
   Los jobs custom WASM se definen en el propio componente guest.

6. Ejecucion en proceso.
   Si el proceso cae o reinicia, no hay garantia de ejecutar corridas perdidas durante downtime.

## 5. Como configurarlo (system jobs)

En `traildepot/config.textproto`:

```textproto
jobs {
  system_jobs: [
    { id: BACKUP, schedule: "@daily", disabled: false },
    { id: LOG_CLEANER, schedule: "@hourly", disabled: false },
    { id: HEARTBEAT, schedule: "17 * * * * * *", disabled: true }
  ]
}
```

Notas:

- `id` y `schedule` son obligatorios cuando declares una entrada en `system_jobs`.
- `disabled: true` evita que inicie automaticamente.
- Si omites un job del sistema, usa default interno.

## 6. Como crear jobs custom (WASM)

### 6.1 TypeScript

Ejemplo minimo:

```ts
import { defineConfig } from "trailbase-wasm";
import { JobHandler } from "trailbase-wasm/job";

export default defineConfig({
  jobHandlers: [
    JobHandler.hourly("sync_users", async () => {
      console.log("sync_users running");
    }),
  ],
});
```

Tambien puedes usar spec custom (6/7 campos):

```ts
new JobHandler("cleanup_cache", "5 */10 * * * *", async () => {
  console.log("cleanup every 10 min at second 5");
});
```

### 6.2 Rust

Ejemplo minimo:

```rust
use trailbase_wasm::job::Job;
use trailbase_wasm::{Guest, export};

struct Endpoints;

impl Guest for Endpoints {
  fn job_handlers() -> Vec<Job> {
    vec![Job::hourly("sync_users", async || {
      println!("sync_users running");
    })]
  }
}

export!(Endpoints);
```

## 7. Como operarlo (admin/API)

### 7.1 Ver estado de jobs

Request:

```http
GET /api/_admin/jobs
```

Respuesta (shape simplificado):

```json
{
  "jobs": [
    {
      "id": 2,
      "name": "Heartbeat",
      "schedule": "17 * * * * * *",
      "enabled": true,
      "next": 1770000000,
      "latest": [1769999990, 12, null]
    }
  ]
}
```

### 7.2 Ejecutar un job manualmente

Request:

```http
POST /api/_admin/job/run
Content-Type: application/json

{ "id": 2 }
```

Respuesta:

```json
{ "error": null }
```

Si no existe:

- error de precondicion (`Job not found`).

## 8. Buenas practicas recomendadas

1. Haz jobs idempotentes.
   Especialmente si planeas escalar a varias replicas.

2. Evita trabajo pesado bloqueante dentro del callback.
   Usa operaciones async y separa tareas largas en etapas.

3. Define ventanas horarias claras para cargas pesadas.
   Ejemplo: backups y optimize en horas de baja demanda.

4. Registra logs con contexto suficiente.
   Incluye `job_name`, inicio, fin, duracion y causa de error.

5. Prueba specs cron en ambientes de test.
   Empieza con expresiones frecuentes y luego pasa a productivas.

6. Para periodicidad exacta de negocio, prefiere cron explicito en lugar de "cada X segundos" ad-hoc.

## 9. Checklist de puesta en produccion

- [ ] Definir que jobs del sistema quedan habilitados/deshabilitados.
- [ ] Validar todos los `schedule` en `config.textproto`.
- [ ] Confirmar estrategia multi-replica (single leader o jobs idempotentes).
- [ ] Revisar observabilidad (logs, alertas, metricas).
- [ ] Probar `GET /api/_admin/jobs` y `POST /api/_admin/job/run`.
- [ ] Verificar recovery tras restart/downtime.

## 10. Referencias internas de codigo

- Scheduler core: `crates/core/src/scheduler.rs`
- Integracion WASM jobs: `crates/core/src/wasm/mod.rs`
- Registro desde AppState/config: `crates/core/src/app_state.rs`
- Contrato config jobs: `crates/core/proto/config.proto`
- Validacion de cron en config: `crates/core/src/config.rs`
- Admin endpoints jobs: `crates/core/src/admin/mod.rs`
- List jobs handler: `crates/core/src/admin/jobs/list_jobs.rs`
- Run job handler: `crates/core/src/admin/jobs/run_job.rs`
- Ejemplo TS: `examples/wasm-guest-ts/src/index.ts`
- Ejemplo Rust: `examples/wasm-guest-rust/src/lib.rs`
- Changelog introduccion cron jobs: `CHANGELOG.md` (v0.8.0)
