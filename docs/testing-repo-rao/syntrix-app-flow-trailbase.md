---
title: Syntrix — App Flow (TrailBase)
markmap:
  colorFreezeLevel: 2
---

# Syntrix App Flow (Simplificado)

> Este documento reemplaza el enfoque anterior de flujo y lo adapta al stack real:
> TrailBase + SPA (TanStack) + ACL SQL + multitenancy.

---

## 1. Objetivo del flujo

Definir el flujo funcional de la app sin sobre-ingenieria:

- Auth y sesiones con TrailBase.
- Tenant activo por usuario.
- CRUD por modulos usando Record APIs.
- Permisos con ACL + access rules SQL.
- Realtime para listas/detalles importantes.
- Endpoints WASM solo para casos especiales (reportes/consultas complejas).

---

## 2. Mapa de rutas (MVP)

## Publicas

- `/login`
- `/register` (opcional en MVP, puede salir despues)

## Privadas (requieren sesion)

- `/orgs` (seleccion de tenant)
- `/dashboard`
- `/org/settings`
- `/org/members`
- `/modules/inmuebles/properties`
- `/modules/inmuebles/contracts`
- `/modules/inmuebles/payments`

## Opcionales (fase posterior)

- `/audit/access`
- `/audit/records`
- `/reports/*` (normalmente respaldadas por WASM endpoints)

---

## 3. Flujo principal de usuario

## 3.1 Login

1. Usuario entra a `/login`.
2. Frontend llama auth API de TrailBase.
3. Si credenciales validas:
   - guarda sesion/tokens,
   - redirige a `/orgs`.
4. Si falla: muestra error.

Nota:
- El login UI de TrailBase en `/_/admin` se usa para consola admin de TrailBase.
- Syntrix usa login propio (shadcn/ui) conectado a auth de TrailBase.

## 3.2 Seleccion de tenant

1. En `/orgs`, frontend lista tenants donde el usuario es miembro.
2. Usuario selecciona tenant.
3. App guarda `activeTenantId` en session state.
4. Redirige a `/dashboard`.

## 3.3 Dashboard

1. Muestra modulos habilitados.
2. Sidebar (shadcn base sidebar) con navegacion por modulo.
3. Si no hay `activeTenantId`, redirect automatico a `/orgs`.

## 3.4 CRUD de modulo (ejemplo: properties)

1. Listado: GET con filtro por tenant.
2. Crear: POST con `tenant_id` del tenant activo.
3. Editar: PATCH por `id`.
4. Eliminar: DELETE por `id`.
5. Realtime: subscribe de tabla o registro.

---

## 4. Modelo de permisos (ACL)

## Base

- `acl_authenticated`: operaciones permitidas a usuarios con sesion.
- `access_rules`: restricciones por fila usando SQL.

## Regla patron

- `create_access_rule`: usuario debe pertenecer al tenant enviado en `_REQ_.tenant_id`.
- `read/update/delete_access_rule`: usuario debe pertenecer al tenant de `_ROW_.tenant_id`.

## Ejemplo mental

- Miembro puede leer/escribir sus datos del tenant.
- Solo admin puede gestionar miembros del tenant.

---

## 5. Flujo de datos recomendado

## Tablas base

- `tenants`
- `tenant_members (tenant_id, user_id, role)`
- Tablas de negocio (`properties`, `contracts`, `payments`) con `tenant_id`.

## Regla de consistencia

- Toda entidad de negocio incluye `tenant_id`.
- Toda lectura/escritura sensible pasa por access rules.
- Nada de permisos solo en frontend.

---

## 6. Realtime (sin motor custom)

Uso recomendado:

1. Cargar snapshot inicial con `list`.
2. Abrir `subscribeAll` o `subscribe(id)`.
3. Si hay `Loss` o gap de secuencia:
   - recargar snapshot,
   - re-suscribir.

Esto evita implementar sync propietario en MVP.

---

## 7. Cuándo usar WASM endpoints

Usar WASM solo si Record APIs no alcanzan:

- Reportes con JOIN/GROUP BY dinamicos.
- Endpoints agregados complejos.
- Casos de negocio que no encajan en CRUD directo.

No usar WASM para CRUD basico que ya resuelve Record API.

---

## 8. Fases del flujo (ejecucion)

## Fase A (MVP)

- Login/logout.
- Seleccion de tenant.
- Dashboard + sidebar.
- CRUD de 1 entidad (properties) con ACL.
- Realtime en listado de esa entidad.

## Fase B

- Miembros de tenant (roles admin/member).
- CRUD de contracts y payments.
- JSON schema constraints para campos complejos.

## Fase C

- Auditoria basica.
- Reportes via WASM endpoint cuando aplique.
- Hardening de cache/proxy/observabilidad.

---

## 9. Definiciones de listo (DoD) por flujo

## Auth listo

- Login funciona.
- Logout limpia sesion.
- Rutas privadas protegidas.

## Tenant listo

- Usuario puede seleccionar tenant.
- Sin tenant activo no entra a modulos.

## CRUD listo

- Crea/lee/edita/elimina en tenant activo.
- Usuario de otro tenant no accede (403 o no visible).

## Realtime listo

- Cambios se reflejan sin refresh.
- Recuperacion correcta ante `Loss`.

---

## 10. No objetivos (por ahora)

- Motor de sync propio completo.
- DB por usuario desde inicio.
- Instancia por tenant desde inicio.
- SSR full app desde inicio.

Se pueden evaluar despues segun carga y necesidades de producto.
