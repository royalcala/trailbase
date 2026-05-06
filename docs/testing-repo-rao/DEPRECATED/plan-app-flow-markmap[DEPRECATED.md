---
title: Syntrix — Flujo UI
markmap:
  colorFreezeLevel: 2
---

# Syntrix

> Nota de metodología: este documento es para mapear el flujo y el estado funcional de la app (vista ejecutiva). Los planes de implementación detallados (fases, tareas y diseño técnico) deben vivir en documentos separados dentro de `docs/`.

## 🔓 /login ✅
> `syntrix-auth` — sin token

- ✅ Form → POST `/api/auth/login`
- ✅ Respuesta: `{ access_token, refresh_token, user_id }`
- ✅ guarda tokens en `sessionStore` → `/orgs`
- ✅ muestra error → reintentar | → `/register`

### /register ✅
- ✅ Form → POST `/api/auth/register`
- ✅ → misma sesión → `/orgs`

### Auth: ¿qué es User?
- ✅ Struct en `syntrix-auth/src/model.rs` — **no es un doc Loro**
- ✅ Gestionado por el servidor (argon2, JWT)
- ✅ `OrgMembership { org_id, role }` vive dentro del User
- ✅ Roles: `Owner | Admin | Agent | Viewer`

---

## 🏛️ /orgs ✅
> Token válido, sin `activeOrgId`

- ✅ GET `/api/orgs` → lista de orgs del usuario
- ✅ Seleccionar → `sessionStore.setActiveOrg(orgId)` → `/dashboard`
- ⚠️ Org muestra solo `org_id` (backend no devuelve `name` en `/api/orgs`)

### OrganizationDoc (Loro) ✅
- ✅ Crate: `crates/core/src/domain/organization.rs`
- ✅ **Loro map** `"organization"`: `id`, `name`, `address`, `contact_email`, `max_properties: i64`
- ✅ **Loro text** `"legal_info"` ← editable colaborativamente
- ✅ OPFS: `tenant/{org}/doc/organization.loro`
- ✅ Relay key: `tenant/{org}/module/platform/doc/organization/{id}/snapshot/latest`
- ✅ Raíz de todo — todos los módulos dependen del `org.id`

### ReBAC — organization ✅
- ✅ `owner: [user]`
- ✅ `admin: [user]` ← incluye `owner`
- ✅ `member: [user]` ← incluye `admin`

---

## 👤 Avatar / dropdown de usuario ✅
> Visible en toda la app (navbar) — requiere token válido

- ✅ Muestra avatar (iniciales) + dropdown activo
- ✅ Dropdown: → `/profile` | → `/logout`

### /profile ✅
- ✅ GET `/api/auth/me` → `{ user_id, email, full_name }`
- ✅ PATCH `/api/auth/me` → actualizar `full_name`, contraseña
- ✅ UI: `ProfilePage` — muestra email (readonly), edita nombre + contraseña
- Sin docs Loro — datos de auth server-side
- ReBAC: ninguno (solo propio usuario)

---

## 🏢 Org switcher / dropdown de org ✅
> Visible en toda la app (navbar) — requiere `activeOrgId`

- ✅ Muestra nombre de org activa + ícono
- ✅ Dropdown: lista orgs | `/org/settings` | `/org/members` | `/orgs/new`

### /org/settings ⚠️
- ✅ UI: `OrgSettingsPage` — muestra Org ID
- ⚠️ PATCH `/api/orgs/{org_id}` → pendiente en servidor
- ✅ **OrganizationDoc (Loro)** — mismo doc que en `/orgs`
- ✅ ReBAC: `organization` → `can_admin` ← `admin`

### /org/members ✅
- ✅ GET `/api/orgs/{org_id}/members`
- ✅ POST `/api/orgs/{org_id}/members` → invitar `{ email, role }`
- ✅ PATCH `/api/orgs/{org_id}/members/{user_id}` → cambiar rol
- ✅ DELETE `/api/orgs/{org_id}/members/{user_id}` → remover
- ✅ UI: `OrgMembersPage` — lista miembros, selector de rol, botón remover
- ✅ ReBAC model: `organization` → `can_admin` ← `admin`

### /orgs/new ✅
- ✅ POST `/api/orgs` (actual: UI/envía `name`; servidor crea org por `user_id` y hoy ignora campos extra)
- ✅ UI: `NewOrgPage` — form con nombre → crea + setea `active_org_id` → `/dashboard`
- ⚠️ Pendiente robustecer create-org para persistir/retornar metadatos de organización (`name`, `address`, `contact_email`)

---

## 🏢 /dashboard ✅
> Guard: `session.activeOrgId` presente

- ✅ Cards de módulos habilitados (web-v2)
- ✅ Fallback de módulos estáticos cuando `platform.modules` viene vacío
- ✅ Módulos registrados en `syntrix-api` via feature flags
- Sin check ReBAC (solo estar autenticado)

---

## 📋 /audit ✅
> Guard: `session.activeOrgId` + ReBAC `audit_log → can_read`

- ✅ Submenú colapsable en sidebar (Accesos, Documentos)

### /audit ✅
- ✅ Hub de auditoría con accesos rápidos a `access` y `docs`

### /audit/access ✅
- ✅ GET `/api/audit/access` — solo lectura
- ✅ Sin docs Loro
- ✅ ReBAC: `audit_log` → `viewer: [user]`, `can_read` ← `viewer`

### /audit/docs ✅ — Inspector de documentos Loro
- ✅ Plan detallado: [docs/plan-audit-docs-loro-inspector.md](plan-audit-docs-loro-inspector.md)
- ✅ Catálogo OPFS/fallback por módulo/tipo + filtros
- ✅ Vista detalle con tabs `State`, `History`, `DAG`, `Relay`
- ✅ Comparación local vs relay mediante endpoint `GET /api/audit/docs/relay/snapshot`

#### /audit esperado

##### /audit/access
- Eventos de acceso solo lectura
- Filtros por usuario/recurso/fecha

##### /audit/docs
- Catalogo de docs Loro por modulo
  - platform
  - inmuebles
  - contabilidad
- Apertura de documento
  - State: estado actual del doc
  - History: timeline de cambios
  - DAG: grafo de dependencias
- Seguridad
  - Guard por `activeOrgId`
  - ReBAC `audit_log.can_read`

---

## 🏠 /modules/inmuebles ✅
> Crate: `modules/inmuebles/core/` ✅

- ✅ Navegación en sidebar + submenú colapsable (Inicio, Propiedades, Contratos, Pagos)

### /propiedades ⚠️
- ✅ **PropertyDoc (Loro)** — struct + snapshot/import implementados
  - ✅ **Loro map** `"property"`: `id`, `name`, `address`, `status`, `rent_price_cents: i64`
  - ✅ `status`: `"draft"` | `"available"` | `"rented"`
  - ✅ OPFS: `tenant/{org}/module/inmuebles/doc/property/{id}.loro`
  - ✅ Relay key: `tenant/{org}/module/inmuebles/doc/property/{id}/snapshot/latest`
- ✅ **ReBAC** — `property` model definido en OpenFGA
- ✅ API inmuebles: `GET/POST /properties`, `PUT/DELETE /properties/:id`
- ✅ UI web-v2 actual: listado + alta + edición + eliminación (CRUD completo)

#### Storage de propiedades — estado actual vs propuesto

- ⚠️ **Estado actual (runtime de módulo API):**
  - La lista de propiedades se sirve desde memoria en `modules/inmuebles/api/src/lib.rs` (`PROPERTIES: RwLock<Vec<PropertyRecord>>`).
  - El CRUD de `/inmuebles/properties` hoy no usa `PropertyDoc` como source of truth en ese endpoint.
- ✅ **Estado actual (modelo de dominio):**
  - `PropertyDoc` sí existe en `modules/inmuebles/core/` y soporta snapshot/import Loro.
  - OPFS/relay path por propiedad está definido para inspección/sync.

- ✅ **Propuesta recomendada (source of truth en docs Loro):**
  1. **Un doc por propiedad**: `tenant/{org}/module/inmuebles/doc/property/{id}.loro`.
  2. **Un doc índice por org** (nuevo): `tenant/{org}/module/inmuebles/doc/property_index/main.loro`.
     - Contiene lista resumida: `{ id, name, status, address, rent_price_cents, updated_at }`.
  3. **Listado** de `/propiedades` lee `property_index`.
  4. **Detalle/edición** lee y guarda `property/{id}.loro`.
  5. **Alta** crea `property/{id}` y agrega entrada al `property_index`.
  6. **Eliminación** remueve del `property_index` (y opcionalmente marca tombstone en doc individual).

- ✅ Esta propuesta mantiene el patrón esperado: doc individual por entidad + doc agregador por organización para consultas de lista.

### /contratos ⚠️
- ✅ **ContractDoc (Loro)** — struct + snapshot/import implementados
  - ✅ **Loro map** `"contract"`: `id`, `title`, `status`
  - ✅ `status`: `"draft"` | `"active"` | `"terminated"`
  - ✅ **Loro text** `"clauses"` ← texto colaborativo (multi-cursor)
  - ✅ OPFS: `tenant/{org}/module/inmuebles/doc/contract/{id}.loro`
- ✅ **ReBAC** — `contract` model definido en OpenFGA
- ✅ API inmuebles: `GET/POST /contracts`, `PUT/DELETE /contracts/:id`
- ✅ UI web-v2 actual: listado + alta + edición + eliminación (CRUD completo)
- ✅ Estados alineados: `draft|active|terminated` en API y UI; normalización de `signed→active` en backend
- ⚠️ Runtime de módulo API actual usa `CONTRACTS: RwLock<Vec<ContractRecord>>` (aún no usa `ContractDoc` como source of truth)

### /pagos ⚠️
- ✅ **PaymentDoc (Loro)** — struct + snapshot/import implementados
  - ✅ **Loro map** `"payment"`: `id`, `contract_id`, `tenant_id`, `amount_cents: i64`, `due_date`, `status`
  - ✅ `status`: `"pending"` | `"paid"` | `"overdue"`
  - ✅ **Loro text** `"notes"`
  - ✅ OPFS: `tenant/{org}/module/inmuebles/doc/payment/{id}.loro`
- ✅ **TenantDoc (Loro)** — struct + snapshot/import implementados
  - ✅ **Loro map** `"tenant"`: `id`, `name`, `email`, `phone`, `identification_number`, `status`
  - ✅ OPFS: `tenant/{org}/module/inmuebles/doc/tenant/{id}.loro`
- ✅ **ReBAC** — `payment` model definido en OpenFGA
- ✅ API inmuebles: `GET/POST /payments`, `PUT/DELETE /payments/:id`
- ✅ UI web-v2 actual: listado + alta + edición + eliminación (CRUD completo)
- ✅ Payload alineado: `PaymentRecord` ahora incluye `due_date` y `notes` en API y UI/tipos
- ⚠️ Runtime de módulo API actual usa `PAYMENTS: RwLock<Vec<PaymentRecord>>` (aún no usa `PaymentDoc` como source of truth)

### Árbol de dependencias (Inmuebles)
- `OrganizationDoc` ✅
  - → `PropertyDoc` ✅
    - → `ContractDoc` ✅
      - → `PaymentDoc` ✅ ← `TenantDoc` ✅

---

## 📊 /modules/contabilidad ⚠️
> Crate: `modules/contabilidad/` (router/API) + `modules/contabilidad/core/` (dominio Loro)

- ✅ Navegación en sidebar + submenú colapsable (Inicio, Cuentas, Asientos, Balance)

### /cuentas ⚠️
- ✅ **CuentaDoc (Loro)** — struct implementado en Rust
  - ✅ **Loro map** `"cuenta"`: `id`, `codigo`, `nombre`, `tipo`, `saldo_inicial: i64`
  - ✅ OPFS: `tenant/{org}/module/contabilidad/doc/cuenta/{id}.loro`
  - ✅ Relay key: `tenant/{org}/module/contabilidad/doc/cuenta/{id}/snapshot/latest`
- ✅ API contabilidad: `GET /contabilidad/cuentas`
- ✅ UI web-v2 (listado)
- ⚠️ Runtime actual lista cuentas demo en memoria/instanciación al vuelo (no lectura desde snapshots Loro persistidos)

### /asientos ⚠️
- ✅ **AsientoDoc (Loro)** — struct implementado en Rust (`modules/contabilidad/core/src/lib.rs`)
- ✅ API contabilidad: `GET/POST /contabilidad/asientos`
- ✅ UI web-v2 base (listado + alta)
- ✅ UX/API alineados: el campo `id` en el formulario es opcional y el backend autogenera si llega vacío.
- ⚠️ Runtime actual persiste asientos en memoria (`ASIENTOS: RwLock<Vec<AsientoPayload>>`) y no en snapshots Loro por doc

### /balance ⚠️
- ⚠️ Vista calculada en memoria — sin doc propio
- ⚠️ Se calcula sobre `ASIENTOS` en memoria (agregación de debe/haber), no sobre lectura de docs Loro versionados por período
- ✅ UI web-v2 base (tarjetas de resumen)

---

## 🔄 Ciclo Loro + Sync — [📄 ver doc detallado](sync-loro-relay.md)
> Aplica a todos los docs de todos los módulos

---

## 🔒 ReBAC — CheckRequest (cómo se usa)
> `crates/authz/` + `docs/openfga/model.fga`

- Antes de cargar o editar cualquier doc → `OpenFgaClient.check(request)`
- `{ user: "user:{uuid}", relation: "can_write", object: "property:{uuid}" }`
- ✅ permitido → carga doc desde OPFS
- ❌ denegado → error 403, nunca llega al doc
- `ResourceType`: `Organization | Property | Contract | Payment | Tenant | User`
- `Relation`: `Owner | Admin | Agent | Viewer | Member | Can(String)`

---

## 🏗️ Estructura de crates
- `crates/core/` — plataforma: `OrganizationDoc`, `ids`, `errors`
- `crates/auth/` — `User` struct (no Loro, server-side)
- `crates/authz/` — OpenFGA client + model
- `crates/storage/` — `RelayStore` (fjall), keys con `module_id`
- `crates/sync/` — WebTransport relay, protocolo `JoinDoc / ChangeBatch`
- `modules/inmuebles/core/` — `PropertyDoc`, `ContractDoc`, `PaymentDoc`, `TenantDoc`
- `modules/contabilidad/` — módulo API+router de contabilidad (hoy en un solo crate)
- `modules/contabilidad/core/` — `CuentaDoc`, `AsientoDoc`, `PresupuestoDoc`
- Módulo nuevo → idealmente `modules/{nombre}/core|api|web`, con posibilidad de arranque incremental en un solo crate

---

## 🚪 Cierre de sesión
- `sessionStore.clear()` → borra tokens + `activeOrgId` → `/login`
