# Decision Final: Stack y Monorepo Soberano

## Estado
Decision final acordada para avanzar implementacion.

---

## 1) Stack final

### Backend
- Rust
- Axum
- Turso/Limbo (libSQL) como data plane
- Sync local-first multi-tenant (DB por tenant)
- Cedar Policy embebido para autorizacion

### Frontend
- Vite + React (SPA)
- TanStack Router
- TanStack React Query
- `@libsql/client` (TypeScript)
- Drizzle ORM (esquema y migraciones)
- Runtime declarativo (DSL JSON + JSON Schema)

### Infra y DX
- Nix Flakes (NixOS-friendly devshell reproducible)
- Workspaces nativos (`pnpm-workspace.yaml`)
- `just` como orquestador de tareas
- Sin servidor Node.js en produccion

---

## 2) Principios de arquitectura

1. Soberania total de infraestructura.
2. Costos operativos minimos (sin dependencia de servicios externos innecesarios).
3. Multi-tenant real con aislamiento por DB de tenant para datos de negocio.
4. Identidad/membresias en control plane minimo compartido.
5. Cliente offline-first con sync y reconciliacion.
6. Validacion y autorizacion siempre en backend.
7. Cliente no confiable: ninguna DB local ni la UI son autoridad.
8. Contratos declarativos validados; no se ejecuta codigo arbitrario de usuario/IA.
9. Un usuario puede pertenecer a multiples orgs (tenants) y gestionar invitaciones segun permisos.

---

## 3) Estructura final del monorepo

```text
/syntrix
 │
 ├── flake.nix
 ├── Justfile
 ├── pnpm-workspace.yaml
 ├── .gitignore
 │
 ├── /packages
 │    └── /db-schema
 │         ├── package.json
 │         ├── drizzle.config.ts
 │         ├── /src
 │         │    └── schema.ts
 │         └── /migrations
 │
 └── /apps
      ├── /backend
      │    ├── Cargo.toml
      │    ├── /migrations
      │    └── /src
      │         ├── main.rs
      │         ├── db.rs
      │         ├── auth.rs
      │         ├── write_pipe.rs
      │         ├── policy.rs
      │         ├── runtime_api.rs
      │         ├── runtime_mcp.rs
      │         └── sync.rs
      │
       └── /frontend
         ├── package.json
         ├── vite.config.ts
         ├── tsconfig.json
         └── /src
           ├── main.tsx
           ├── db.ts
           ├── /routes
           ├── /hooks
           ├── /runtime
           └── /components
             ├── /ui
             └── /ai
```

### Minimo funcional no negociable
- `tenant_shared.db`: replica local compartida del estado aprobado del tenant.
- `user_workspace.db`: drafts, outbox, errores de validacion y capacidades efectivas del usuario.
- Data plane por tenant: snapshots + `change_history` + `write_idempotency`.
- Estado canonico por entidad en una tabla `entities` (snapshot actual), con historial append-only en `change_history`.
- Control plane compartido: `users`, `sessions`, `tenant_memberships`.
- Soporte de multi-org e invitaciones: `users`, `tenants`, `tenant_memberships`, `tenant_invitations`.

---

## 4) Configuracion clave

### `pnpm-workspace.yaml`
```yaml
packages:
  - 'apps/frontend'
  - 'packages/*'
```

### `packages/db-schema/package.json`
```json
{
  "name": "@mi-proyecto/db-schema",
  "version": "1.0.0",
  "main": "src/schema.ts",
  "dependencies": {
    "drizzle-orm": "latest"
  },
  "devDependencies": {
    "drizzle-kit": "latest"
  }
}
```

### `apps/frontend/package.json`
```json
{
  "name": "frontend",
  "dependencies": {
    "@mi-proyecto/db-schema": "workspace:*",
    "@libsql/client": "latest",
    "@tanstack/react-query": "latest",
    "@tanstack/react-router": "latest"
  }
}
```

### `Justfile`
```just
dev:
    @echo "🚀 Iniciando Arquitectura Local-First..."
    just --parallel dev-frontend dev-backend

dev-frontend:
    cd apps/frontend && pnpm run dev

dev-backend:
    cd apps/backend && cargo run

db-generate:
    @echo "📦 Generando nuevo esquema SQL..."
    cd packages/db-schema && pnpm drizzle-kit generate
    @echo "🔄 Copiando migraciones al backend..."
    cp -r packages/db-schema/migrations/* apps/backend/migrations/
    @echo "✅ Listo."
```

### Contrato minimo de write pipe
- Concurrencia por CAS: `base_version` debe coincidir para aplicar write.
- Idempotencia persistida por `(tenant_id, actor_user_id, idempotency_key)`.
- Respuestas cacheadas de idempotencia para `200`, `403` y `409`.
- Delta canonico: JSON Patch.
- Validaciones obligatorias antes de persistir: sesion/token, tenant activo, membership activa, reglas Cedar y JSON Schema del tipo de entidad/artefacto.
- Escritura autoritativa: el write aplica sobre `entities` y registra evento en `change_history` dentro de una transaccion atomica.

### Contrato minimo de read-only pipe
- El read pipe nunca promueve escrituras de negocio; solo entrega estado aprobado y proyecciones.
- Pull incremental por `change_seq` para estado compartido del tenant hacia `tenant_shared.db`.
- Pull privado por usuario para capacidades, errores de validacion y estados pendientes hacia `user_workspace.db`.
- Filtros por `tenant_id`, `user_id` y permisos efectivos (Cedar) antes de exponer datos.
- Respuesta deterministica y reintentos idempotentes para clientes offline-first.

### Runtime y APIs derivadas
- Rutas estaticas minimas: `/login`, `/select-tenant`, `/app`, `/app/*`.
- Rutas de negocio en `runtime_routes` (tenant DB) resueltas en runtime.
- Artefactos declarativos (`module_definition`, `entity_schema`, `page_definition`, `form_definition`, `action_definition`, `api_contract_definition`, `mcp_tool_definition`) validados con JSON Schema.
- Restriccion AI-first: no JSX/TS arbitrario, no SQL libre y no plugins MCP ejecutables sin sandbox.
- Exposicion HTTP/MCP derivada de contratos publicados y permisos Cedar.

---

## 5) Decision operativa

Esta arquitectura se toma como baseline para implementacion.

No se cambia de stack salvo que aparezca una restriccion tecnica medible que el baseline no pueda resolver.

Siguientes pasos:
1. Inicializar estructura real de carpetas.
2. Definir esquema inicial en `packages/db-schema/src/schema.ts`.
3. Levantar backend Axum con health + auth + sync base.
4. Implementar write pipe (`200/403/409`) con CAS + idempotencia persistida.
5. Levantar frontend con `/app/*` y resolucion de rutas dinamicas por tenant.
6. Habilitar runtime declarativo (JSON Schema) para UI/API/MCP sin codigo arbitrario.
7. Conectar sync y primer flujo CRUD offline-first.