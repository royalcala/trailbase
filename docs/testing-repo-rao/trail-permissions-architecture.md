# TrailBase - Patron de Arquitectura para Permisos (tabla + fila)

Este documento propone un patron practico para controlar permisos de usuarios en una app con TrailBase, incluyendo:
- Permisos por operacion (CREATE/READ/UPDATE/DELETE)
- Restricciones por fila (row-level)
- Roles y capacidades
- Evitar escalacion de privilegios

Basado en los mecanismos nativos de TrailBase en esta repo.

## 1) Modelo de autorizacion en TrailBase

TrailBase combina dos capas:

1. ACL basica por API
- `acl_world`: permisos para cualquier usuario (anonimo o autenticado)
- `acl_authenticated`: permisos para usuarios autenticados

2. Access Rules SQL por operacion
- `create_access_rule`
- `read_access_rule`
- `update_access_rule`
- `delete_access_rule`

Regla de evaluacion:
- Primero pasa ACL
- Luego se evalua la regla SQL (si existe)

Variables inyectadas en reglas:
- `_USER_.id`: usuario autenticado actual (o `NULL`)
- `_REQ_`: payload de request (CREATE/UPDATE)
- `_ROW_`: fila objetivo (READ/UPDATE/DELETE)

## 2) Patron recomendado (en capas)

### Capa A: Permiso macro por API

Define en ACL lo minimo indispensable:
- APIs publicas: `acl_world: [READ]`
- APIs privadas: solo `acl_authenticated` con operaciones necesarias

Ejemplo:

```protobuf
{
  name: "articles"
  table_name: "articles"
  acl_world: [READ]
  acl_authenticated: [CREATE, UPDATE, DELETE]
}
```

### Capa B: Permiso por fila (owner o tenant member)

Usa reglas SQL para ownership y tenancy.

Patron owner:

```sql
create_access_rule: "_REQ_.author = _USER_.id"
read_access_rule:   "_ROW_.author = _USER_.id"
update_access_rule: "_ROW_.author = _USER_.id"
delete_access_rule: "_ROW_.author = _USER_.id"
```

Patron tenant/member:

```sql
create_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _REQ_.org AND user = _USER_.id)"
read_access_rule:   "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
update_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
delete_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
```

### Capa C: Roles y capacidades

Agrega rol para operaciones sensibles (ej. admin/editor).

Patron admin por tenant:

```sql
create_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _REQ_.org AND user = _USER_.id AND role = 'admin')"
update_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id AND role = 'admin')"
delete_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id AND role = 'admin')"
```

Patron capability table (mas fino):

```sql
EXISTS(
  SELECT 1
  FROM user_capabilities uc
  WHERE uc.user_id = _USER_.id
    AND uc.capability = 'articles:publish'
)
```

## 3) Arquitectura sugerida por tipo de tabla

### 3.1 Tablas de negocio principales

Ejemplos: `projects`, `properties`, `orders`, `tasks`

Recomendacion:
- ACL: autenticados para CRUD (o solo R/C segun caso)
- Regla por fila con tenant o owner
- Si hay accion destructiva, exigir rol admin/editor

### 3.2 Tablas de autorizacion

Ejemplos: `org_members`, `roles`, `user_capabilities`

Recomendacion:
- Nunca dejarlas abiertas a todos los autenticados sin reglas
- Solo admin del tenant puede crear/eliminar membresias
- El usuario normal puede leer solo su propio membership

Ejemplo:

```protobuf
{
  name: "org_members"
  table_name: "org_members"
  acl_authenticated: [CREATE, READ, DELETE]
  create_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _REQ_.org AND user = _USER_.id AND role = 'admin')"
  read_access_rule:   "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
  delete_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id AND role = 'admin')"
}
```

### 3.3 APIs publicas de lectura

Ejemplos: catalogos, posts publicados, perfiles publicos

Recomendacion:
- `acl_world: [READ]`
- Escribir solo autenticado + regla de rol
- Considerar `VIEW` para no exponer columnas sensibles

## 4) Tabla vs fila vs columna: cuando usar cada nivel

1. Permiso por tabla/API
- Para habilitar o bloquear operaciones generales
- Se configura con ACL

2. Permiso por fila
- Para aislamiento real de datos por usuario/tenant
- Se configura con access rules SQL y `_ROW_`/`_REQ_`

3. Permiso por columna
- Para ocultar atributos sensibles de una API
- Preferir `VIEW`s o `excluded_columns`

Regla practica:
- ACL decide "quien puede intentar"
- Access rules deciden "en que filas"
- VIEW/excluded_columns deciden "que campos"

## 5) Blueprint minimo recomendado para SaaS B2B

Tablas:
- `organizations(id, name)`
- `org_members(org, user, role)`
- `projects(id, org, owner, ...)`

Permisos:
- `projects`: miembros leen/escriben filas de su org
- `org_members`: solo admins gestionan membresias
- `organizations`: lectura para miembros; mutaciones solo admin

Fragmento de ejemplo:

```protobuf
record_apis: [
  {
    name: "projects"
    table_name: "projects"
    acl_authenticated: [CREATE, READ, UPDATE, DELETE]
    create_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _REQ_.org AND user = _USER_.id)"
    read_access_rule:   "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
    update_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
    delete_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id AND role = 'admin')"
  },
  {
    name: "org_members"
    table_name: "org_members"
    acl_authenticated: [CREATE, READ, DELETE]
    create_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _REQ_.org AND user = _USER_.id AND role = 'admin')"
    read_access_rule:   "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)"
    delete_access_rule: "EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id AND role = 'admin')"
  }
]
```

## 6) Anti-patrones a evitar

1. Confiar solo en frontend
- Nunca asumir que el UI evita acciones invalidas.
- El backend debe validar siempre con reglas.

2. Dar CRUD completo a `acl_authenticated` sin rules
- Eso permite acceso transversal entre tenants/usuarios.

3. Exponer tablas de permisos sin proteccion fuerte
- Puede terminar en auto-promocion a admin.

4. Mezclar lectura publica con columnas sensibles
- Usa `VIEW` o `excluded_columns` para reducir superficie.

## 7) Plan de implementacion recomendado

1. Clasificar APIs en 3 grupos: publico, privado, admin.
2. Definir ACL minima por API.
3. Agregar reglas row-level por ownership/tenant.
4. Agregar rol/capability para operaciones sensibles.
5. Pasar endpoints publicos por VIEW para ocultar columnas.
6. Probar casos negativos:
- usuario A intentando leer/editar fila de B
- member intentando accion de admin
- anonimo intentando mutar

## 8) Snippets utiles de reglas

Ownership directo:

```sql
_ROW_.owner = _USER_.id
```

Create solo para owner declarado:

```sql
_REQ_.owner = _USER_.id
```

Miembro de tenant:

```sql
EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id)
```

Admin de tenant:

```sql
EXISTS(SELECT 1 FROM org_members WHERE org = _ROW_.org AND user = _USER_.id AND role = 'admin')
```

Capability puntual:

```sql
EXISTS(SELECT 1 FROM user_capabilities WHERE user_id = _USER_.id AND capability = 'billing:refund')
```

## 9) Referencias de esta repo

- Base de permisos y variables `_REQ_/_ROW_/_USER_`: [docs/src/content/docs/documentation/apis_record.mdx](docs/src/content/docs/documentation/apis_record.mdx)
- Patron multitenancy con reglas por tenant: [docs/src/content/docs/documentation/multitenancy.mdx](docs/src/content/docs/documentation/multitenancy.mdx)
- Ejemplo real con grupo editor: [examples/blog/traildepot/config.textproto](examples/blog/traildepot/config.textproto)
- Campos oficiales de `RecordApiConfig`: [crates/core/proto/config.proto](crates/core/proto/config.proto)
- Config de test con varias ACL/rules: [client/testfixture/config.textproto](client/testfixture/config.textproto)
