# Referencia Completa de config.textproto (TrailBase)

Este documento describe, en detalle, todo lo que acepta `traildepot/config.textproto` en TrailBase:

- Campos soportados (según `crates/core/proto/config.proto`).
- Tipos y enums permitidos.
- Valores por defecto reales (cuando existen).
- Validaciones de runtime (según `crates/core/src/config.rs` y `crates/core/src/records/validate.rs`).
- Ejemplos prácticos listos para usar.

## 1) Estructura top-level

`config.textproto` corresponde al mensaje `config.Config` y contiene estos bloques:

- `email` (requerido)
- `server` (requerido)
- `auth` (requerido)
- `jobs` (requerido)
- `databases` (opcional/repetido)
- `record_apis` (opcional/repetido)
- `schemas` (opcional/repetido)

Ejemplo mínimo funcional:

```textproto
email {}
server {
  application_name: "TrailBase"
}
auth {}
jobs {}
```

Nota: TrailBase inicializa defaults cuando crea un `config.textproto` nuevo, pero en validación algunos campos siguen siendo obligatorios en la práctica (ej. `server.application_name`).

---

## 2) Bloque email

### 2.1 Campos

```textproto
email {
  smtp_host: "smtp.example.com"
  smtp_port: 587
  smtp_username: "user"
  smtp_password: "secret"
  smtp_encryption: SMTP_ENCRYPTION_STARTTLS

  sender_name: "Mi App"
  sender_address: "noreply@example.com"

  user_verification_template {
    subject: "Verifica tu cuenta"
    body: "Haz click en {{ VERIFICATION_URL }}"
  }
  password_reset_template {
    subject: "Reset"
    body: "Reset: {{ VERIFICATION_URL }}"
  }
  change_email_template {
    subject: "Cambio de email"
    body: "Confirma: {{ VERIFICATION_URL }}"
  }
  otp_template {
    subject: "Tu codigo"
    body: "Codigo: {{ CODE }}"
  }
}
```

### 2.2 Enum smtp_encryption

- `SMTP_ENCRYPTION_UNDEFINED` (0)
- `SMTP_ENCRYPTION_NONE` (1)
- `SMTP_ENCRYPTION_STARTTLS` (2)
- `SMTP_ENCRYPTION_TLS` (3)

Comportamiento:

- Comentario del proto: STARTTLS por defecto.
- Si no configuras SMTP (`smtp_host`, `smtp_port`, `smtp_username`, `smtp_password` ausentes), se permite y TrailBase no falla por eso.

### 2.3 Validaciones importantes

- Si falta `smtp_host` y envías partes sueltas de SMTP, error: configuración parcial.
- Si hay `sender_address`, debe ser email válido y además requiere `sender_name`.
- `smtp_port` debe estar presente cuando hay SMTP y ser > 0 (rango uint16 válido).
- Con cifrado distinto de `NONE`, `smtp_username` y `smtp_password` son requeridos y no vacíos.
- En templates de email, si defines `body`, debe contener `{{ VERIFICATION_URL }}` o `{{ CODE }}`.

---

## 3) Bloque server

### 3.1 Campos

```textproto
server {
  application_name: "Mi App"
  site_url: "https://miapp.com"
  logs_retention_sec: 604800

  s3_storage_config {
    endpoint: "https://s3.amazonaws.com"
    region: "us-east-1"
    bucket_name: "mi-bucket"
    access_key: "AKIA..."
    secret_access_key: "..."
  }

  enable_record_transactions: true
  request_size_limit_bytes: 10485760
  auth_ip_rate_limit: 20
}
```

### 3.2 Defaults conocidos

- `application_name`: `TrailBase` (al inicializar config por defecto).
- `logs_retention_sec`: 7 días = `604800`.
- `request_size_limit_bytes`: si no lo defines, el server aplica 10 MB en runtime.

### 3.3 Validaciones importantes

- `application_name` es requerido en la práctica.
- `application_name` solo permite: alfanumérico ASCII, espacio, `_`, `-`, `.`.
- `application_name` no puede ser vacío.
- `site_url` si está presente debe parsear como URL válida.

---

## 4) Bloque auth

### 4.1 Campos

```textproto
auth {
  auth_token_ttl_sec: 3600
  refresh_token_ttl_sec: 2592000

  disable_password_auth: false
  enable_otp_signin: true

  password_minimal_length: 8
  password_must_contain_upper_and_lower_case: false
  password_must_contain_digits: false
  password_must_contain_special_characters: false

  oauth_providers {
    key: "google"
    value {
      provider_id: GOOGLE
      client_id: "..."
      client_secret: "..."
    }
  }

  custom_uri_schemes: "myapp"
  redirect_uri_allowlist: "https://miapp.com/callback"
}
```

### 4.2 Defaults conocidos

- `auth_token_ttl_sec`: 3600 en builds release (en debug, código usa 120s).
- `refresh_token_ttl_sec`: 2592000 (30 días).

### 4.3 Enum OAuthProviderId

- `OAUTH_PROVIDER_ID_UNDEFINED` (0)
- `TEST` (1)
- `OIDC0` (2)
- `APPLE` (9)
- `DISCORD` (10)
- `GITLAB` (11)
- `GOOGLE` (12)
- `FACEBOOK` (13)
- `MICROSOFT` (14)
- `TWITCH` (15)
- `YANDEX` (16)
- `GITHUB` (17)

### 4.4 Validaciones OAuth

Para cada entrada en `oauth_providers`:

- `provider_id` debe ser válido y distinto de `UNDEFINED`.
- La key del map debe coincidir exactamente con el nombre de factory interno.
  - Ejemplo esperado para Google: key `google`.
- `client_id` requerido y sin espacios extra al inicio/fin.
- `client_secret` requerido y sin espacios extra al inicio/fin.
- Si `provider_id = OIDC0`, además son obligatorios y válidos:
  - `auth_url`
  - `token_url`
  - `user_api_url`

---

## 5) Bloque jobs

### 5.1 Campos

```textproto
jobs {
  system_jobs: [
    {
      id: BACKUP
      schedule: "@daily"
      disabled: true
    },
    {
      id: HEARTBEAT
      schedule: "17 * * * * * *"
      disabled: false
    }
  ]
}
```

### 5.2 Enum SystemJobId

- `SYSTEM_JOB_ID_UNDEFINED` (0)
- `BACKUP` (1)
- `HEARTBEAT` (2)
- `LOG_CLEANER` (3)
- `AUTH_CLEANER` (4)
- `QUERY_OPTIMIZER` (5)
- `FILE_DELETIONS` (6)

### 5.3 Schedules default internos

Si no defines override, TrailBase usa:

- `BACKUP`: `@daily` (deshabilitado por defecto)
- `HEARTBEAT`: `17 * * * * * *`
- `LOG_CLEANER`: `@hourly`
- `AUTH_CLEANER`: `@hourly`
- `QUERY_OPTIMIZER`: `@daily`
- `FILE_DELETIONS`: `@hourly`

### 5.4 Validaciones

- Cada entrada requiere `id` y `schedule`.
- `schedule` debe ser cron válido según parser usado por TrailBase.

---

## 6) Bloque databases

Define bases adjuntas (adjacent db files) para usar en Record APIs.

```textproto
databases: [{ name: "tenant_a" }, { name: "analytics" }]
```

Validaciones:

- `name` requerido.
- Nombres reservados no permitidos: `main`, `logs`, `session`, `""`.
- Permitido solo: alfanumérico ASCII, `_`, `-`.
- No puede haber duplicados.

---

## 7) Bloque record_apis (la parte más crítica)

### 7.1 Campos

```textproto
record_apis: [
  {
    name: "todos"
    table_name: "todos"
    attached_databases: ["analytics"]

    conflict_resolution: ABORT
    autofill_missing_user_id_columns: false
    enable_subscriptions: true

    acl_world: [CREATE, READ, UPDATE, DELETE, SCHEMA]
    acl_authenticated: [READ]

    excluded_columns: ["internal_note"]

    create_access_rule: "_USER_.id = _REQ_.owner"
    read_access_rule: "_ROW_.owner = _USER_.id"
    update_access_rule: "_ROW_.owner = _USER_.id"
    delete_access_rule: "_ROW_.owner = _USER_.id"
    schema_access_rule: "_USER_.id IS NOT NULL"

    expand: ["project_id"]

    listing_hard_limit: 1024
  }
]
```

### 7.2 Enums

#### ConflictResolutionStrategy

- `CONFLICT_RESOLUTION_STRATEGY_UNDEFINED` (0)
- `ABORT` (1)
- `ROLLBACK` (2)
- `FAIL` (3)
- `IGNORE` (4)
- `REPLACE` (5)

#### PermissionFlag

- `PERMISSION_FLAG_UNDEFINED` (0)
- `CREATE` (1)
- `READ` (2)
- `UPDATE` (4)
- `DELETE` (8)
- `SCHEMA` (16)

### 7.3 Defaults y comportamiento

- `enable_subscriptions`: false si no se define.
- `autofill_missing_user_id_columns`: false si no se define.
- `listing_hard_limit`: si no se define, el hard limit global efectivo es 1024.
- Límite por defecto de list si no mandas `limit`: 50.

### 7.4 Validaciones de nombre y tabla/view

- `name` requerido, no vacío, solo alfanumérico ASCII o `_`.
- `table_name` requerido, debe parsear como nombre SQL válido.
- Puedes apuntar a TABLE o VIEW.
- TABLE/VIEW no puede ser TEMPORARY.
- TABLE debe ser STRICT.
- Debe existir una PK apta (INTEGER o UUID con constraints esperados por TrailBase).
- No se permiten dos APIs con el mismo `name`.

### 7.5 attached_databases

- Todos los nombres en `attached_databases` deben existir en `databases`.

### 7.6 excluded_columns

- Cada columna excluida debe existir.
- No puedes excluir la PK.
- No puedes excluir una columna NOT NULL sin DEFAULT (porque rompería inserts/updates).

### 7.7 expand

- Solo columnas explícitamente listadas aquí pueden expandirse por query.
- No puede empezar con `_` (columnas hidden).
- Debe ser columna existente.
- Debe ser FOREIGN KEY.
- La tabla referenciada no puede ser hidden (no `_...`).
- La tabla referenciada debe tener PK apta.
- No soporta referencias a PK compuesta para expand.

### 7.8 ACL + access rules

ACL:

- `acl_world`: permisos para público anónimo.
- `acl_authenticated`: permisos para usuario autenticado.

Rules SQL:

- `create_access_rule`
- `read_access_rule`
- `update_access_rule`
- `delete_access_rule`
- `schema_access_rule`

Las rules son expresiones SQL (se validan con `SELECT <expr>`). Deben usar placeholders en MAYUSCULA:

- `_USER_`
- `_REQ_`
- `_REQ_FIELDS_`
- `_ROW_`

Restricciones de placeholders por tipo:

- Create: no puede usar `_ROW_`.
- Read: no puede usar `_REQ_` ni `_REQ_FIELDS_`.
- Delete: no puede usar `_REQ_` ni `_REQ_FIELDS_`.
- Schema: no puede usar `_ROW_`, `_REQ_`, ni `_REQ_FIELDS_`.
- Update: puede usar `_ROW_`, `_REQ_`, `_REQ_FIELDS_`.

Regla especial para `_REQ_FIELDS_`:

- Debe usarse como: `'field_name' IN _REQ_FIELDS_` (LHS string literal).

Ejemplos válidos:

```text
_USER_.id = _REQ_.owner
_ROW_.owner = _USER_.id
'status' IN _REQ_FIELDS_
```

Ejemplos inválidos:

```text
_row_.owner = _USER_.id         # placeholder en minúscula
_REQ_.x = 1                     # en read/delete
field IN _REQ_FIELDS_           # lhs no literal string
```

---

## 8) Bloque schemas

Permite registrar JSON Schemas reutilizables para columnas/tipos JSON.

```textproto
schemas: [
  {
    name: "TaskMeta"
    schema: "{\"type\":\"object\",\"properties\":{\"priority\":{\"type\":\"integer\"}}}"
  }
]
```

Validaciones:

- `name` requerido.
- `schema` requerido.
- `schema` debe ser JSON válido.
- Debe pasar validación de meta-schema JSON Schema.

---

## 9) Secretos y redacción automática

TrailBase marca algunos campos como secretos en el proto:

- `email.smtp_password`
- `auth.oauth_providers[*].client_secret`
- `server.s3_storage_config.secret_access_key`

Al guardar config por admin APIs, esos secretos se separan a `secrets.textproto` (`config.Vault`) y en `config.textproto` se redaccionan como `"<REDACTED>"`.

Formato de `secrets.textproto`:

```textproto
secrets {
  key: "TRAIL_EMAIL_SMTP_PASSWORD"
  value: "real-secret"
}
```

---

## 10) Overrides por variables de entorno

TrailBase soporta override por variables `TRAIL_*` para campos escalares y secretos.

Patrón general:

- `TRAIL_<RUTA_DE_CAMPOS_EN_MAYUSCULAS>`
- Ejemplos:
  - `TRAIL_EMAIL_SMTP_USERNAME`
  - `TRAIL_EMAIL_SMTP_PASSWORD`
  - `TRAIL_SERVER_APPLICATION_NAME`
  - `TRAIL_AUTH_OAUTH_PROVIDERS_GOOGLE_CLIENT_SECRET`

Notas importantes:

- Para fields `string`, env var tiene prioridad sobre config/secrets.
- Repeated fields no tienen soporte general por env vars.
- En mensajes opcionales no inicializados, algunos env vars pueden no aplicarse por limitación actual del merge.

---

## 11) Config de ejemplo completa (plantilla)

```textproto
email {
  smtp_host: "smtp.mailgun.org"
  smtp_port: 587
  smtp_username: "postmaster@mg.example.com"
  smtp_password: "supersecret"
  smtp_encryption: SMTP_ENCRYPTION_STARTTLS
  sender_name: "Acme App"
  sender_address: "noreply@acme.com"
}

server {
  application_name: "Acme App"
  site_url: "https://app.acme.com"
  logs_retention_sec: 604800
  request_size_limit_bytes: 10485760
  enable_record_transactions: true
  auth_ip_rate_limit: 25
}

auth {
  auth_token_ttl_sec: 3600
  refresh_token_ttl_sec: 2592000
  enable_otp_signin: false
  password_minimal_length: 10
  password_must_contain_upper_and_lower_case: true
  password_must_contain_digits: true
  password_must_contain_special_characters: false

  oauth_providers {
    key: "google"
    value {
      provider_id: GOOGLE
      client_id: "google-client-id"
      client_secret: "google-client-secret"
    }
  }

  redirect_uri_allowlist: "https://app.acme.com/api/auth/v1/oauth/callback"
}

jobs {
  system_jobs: [
    { id: BACKUP schedule: "@daily" disabled: true },
    { id: LOG_CLEANER schedule: "@hourly" disabled: false }
  ]
}

databases: [{ name: "analytics" }]

record_apis: [
  {
    name: "todos"
    table_name: "todos"
    acl_world: [READ]
    acl_authenticated: [CREATE, READ, UPDATE, DELETE]
    enable_subscriptions: true
    listing_hard_limit: 1000
  },
  {
    name: "projects"
    table_name: "projects"
    acl_authenticated: [READ]
    expand: ["owner_id"]
  }
]

schemas: [
  {
    name: "TodoMeta"
    schema: "{\"type\":\"object\",\"properties\":{\"estimate\":{\"type\":\"integer\"}}}"
  }
]
```

---

## 12) Checklist rápida antes de arrancar

- `server.application_name` presente y válido.
- Si usas OAuth: `site_url` público correcto y providers bien configurados.
- Cada `record_api` apunta a TABLE STRICT o VIEW no temporal con PK apta.
- `access_rule` usa placeholders en mayúscula y compatibles con su operación.
- `expand` solo en FKs válidas hacia tabla no hidden con PK apta.
- `jobs.system_jobs` con cron válido.
- `schemas` con JSON Schema válido.

---

## 13) Referencias de código usadas

- `crates/core/proto/config.proto`
- `crates/core/src/config.rs`
- `crates/core/src/records/validate.rs`
- `crates/core/src/listing.rs`
- `crates/core/src/server/mod.rs`
- `crates/core/src/scheduler.rs`
- `crates/core/proto/vault.proto`
