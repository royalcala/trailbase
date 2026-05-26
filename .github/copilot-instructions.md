# Copilot Instructions — trailbase (royalcala fork)

## Contexto del proyecto

Fork de [trailbaseio/trailbase](https://github.com/trailbaseio/trailbase) en la rama `syntrix-main`.
Imagen Docker publicada en `ghcr.io/royalcala/trailbase`.

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
| `IMAGE_NAME` | `royalcala/trailbase`         | Nombre de la imagen                |
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

Ahí ves:
- Estado del workflow (en ejecución, completado, fallido)
- Logs en vivo de cada paso
- Tags generados
- Push exitoso o errores

---

## Arquitectura de Esquemas (system/ vs app/)

> **Decisión arquitectónica**: Separación explícita entre esquemas generados por TrailBase (siempre sincronizados) vs esquemas de aplicación (usuario-editables).

### Contexto

TrailBase crea internamente tablas del sistema (_user, _org, _org_membership, sessions, etc). Estas son críticas pero típicamente ocultas en el CLI. En Syntrix (y en cualquier cliente que use TrailBase custom), queremos:
- ✅ **Transparencia**: Ver exactamente qué crea TrailBase
- ✅ **Versionado**: Cada cambio queda en Git con audit trail
- ✅ **Sincronización**: Detectar cuando TrailBase evoluciona
- ✅ **Educación**: Separación clara qué es sistema vs qué es negocio

### Estructura

```
traildepot/schema/
├── system/                        # ⚠️ Autogenerado por 'trail schema export'
│   ├── main.sql                   # Sistema: _user, _org, _org_membership, _session, etc
│   └── org.sql                    # Sistema org-scoped (vacío o mínimo)
│
└── app/                           # ✅ Código de aplicación (edita aquí)
    ├── main.sql                   # Negocio: contracts, properties, tenants, etc
    └── org.sql                    # Negocio org-scoped: org_contracts, org_properties, etc
```

### CLI Integration

El CLI de TrailBase entiende esta estructura:

1. **`trail schema export`** → Genera `system/main.sql` (tablas del sistema)
2. **`trail declarative plan/apply`** → Compara `app/main.sql + app/org.sql` contra DB actual
3. **`trail migration`** → Crea archivos en `migrations/main/` y `migrations/orgs/`

### Flujo de Sincronización

**Cuando TrailBase actualiza** (nueva versión del binary o custom changes):

```bash
# 1. Extraer nuevos esquemas del sistema
trail schema export > traildepot/schema/system/main.sql

# 2. Ver cambios
git diff traildepot/schema/system/main.sql

# 3. Commitear (auditoría)
git add traildepot/schema/system/main.sql
git commit -m "chore(trailbase): sync system schemas v1.2.3 → v1.3.0"
```

**Cuando TÚ actualizas esquema de negocio**:

```bash
# 1. Edita app/main.sql o app/org.sql
vim traildepot/schema/app/main.sql

# 2. Planifica cambios
trail declarative plan --db main --schema traildepot/schema/app/main.sql

# 3. Aplica migrations
trail declarative apply --db main --schema traildepot/schema/app/main.sql

# 4. Commit
git add traildepot/schema/app/
git commit -m "feat(schema): add new business table"
```

### Relación con Migrations

- **Migrations** (`migrations/main/U*.sql`) → Append-only history, version control para schema evolution
- **Declarative schema** (`schema/app/main.sql`) → Source of truth para estado deseado
- **System schema** (`schema/system/main.sql`) → Immutable reference, actualizado cuando TrailBase evoluciona

TrailBase materializa la diferencia: `app/*` - `system/*` = `migrations/` que se necesitan.

### Multi-Org Behavior

Con TrailBase custom multi-org:

```
traildepot/
├── schema/
│   ├── system/
│   │   ├── main.sql        # Sistema main.db
│   │   └── org.sql         # Sistema org_*.db
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
    ├── main.db             # Base de datos principal
    ├── org_slug1.db        # Org 1 database
    └── org_slug2.db        # Org 2 database
```

TrailBase aplica `schema/system/org.sql + schema/app/org.sql` a cada org DB cuando se abre (lazy o startup sweep).

### Best Practices

1. **Nunca edites `system/` manualmente** — será sobrescrito
2. **Siempre commitea `system/` changes** — auditoría de evolución de TrailBase
3. **Migraciones destructivas** — Requieren `--destructive` flag explícito
4. **Schema-driven workflow** — Prefiere `schema/app/` + `trail declarative apply` sobre migrations manuales para cambios reversibles
5. **Monitorea diffs** — `git diff schema/system/main.sql` después de actualizar TrailBase para detectar cambios incompatibles

### Ventajas en Syntrix (cliente)

En el proyecto Syntrix que usa este TrailBase custom:
- CLI `just sync-trailbase-schemas` automatiza la sincronización
- Documentación clara de qué es sistema vs negocio
- Git history completo de evolución de ambos
- Fácil reproducir estructura en otros ambientes


