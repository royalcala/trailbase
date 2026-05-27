# Copilot Instructions — syntrix-core

## Contexto del proyecto

**syntrix-core** es el motor backend del producto Syntrix. Nació como fork de
[trailbaseio/trailbase](https://github.com/trailbaseio/trailbase) (OSL-3.0) pero ha
divergido lo suficiente para vivir como producto independiente. Ver [ATTRIBUTION.md](../ATTRIBUTION.md).

- Repo local: `/home/alcala/Documents/github/trailbase` (nombre de carpeta legacy, el producto es syntrix-core)
- Branch principal: `syntrix-main`
- Imagen Docker: `ghcr.io/royalcala/trailbase` (se migrará a `ghcr.io/royalcala/syntrix-core`)
- Crates internos aún usan nombre `trailbase` (rename pendiente, no bloqueante)

### Features propias de syntrix-core vs TrailBase upstream

| Feature | syntrix-core | TrailBase upstream |
|---------|-------------|-------------------|
| Multi-org (DB por org) | ✅ `org_<slug>.db` | ❌ |
| Queue system (`queue.db`) | ✅ 3 tablas + admin API | ❌ |
| Schema system/ vs app/ | ✅ Declarativo separado | ❌ |
| CLI `schema export` 5 DBs | ✅ main/org/session/logs/queue | Parcial |
| Admin API queue endpoints | ✅ `/api/_admin/queue/*` | ❌ |
| SDK TS/Rust queue methods | ✅ | ❌ |
| Admin UI Queue page | ✅ `/_/admin/queue` | ❌ |
| OpenAPI queue endpoints | ✅ `AdminQueueApi` vía utoipa | ❌ |

### Stack

- **Runtime**: Rust + Axum + SQLite (via trailbase-sqlite wrapper)
- **Admin UI**: SolidJS + TanStack Table + utoipa (OpenAPI)
- **CLI**: `trail` binary (crate `trailbase-cli`)
- **SDKs**: TypeScript (`crates/assets/js/client/`), Rust (`crates/client/`)

---

## Queue System (syntrix-core feature)

> Sistema de colas persistente en `queue.db`, separado de `main.db`.

### Bases de datos del sistema

| DB | Archivo | Propósito |
|----|---------|-----------|
| main | `main.db` | Datos de aplicación + usuarios/orgs |
| org | `org_<slug>.db` | Datos aislados por organización |
| session | `sessions.db` | Sesiones de autenticación |
| logs | `logs.db` | Request logs + métricas |
| queue | `queue.db` | Jobs de trabajo asíncrono |

### Tablas queue.db

- `_queue_job` — job con status, priority, attempts, worker_id, lease_until
- `_queue_job_attempt` — historial de cada intento
- `_queue_dead_letter` — jobs fallidos tras max_attempts

### Admin API endpoints

```
GET /api/_admin/queue/jobs   → ListQueueJobsResponse  (hasta 200 jobs recientes)
GET /api/_admin/queue/stats  → QueueStatsResponse     (totales por status)
```

Ambos endpoints están documentados en OpenAPI vía `AdminQueueApi` struct (utoipa),
wired en `crates/core/src/lib.rs` bajo `/api/_admin`.

### SDK usage

**TypeScript:**
```ts
const jobs = await client.queueJobs();   // { total_row_count, jobs[] }
const stats = await client.queueStats(); // { total_jobs, by_status[] }
```

**Rust:**
```rust
let jobs = client.queue_jobs().await?;
let stats = client.queue_stats().await?;
```

### Admin UI

Ruta: `/_/admin/queue` — tabla de jobs + cards de stats por status.
Ícono en sidebar: `TbOutlineListDetails`.

### Archivos clave — Queue

| Archivo | Propósito |
|---------|-----------|
| `crates/core/migrations/queue/V1__initial.sql` | Schema inicial embebido |
| `crates/core/src/admin/queue/list_jobs.rs` | Handler GET /queue/jobs |
| `crates/core/src/admin/queue/stats.rs` | Handler GET /queue/stats |
| `crates/core/src/admin/queue/mod.rs` | OpenAPI `AdminQueueApi` struct |
| `crates/core/src/connection.rs` | `init_queue_db()` |
| `crates/core/src/migrations.rs` | `apply_queue_migrations()` |
| `crates/assets/js/admin/src/components/queue/QueuePage.tsx` | Admin UI page |
| `crates/assets/js/admin/src/lib/api/queue.ts` | Admin UI API calls |

---

## Publicar imagen Docker multi-arch desde server-1

> **Alternativa al CI/CD de GitHub Actions**: build y push manual desde `server-1` usando buildx multi-arch (amd64 + arm64).

### Prerequisitos

- `server-1` tiene el repo en `/root/Documents/github/trailbase`
- BuildKit builder `trailbase-local-builder` ya creado (`docker buildx ls` para verificar)
- Token de GitHub con scopes `read:packages`, `write:packages`, `repo`

### Flujo completo

#### 1. Autenticar Docker en server-1

Desde el laptop (requiere que `gh` CLI esté autenticado localmente con los scopes correctos):

```bash
# Si el token local no tiene write:packages, renovar primero:
gh auth refresh -h github.com -s read:packages -s write:packages -s repo
# (abre browser con device code de 8 dígitos — autorizar en https://github.com/login/device)

# Pasar token a server-1
gh auth token | ssh server-1 "docker login ghcr.io -u royalcala --password-stdin"
# Debe responder: Login Succeeded
```

Verificar que quedó guardado:
```bash
ssh server-1 "python3 -c \"import json; c=json.load(open('/root/.docker/config.json')); print('OK' if 'ghcr.io' in c.get('auths',{}) else 'MISSING')\""
```

#### 2. Lanzar el publish en background

```bash
ssh server-1 "cd /root/Documents/github/trailbase && IMAGE_NAME=royalcala/trailbase TAGS=latest,\$(git rev-parse --short HEAD) nohup ./deploy/docker_publish_local.sh > /tmp/trailbase_publish.log 2>&1 & echo PID=\$!"
```

Variables configurables del script (`deploy/docker_publish_local.sh`):

| Variable     | Default                       | Descripción                        |
|--------------|-------------------------------|------------------------------------|
| `REGISTRY`   | `ghcr.io`                     | Registry destino                   |
| `IMAGE_NAME` | `royalcala/trailbase`         | Nombre de la imagen (→ syntrix-core pending) |
| `TAGS`       | `latest,sha-<shortsha>`       | Tags separados por coma            |
| `PLATFORMS`  | `linux/amd64,linux/arm64`     | Plataformas target                 |
| `BUILDER`    | `trailbase-local-builder`     | buildx builder                     |
| `PUSH`       | `true`                        | `false` para solo build local      |

#### 3. Monitorear progreso

```bash
ssh server-1 "tail -f /tmp/trailbase_publish.log"
# o para no bloquear:
ssh server-1 "tail -30 /tmp/trailbase_publish.log"
```

El build Rust multi-arch tarda ~2 horas la primera vez. Con cache (~30 min si solo cambia código Rust). El push final tarda ~1-2 min.

#### 4. Verificar resultado

```bash
ssh server-1 "docker buildx imagetools inspect ghcr.io/royalcala/trailbase:latest | head -20"
```

### Errores comunes

| Error | Causa | Solución |
|-------|-------|----------|
| `permission_denied: The token provided does not match expected scopes` | Token sin `write:packages` | Renovar con `gh auth refresh -s write:packages` y re-login |
| `error: cannot perform an interactive login from a non-TTY device` | Normal al verificar login — ignorar | Ver `config.json` directamente |
| Builder no encontrado | Builder eliminado o no inicializado | `ssh server-1 "docker buildx create --name trailbase-local-builder --use"` |

### Seguridad

- **Nunca pegar tokens en el chat**. Usar siempre el pipe `gh auth token | ssh ...`
- Revocar tokens temporales en https://github.com/settings/tokens después de usarlos
- El token de `gh auth token` tiene vida limitada; si expira, repetir `gh auth refresh`

---

## Publicar imagen Docker multi-arch desde GitHub Actions

> **Alternativa a server-1**: Usar el workflow manual `docker-publish.yml` ejecutado en los runners de GitHub Actions. Más simple (sin SSH), pero más lento (~40+ min build Rust multi-arch).

### Prerequisitos

- Token de GitHub con scopes `write:packages` (automático si usas GITHUB_TOKEN dentro del workflow)
- Acceso a Actions en el repo

### Flujo

1. Ve a: https://github.com/royalcala/trailbase/actions
2. Click en workflow **`docker-publish`** (izquierda)
3. Click en **`Run workflow`** (derecha)
4. Selecciona entrada manual (opcional):
   - `registry`: Por defecto `ghcr.io`
   - `image_name`: Por defecto `royalcala/trailbase`
   - `push`: Por defecto `true` (si es `false`, solo builddea localmente sin subir)
5. Click **`Run workflow`**

El workflow:
- ✅ Hace checkout con submodules
- ✅ Builddea multi-arch (linux/amd64, linux/arm64) en paralelo
- ✅ Tags automáticos: `latest` + commit SHA corto (e.g., `820e49fd`)
- ✅ Usa GitHub Actions cache para acelerar builds posteriores
- ✅ Pushea a `ghcr.io/royalcala/trailbase:latest` y `ghcr.io/royalcala/trailbase:820e49fd`

### Comparación: server-1 vs GitHub Actions

| Aspecto | server-1 | GitHub Actions |
|--------|----------|-----------------|
| Tiempo build Rust (primera vez) | ~2 horas (amd64+arm64 paralelo) | ~4-5 horas (runners compartidos más lentos) |
| Tiempo build (con cache) | ~30-45 min | ~3-4 horas (caché menos efectiva) |
| Cache local | ✅ Persistente entre publishes | ⚠️ Caché GHA compartida, menos confiable |
| Control manual | Alto (SSH directo, logs en tiempo real) | Medio (UI GitHub, logs en Actions tab) |
| Autenticación | Manual con `gh auth refresh` | Automático con `GITHUB_TOKEN` |
| Costo | Gratis (tu hardware) | Gratis (minutos incluida) |
| Best for | **Producción (rápido)**, desarrollo iterativo | Backup / cuando server-1 no esté disponible |

### Monitorear desde Actions tab

```
https://github.com/royalcala/trailbase/actions/workflows/docker-publish.yml
```

---

## Arquitectura de Esquemas (system/ vs app/)

> **Decisión arquitectónica**: Separación explícita entre esquemas generados por syntrix-core (siempre sincronizados) vs esquemas de aplicación (usuario-editables).

### Estructura

```
traildepot/schema/
├── system/                        # ⚠️ Autogenerado por 'trail schema export'
│   ├── main.sql                   # Sistema: _user, _org, _org_membership, _session, etc
│   ├── org.sql                    # Sistema org-scoped
│   ├── session.sql                # Sistema sessions.db
│   ├── logs.sql                   # Sistema logs.db
│   └── queue.sql                  # Sistema queue.db
│
└── app/                           # ✅ Código de aplicación (edita aquí)
    ├── main.sql                   # Negocio: contracts, properties, tenants, etc
    └── org.sql                    # Negocio org-scoped: org_contracts, org_properties, etc
```

### CLI Integration

1. **`trail schema export`** → Genera `system/*.sql` (tablas del sistema, 5 bases de datos)
2. **`trail declarative plan/apply`** → Compara `app/main.sql + app/org.sql` contra DB actual
3. **`trail migration`** → Crea archivos en `migrations/main/` y `migrations/orgs/`

### Flujo de Sincronización

**Cuando syntrix-core actualiza**:

```bash
# 1. Extraer nuevos esquemas del sistema
trail schema export --db all --output traildepot/schema/system/

# 2. Ver cambios
git diff traildepot/schema/system/

# 3. Commitear (auditoría)
git add traildepot/schema/system/
git commit -m "chore(syntrix-core): sync system schemas vX.Y.Z"
```

**Cuando TÚ actualizas esquema de negocio**:

```bash
# 1. Edita app/main.sql o app/org.sql
# 2. Planifica cambios
trail declarative plan --db main --schema traildepot/schema/app/main.sql
# 3. Aplica migrations
trail declarative apply --db main --schema traildepot/schema/app/main.sql
# 4. Commit
git add traildepot/schema/app/
git commit -m "feat(schema): add new business table"
```

### Multi-Org Behavior

```
traildepot/
├── schema/
│   ├── system/
│   │   ├── main.sql        # Sistema main.db
│   │   ├── org.sql         # Sistema org_*.db
│   │   ├── session.sql     # Sistema sessions.db
│   │   ├── logs.sql        # Sistema logs.db
│   │   └── queue.sql       # Sistema queue.db
│   └── app/
│       ├── main.sql        # Negocio main.db
│       └── org.sql         # Negocio org_*.db
│
├── migrations/
│   ├── main/               # Migraciones para main.db
│   │   └── U*.sql
│   └── orgs/               # Per-org migrations (lazy created)
│       └── org_<slug>/
│           └── U*.sql
│
└── data/
    ├── main.db
    ├── queue.db
    ├── org_slug1.db
    └── org_slug2.db
```

### Best Practices

1. **Nunca edites `system/` manualmente** — será sobrescrito
2. **Siempre commitea `system/` changes** — auditoría de evolución de syntrix-core
3. **Migraciones destructivas** — Requieren `--destructive` flag explícito
4. **Schema-driven workflow** — Prefiere `schema/app/` + `trail declarative apply` sobre migrations manuales

---

## Migración pendiente: trailbase → syntrix-core (nombres internos)

Los crates internos aún se llaman `trailbase`, `trailbase-cli`, etc.
El rename es un refactor grande — se hará cuando el delta con upstream lo justifique.

### Checklist del rename (cuando se decida hacer)

- [ ] `Cargo.toml` root + todos los `crates/*/Cargo.toml` (`name = "trailbase*"` → `"syntrix-*"`)
- [ ] `use trailbase::` → `use syntrix_core::` en todo el workspace
- [ ] Binary name en `crates/cli/Cargo.toml`: `trail` → `syntrix`
- [ ] Dockerfile `ENTRYPOINT`
- [ ] GitHub Actions workflows
- [ ] `deploy/install.sh` y `deploy/install.ps1`
- [ ] Docker image: `ghcr.io/royalcala/trailbase` → `ghcr.io/royalcala/syntrix-core`
- [ ] Constantes internas: `AUTH_API_PATH`, `ADMIN_API_PATH` etc. (opcionales, no user-facing)
