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

### Ejemplo: Usar GitHub Actions para multi-org

Mismo flujo que usamos hoy en server-1, pero sin SSH:

1. Merge `syntrix-multi-org` → `syntrix-main` ✅ (ya hecho)
2. Push a remoto ✅ (ya hecho)
3. Ve a GitHub Actions
4. Click `Run workflow` (usa defaults: ghcr.io, royalcala/trailbase, latest+sha)
5. Espera ~40 min
6. Verifica: https://ghcr.io/royalcala/trailbase:latest tiene multi-org support
