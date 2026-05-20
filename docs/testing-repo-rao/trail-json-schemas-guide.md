# Guía Detallada: JSON Schemas y Columnas JSON en TrailBase

Esta guía cubre cómo usar, definir y validar JSON Schemas en TrailBase, desde la definición en config.textproto hasta su uso en columnas de tablas y generación de tipos en cliente.

---

## 1) Conceptos fundamentales

### ¿Qué es un JSON Schema en TrailBase?

Un JSON Schema es una especificación formal que define la estructura y tipo de datos de un objeto JSON. TrailBase:

- Registra esquemas en `config.textproto`.
- Los valida en columnas SQL usando `CHECK(jsonschema(...))`.
- Los aplica automáticamente para validar inserts/updates.
- Genera tipos TypeScript, Dart, etc., desde estos esquemas para type-safety end-to-end.

### Casos de uso

- **Metadata nestrada**: Columnas con datos complejos/denormalizados (comentarios, tags, config).
- **Archivos**: `std.FileUpload` y `std.FileUploads` para metadata de uploads.
- **Type-safety en cliente**: Generar tipos desde la API JSON schema del backend.

---

## 2) Tipos y Storage

### Almacenamiento de JSON en SQLite

TrailBase soporta JSON en **TEXT** (textual). **No** soporta JSONB (formato comprimido interno de SQLite):

```sql
CREATE TABLE example (
    id         INTEGER PRIMARY KEY,
    -- JSON en TEXT (soportado)
    data_json  TEXT CHECK(is_json(data_json)),
    -- JSON en BLOB (JSONB, no soportado para schemas)
    -- bin_json BLOB,
) STRICT;
```

Nota: Puedes almacenar cualquier JSON en TEXT sin validación con `CHECK(is_json(...))`, pero para schemas personalizados debes usar `CHECK(jsonschema(...))`.

---

## 3) Constraints de validación JSON

### CHECK(is_json(...))

Valida que la columna contiene JSON válido, sin schema específico:

```sql
CREATE TABLE posts (
    id        INTEGER PRIMARY KEY,
    metadata  TEXT CHECK(is_json(metadata))
) STRICT;
```

Acepta cualquier JSON válido:
```json
{ "author": "Alice", "tags": ["tech", "web"] }
```

### CHECK(jsonschema('SchemaName', column))

Valida que el JSON cumple con un esquema registrado:

```sql
CREATE TABLE tasks (
    id     INTEGER PRIMARY KEY,
    meta   TEXT CHECK(jsonschema('TaskMetadata', meta))
) STRICT;
```

Solo acepta JSON que cumpla el schema `TaskMetadata` registrado en config.

### Schemas built-in: std.FileUpload y std.FileUploads

TrailBase proporciona esquemas para archivos:

**std.FileUpload** (un archivo):
```sql
CREATE TABLE articles (
    id    INTEGER PRIMARY KEY,
    logo  TEXT CHECK(jsonschema('std.FileUpload', logo))
) STRICT;
```

Estructura esperada:
```json
{
  "id": "uuid-string",
  "name": "logo.png",
  "size": 1024,
  "type": "image/png"
}
```

**std.FileUploads** (múltiples):
```sql
CREATE TABLE albums (
    id       INTEGER PRIMARY KEY,
    photos   TEXT CHECK(jsonschema('std.FileUploads', photos))
) STRICT;
```

Estructura esperada (array):
```json
[
  { "id": "uuid1", "name": "photo1.jpg", "size": 2048, "type": "image/jpeg" },
  { "id": "uuid2", "name": "photo2.jpg", "size": 3072, "type": "image/jpeg" }
]
```

---

## 4) Registrar esquemas personalizados en config.textproto

### Estructura

```textproto
schemas: [
  {
    name: "SchemaName"
    schema: "{JSON Schema como string}"
  }
]
```

El `schema` debe ser un string con JSON válido y que pase meta-validación JSON Schema.

### Ejemplo: TaskMetadata

En config.textproto:

```textproto
schemas: [
  {
    name: "TaskMetadata"
    schema: "{\"type\":\"object\",\"properties\":{\"priority\":{\"type\":\"integer\",\"minimum\":1,\"maximum\":5},\"estimated_hours\":{\"type\":\"number\"},\"tags\":{\"type\":\"array\",\"items\":{\"type\":\"string\"}}},\"required\":[\"priority\"]}"
  }
]
```

Más legible (antes de escapar):
```json
{
  "type": "object",
  "properties": {
    "priority": { "type": "integer", "minimum": 1, "maximum": 5 },
    "estimated_hours": { "type": "number" },
    "tags": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["priority"]
}
```

Y luego en SQL:

```sql
CREATE TABLE tasks (
    id       INTEGER PRIMARY KEY,
    title    TEXT NOT NULL,
    metadata TEXT CHECK(jsonschema('TaskMetadata', metadata))
) STRICT;
```

### Validaciones en el schema

TrailBase valida:

- Que el `name` sea presente y único.
- Que `schema` sea JSON válido.
- Que `schema` sea un JSON Schema válido según meta-schema.

Si alguno falla, TrailBase rechaza la config al iniciar.

---

## 5) Usando CHECK(jsonschema(...)) en SQL

### Sintaxis general

```sql
CHECK(jsonschema('SchemaName', column_name [, mime_type_pattern]))
```

Parámetros:

- `'SchemaName'`: Nombre del schema registrado.
- `column_name`: Columna a validar.
- `mime_type_pattern` (opcional): Para `std.FileUpload*`, restricción de tipos MIME, ej. `'image/png, image/jpeg'`.

### Ejemplos

**Básico:**
```sql
CREATE TABLE config (
    id     INTEGER PRIMARY KEY,
    data   TEXT CHECK(jsonschema('AppConfig', data)) NOT NULL
) STRICT;
```

**Con restricción MIME:**
```sql
CREATE TABLE articles (
    id            INTEGER PRIMARY KEY,
    cover_image   TEXT CHECK(jsonschema('std.FileUpload', cover_image, 'image/png, image/jpeg')),
    attachments   TEXT CHECK(jsonschema('std.FileUploads', attachments, 'application/pdf, application/msword'))
) STRICT;
```

### Errores comunes

1. **Schema no registrado**: Si intenta validar contra 'UnknownSchema' sin definirlo en config:
   - Error al insertar/actualizar.

2. **JSON inválido o no conforme**: Si inserta JSON que no cumple el schema:
   - Violación de constraint CHECK.
   - Transacción rollback.

3. **Mime type incorrecto**: Si usas `std.FileUpload` con restricción 'image/png' pero subes 'image/jpeg':
   - Violación de constraint.

---

## 6) Generación automática de tipos en cliente

### JSON Schema endpoint

Cada Record API tiene un endpoint `/api/records/v1/<api_name>/schema` que retorna JSON Schema del API.

Si una tabla tiene columnas con JSON schemas registrados, el schema endpoint incluye definiciones nestradas para esas columnas.

### Generación de tipos

Usando herramientas estándar (ej. `quicktype`, `json-schema-codegen`), puedes generar tipos TypeScript, Dart, Python, etc.:

**En TypeScript:**
```typescript
// Generated from JSON Schema
export interface TaskMetadata {
  priority: number;  // 1-5
  estimated_hours?: number;
  tags?: string[];
}

export interface Task {
  id: string;
  title: string;
  metadata: TaskMetadata;
}
```

**En Flutter/Dart:**
```dart
class TaskMetadata {
  final int priority;    // 1-5
  final double? estimatedHours;
  final List<String>? tags;
  
  TaskMetadata({
    required this.priority,
    this.estimatedHours,
    this.tags,
  });
}
```

### Ventaja

Type-safety end-to-end: database → backend → JSON Schema → client types.

---

## 7) Consultas JSON con SQLite

Aunque TrailBase valida y sirve el JSON, puedes consultar propiedades nestradas directamente en SQL:

### JSON Operators

- `json->>'key'`: Extrae valor como texto.
- `json->'key'`: Extrae valor como JSON.
- `json_extract(json, '$.path.to.value')`: Extrae con path.

### Ejemplos

```sql
-- Extrae priority de metadata
SELECT 
  id, 
  title, 
  json->>'priority' AS priority
FROM tasks;

-- Filtra tasks por priority >= 3
SELECT id FROM tasks 
WHERE CAST(json->>'priority' AS INTEGER) >= 3;

-- Filtra tags (suponiendo tags es array en metadata)
SELECT id FROM tasks 
WHERE json_array_length(json_extract(metadata, '$.tags')) > 0;
```

### Rendimiento

Nota: Consultas JSON requieren parsing en cada fila. Si consultas JSON frecuentemente, considera desnormalizar:

```sql
-- Denormalization: almacenar priority en columna separada
CREATE TABLE tasks (
    id              INTEGER PRIMARY KEY,
    title           TEXT NOT NULL,
    priority_num    INTEGER NOT NULL,  -- Denormalized para queries rápidas
    metadata        TEXT CHECK(jsonschema('TaskMetadata', metadata)),
    CHECK (CAST(json->>'priority' AS INTEGER) = priority_num)  -- Mantén coherencia
) STRICT;
```

---

## 8) Flujo completo: ejemplo práctico

Supongamos que quieres una tabla de tareas con metadata validada.

### Paso 1: Definir schema en config.textproto

```textproto
schemas: [
  {
    name: "TaskMetadata"
    schema: "{\"type\":\"object\",\"properties\":{\"priority\":{\"type\":\"integer\",\"minimum\":1,\"maximum\":5},\"labels\":{\"type\":\"array\",\"items\":{\"type\":\"string\"}},\"estimated_hours\":{\"type\":\"number\"}},\"required\":[\"priority\"]}"
  }
]
```

### Paso 2: Crear tabla en SQL migration

```sql
CREATE TABLE tasks (
    id       INTEGER PRIMARY KEY,
    title    TEXT NOT NULL,
    metadata TEXT NOT NULL CHECK(jsonschema('TaskMetadata', metadata)),
    created  INTEGER NOT NULL DEFAULT (UNIXEPOCH()),
    UNIQUE(title)
) STRICT;
```

### Paso 3: Exponer vía Record API en config.textproto

```textproto
record_apis: [
  {
    name: "tasks"
    table_name: "tasks"
    acl_world: [READ]
    acl_authenticated: [CREATE, READ, UPDATE, DELETE]
  }
]
```

### Paso 4: Usar desde cliente (TypeScript)

Generar tipos desde `/api/records/v1/tasks/schema`:

```typescript
interface Task {
  id: string;
  title: string;
  metadata: {
    priority: number;     // 1-5
    labels?: string[];
    estimated_hours?: number;
  };
  created: number;
}

// Crear tarea
const task: Task = {
  id: "uuid",
  title: "Fix bug",
  metadata: {
    priority: 3,
    labels: ["urgent", "backend"],
    estimated_hours: 2.5
  },
  created: Date.now()
};

// POST /api/records/v1/tasks
const created = await client.POST('/api/records/v1/tasks', task);
```

### Paso 5: Query con filtro (SQLite)

```sql
-- ListaTaskswith priority >= 3
SELECT id, title, metadata 
FROM tasks 
WHERE CAST(json->>'priority' AS INTEGER) >= 3;
```

---

## 9) Checklist de validación

Antes de poner un schema en producción:

- [ ] Schema registrado en config.textproto con nombre único.
- [ ] JSON Schema sintacticamente válido (JSON válido + meta-schema válido).
- [ ] Columna es TEXT (no BLOB).
- [ ] CHECK incluye nombre exacto del schema.
- [ ] Test insert/update con datos válidos (debe pasar).
- [ ] Test insert/update con datos inválidos (debe fallar con constraint violation).
- [ ] Tipos generados en cliente coinciden con schema.
- [ ] Si usas std.FileUpload*, mime_type_pattern restringido según tus necesidades.

---

## 10) Referencias en código

- `crates/core/proto/config.proto` (JsonSchemaConfig message).
- `crates/core/src/config.rs` (validación de schemas).
- `docs/src/content/docs/documentation/apis_record.mdx` (custom JSON schemas).
- `docs/src/content/docs/documentation/models_and_relations.mdx` (JSON type safety).
- `docs/testing-repo-rao/trail-file-upload-guide.md` (FileUpload schemas).
- Ejemplos SQL: `examples/blog/traildepot/migrations/`.

---

## 11) Schemasuarios built-in disponibles

Por defecto, TrailBase ofrece:

- `std.FileUpload`: Metadatos de un archivo (id, name, size, type).
- `std.FileUploads`: Array de metadatos de múltiples archivos.

Ambos se usan con columnas TEXT y CHECK descrito arriba.

---

## 12) Limitaciones conocidas

- No soporta JSONB (binary JSON).
- Repeated fields (arrays en record_apis) no tienen soporte general para env var override.
- JSON queries pueden ser lentas en tablas grandes (sin índices).
- Desnormalización de propiedades frecuentes recomendada si performance es crítica.
