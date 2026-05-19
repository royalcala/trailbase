# TrailBase SDK JS/TS para Frontend: capacidades completas

Este documento resume todo lo que puedes hacer en una app frontend (web) con el SDK oficial de TrailBase para JavaScript/TypeScript.

## 1. Instalación e inicialización

```bash
npm i trailbase
# o
pnpm add trailbase
# o
yarn add trailbase
```

### Inicializar cliente

```ts
import { initClient, initClientFromCookies } from "trailbase";

// Caso común local/dev
const client = initClient("http://localhost:4000");

// Caso recomendado para apps web con cookies/sesión previa
const clientFromCookies = await initClientFromCookies("http://localhost:4000");
```

### Opciones de cliente

- `tokens`: inyecta tokens persistidos (auth/refresh/csrf).
- `onAuthChange`: callback para sincronizar estado auth en tu store/frontend.
- `transport`: transporte HTTP custom (útil para SSR/testing).

## 2. Estado de sesión y usuario

Con el cliente puedes leer y administrar estado de sesión:

- `tokens()` devuelve `{ auth_token, refresh_token, csrf_token }` o `undefined`.
- `user()` devuelve `{ id, email, admin?, mfa? }` o `undefined`.
- `headers()` devuelve headers listos para requests autenticados.
- `checkCookies()` intenta convertir cookies válidas en tokens de sesión.
- `refreshAuthToken({ force? })` refresca token cuando está por vencer (o forzado).

## 3. Autenticación

### Login/password

```ts
const maybeMfaToken = await client.login("user@example.com", "secret");

// Si la cuenta usa MFA, devuelve token intermedio.
if (maybeMfaToken) {
  await client.loginSecond({
    mfaToken: maybeMfaToken,
    totpCode: "123456",
  });
}
```

### Login OTP por email

```ts
await client.requestOtp("user@example.com", { redirectUri: "https://miapp.com" });
await client.loginOtp("user@example.com", "123456");
```

### MFA/TOTP

- `registerTOTP({ png? })` inicia registro TOTP y devuelve URL/provisión.
- `confirmTOTP(totpUrl, totp)` confirma activación.
- `unregisterTOTP(totp)` desactiva MFA.

### Cierre y borrado de cuenta

- `logout()` cierra sesión actual.
- `deleteUser()` elimina usuario autenticado.

### Avatar

- `avatarUrl(userId?)` construye ruta de avatar del usuario actual o de otro usuario.

## 4. API de records (CRUD en tablas/vistas)

El acceso a datos se hace con:

```ts
const posts = client.records<Post>("posts");
```

Capacidades:

- `list(opts?)`
- `read(id, opts?)`
- `create(record)`
- `update(id, partial)`
- `delete(id)`

También puedes operar sobre vistas expuestas por TrailBase, no solo tablas.

## 5. Listado avanzado: filtros, orden, paginación y expand

`list` soporta:

- `pagination`: `limit`, `offset`, `cursor`
- `order`: por ejemplo `+created_at` o `-created_at`
- `count`: incluye `total_count`
- `expand`: expande relaciones/foreign refs
- `filters`: filtros simples y compuestos

### Operadores de filtro

- `equal`, `notEqual`
- `lessThan`, `lessThanEqual`
- `greaterThan`, `greaterThanEqual`
- `like`, `regexp`
- Geoespaciales: `@within`, `@intersects`, `@contains`

### Composición lógica

- `and: [...]`
- `or: [...]`

Ejemplo:

```ts
const result = await client.records("posts").list({
  filters: [
    {
      and: [
        { column: "status", op: "equal", value: "published" },
        { column: "title", op: "like", value: "%trailbase%" },
      ],
    },
  ],
  order: ["-created_at"],
  pagination: { limit: 20, offset: 0 },
  count: true,
});
```

## 6. Operaciones diferidas y ejecución por lotes

El SDK permite construir operaciones y ejecutarlas después:

- `createOp(record)`
- `updateOp(id, partial)`
- `deleteOp(id)`
- `readOp(id, opts?)`
- `listOp(opts?)`

Luego puedes ejecutar mutaciones en lote:

```ts
const api = client.records("posts");

const ids = await client.execute(
  [
    api.createOp({ title: "A" }),
    api.createOp({ title: "B" }),
  ],
  true, // transacción
);
```

También existe `createBulk(records)` para inserción masiva de una colección.

## 7. Realtime / suscripciones

Puedes escuchar cambios en tiempo real como stream:

- `subscribe(id, opts?)` para un registro puntual.
- `subscribeAll(opts?)` para toda la tabla/vista.

Opciones:

- `filters` para recibir solo eventos que cumplan criterio.
- `onLoss` callback para detectar pérdida de eventos (cliente o servidor).

Eventos posibles:

- `Insert`
- `Update`
- `Delete`
- `Error` (incluye estados como pérdida/forbidden)

Ejemplo:

```ts
const stream = await client.records("posts").subscribeAll();

for await (const ev of stream) {
  if ("Insert" in ev) {
    console.log("nuevo", ev.Insert);
  }
}
```

## 8. Archivos en records

El SDK soporta trabajar con columnas de archivo vía payload de records:

- Puedes enviar archivos codificados en base64/url-safe base64 como parte del `create`/`update`.
- Helpers para construir endpoints de descarga:
  - `filePath(apiName, recordId, columnName)`
  - `filesPath(apiName, recordId, columnName, fileName)`

Utilities útiles:

- `urlSafeBase64Encode(bytes)`
- `urlSafeBase64Decode(str)`

## 9. Geoespacial y GeoJSON

Para tablas/vistas con geometría:

- `listGeoOp(geometryColumn, opts?)` devuelve `FeatureCollection` GeoJSON.
- Puedes usar filtros `@within`, `@intersects`, `@contains`.

## 10. Fetch de bajo nivel con auth integrada

`client.fetch(path, init?)` permite llamar endpoints custom de TrailBase:

- Adjunta headers/tokens automáticamente.
- Intenta refrescar token cuando corresponde.
- Lanza excepción en respuestas no OK (por defecto).

Esto permite combinar API de alto nivel (`records`) con endpoints propios.

## 11. Manejo de errores

El SDK expone `FetchError`:

- `status`
- `message`
- `url`
- `isClient()` para 4xx
- `isServer()` para 5xx

Patrón recomendado:

```ts
import { FetchError } from "trailbase";

try {
  await client.records("posts").read("id-invalido");
} catch (err) {
  if (err instanceof FetchError) {
    console.error(err.status, err.message);
  }
}
```

## 12. Uso en frontend real (estado reactivo)

Patrón típico en apps web:

1. Inicializar con `initClientFromCookies`.
2. Persistir tokens (`localStorage`, store, cookie app-side según tu arquitectura).
3. Reaccionar a `onAuthChange` para reflejar login/logout en UI.
4. Consumir `records(...)` para CRUD y `subscribeAll` para realtime.

## 13. Qué NO está como API pública principal

- Hay muchos tipos generados en `bindings/`, pero no todos representan APIs frontend de alto nivel.
- Existen helpers internos para testing (`exportedForTesting`) que no deberías usar en producto.
- El flujo OAuth/OIDC se maneja principalmente por endpoints/rutas de auth del backend y redirecciones UI.

## 14. Referencia rápida (superficie pública práctica)

### Inicialización y utilidades

- `initClient(site?, opts?)`
- `initClientFromCookies(site?, opts?)`
- `urlSafeBase64Encode(bytes)`
- `urlSafeBase64Decode(str)`
- `filePath(apiName, recordId, columnName)`
- `filesPath(apiName, recordId, columnName, fileName)`

### Cliente

- `tokens()`, `user()`, `headers()`, `base`
- `records(name)`
- `avatarUrl(userId?)`
- `login`, `loginSecond`, `requestOtp`, `loginOtp`, `logout`
- `registerTOTP`, `confirmTOTP`, `unregisterTOTP`
- `deleteUser`, `checkCookies`, `refreshAuthToken`
- `fetch`
- `execute`

### Record API

- `list`, `listOp`, `listGeoOp`
- `read`, `readOp`
- `create`, `createOp`, `createBulk`
- `update`, `updateOp`
- `delete`, `deleteOp`
- `subscribe`, `subscribeAll`

---

Si quieres, puedo hacer una segunda versión de este documento orientada a un stack específico (React, Vue, Svelte, Solid o Next.js) con arquitectura recomendada de auth, cache y realtime para producción.
