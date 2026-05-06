# Syntrix Plan (Simplificado con TrailBase)

## Objetivo
Construir Syntrix con el menor costo de complejidad posible usando:

- TrailBase como backend principal (auth + APIs + ACL + realtime).
- SQLite como base de datos.
- Frontend SPA con TanStack (Router + Query, y TanStack DB opcional para estado local avanzado).

---

## Stack final simplificado

### Backend
- TrailBase.
- SQL migrations en `traildepot/migrations/main`.
- `config.textproto` para Record APIs, ACL y JSON schemas.
- WASM endpoints solo para casos especiales (queries/reportes que no entren en Record APIs).

### Frontend
- Vite + React.
- TanStack Router.
- TanStack Query.
- Cliente TypeScript de TrailBase para auth/CRUD/realtime.
- shadcn/ui como sistema de componentes.
- Admin App con `Sidebar` base de shadcn:
	https://ui.shadcn.com/docs/components/base/sidebar

### Infra
- NixOS-friendly (devshell/flake opcional).
- Reverse proxy (Nginx/Caddy) en producción.

---

## Decisión de arquitectura

Primera versión:

1. **SPA independiente** (frontend separado) + TrailBase como backend.
2. **Multitenancy por fila** al inicio (`tenant_id` + access rules).
3. Escalar luego a **DB por tenant** o **instancia por tenant** solo cuando haya necesidad real.

Esto reduce muchísimo complejidad comparado con mantener un backend custom completo desde el día 1.

### Decisión de login (importante)

- TrailBase **sí** trae login UI para su **admin dashboard** (`/_/admin`).
- Para la **app de Syntrix** (nuestro panel/producto), usaremos login propio en frontend con shadcn/ui.
- El login propio consumirá APIs de auth de TrailBase (no inventar auth aparte).
- Conclusión: no usar "blocks" por obligación; usar shadcn como base visual y adaptar a UX de Syntrix.

---

## Estructura sugerida (simple)

```text
/syntrix
	/apps
		/web
			/src
			package.json
			vite.config.ts

	/backend
		/traildepot
			config.textproto
			/migrations
				/main
			/uploads
			/wasm        # opcional (solo endpoints custom)

	/infra
		docker-compose.yml   # opcional
		nginx.conf           # opcional
```

---

## ACL y multitenancy (regla base)

### Modelo mínimo
- Cada tabla de negocio incluye `tenant_id` y `owner_user`.
- Tabla de membresías: `tenant_members (tenant_id, user_id, role)`.

### Regla base de acceso
- `create_access_rule`: usuario pertenece al tenant del request.
- `read/update/delete_access_rule`: usuario pertenece al tenant de la fila.
- Para acciones sensibles, agregar verificación de `role = 'admin'`.

En resumen: permisos por SQL con `EXISTS(...)` contra tabla de membresías.

---

## Plan de implementación (4 fases)

## Fase 1 - Base funcional
1. Inicializar `traildepot`.
2. Crear migraciones iniciales (`users/tenants/memberships` + 1 tabla de negocio).
3. Exponer primera Record API en `config.textproto`.
4. Configurar ACL por tenant.
5. Levantar SPA con login + listado CRUD básico.

Resultado esperado: app usable con auth + CRUD multi-tenant básico.

## Fase 2 - Frontend productivo
1. Integrar TanStack Router.
2. Integrar TanStack Query para fetch/cache/invalidation.
3. Conectar realtime de TrailBase para updates en vivo.
4. Manejar `onLoss` con estrategia de resync (`list` + re-subscribir).
5. Construir layout de Admin App con `Sidebar` de shadcn + navegación por módulos.
6. Implementar pantalla de login propia con shadcn conectada a auth de TrailBase.

Resultado esperado: UX moderna con datos reactivos.

## Fase 3 - JSON schemas y validación fuerte
1. Registrar `schemas` en `config.textproto` (si hay campos JSON complejos).
2. Agregar `CHECK(jsonschema(...))` en columnas JSON.
3. Regenerar tipos de cliente desde schema cuando cambie DB.

Resultado esperado: contrato de datos consistente end-to-end.

## Fase 4 - Casos avanzados
1. Agregar endpoints WASM para reportes SQL complejos (joins/group by dinámicos).
2. Evaluar multitenancy avanzado:
	 - seguir row-level,
	 - o migrar tenants grandes a DB/instancia dedicada.

Resultado esperado: escalado por necesidad, sin sobre-ingeniería temprana.

---

## Qué NO hacer al inicio

1. No implementar motor de sync propio si TrailBase ya cubre el caso.
2. No separar por instancia por tenant desde día 1.
3. No crear endpoints WASM para todo: primero usar Record APIs.
4. No meter DB por usuario salvo requerimiento de compliance extremo.

---

## Checklist MVP (ultra corto)

- Login funcionando.
- Layout admin con sidebar funcionando.
- Un tenant por usuario inicial.
- CRUD de una entidad con ACL por tenant.
- Realtime de esa entidad funcionando.
- Deploy básico con backup de SQLite.

Cuando esto esté estable, recién avanzar a módulos complejos.

---

## Referencias

- Flujo funcional simplificado de la app:
	[docs/testing-repo-rao/syntrix-app-flow-trailbase.md](docs/testing-repo-rao/syntrix-app-flow-trailbase.md)
- Documento previo de referencia histórica:
	[docs/testing-repo-rao/DEPRECATED/plan-app-flow-markmap[DEPRECATED.md](docs/testing-repo-rao/DEPRECATED/plan-app-flow-markmap[DEPRECATED.md)
