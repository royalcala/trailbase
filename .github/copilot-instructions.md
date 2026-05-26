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
