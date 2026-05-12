# TrailBase File Uploads - Guia Practica

Esta guia resume como funciona File Upload en TrailBase usando evidencia real de esta repo (docs, tests e implementacion).

## 1) Modelo mental rapido

TrailBase separa:
- Metadatos del archivo en la tabla (columna `TEXT` con `jsonschema`)
- Contenido binario en object storage (local o S3)

Eso significa que cuando haces `read` de un record, recibes metadata del archivo, no el binario.
Para bajar el binario usas endpoints dedicados de `file/files`.

## 2) Como definir tablas para upload

Patron recomendado (mismo que usa el testfixture de esta repo):

```sql
CREATE TABLE IF NOT EXISTS file_upload_table (
  id             BLOB PRIMARY KEY NOT NULL CHECK(is_uuid_v7(id)) DEFAULT(uuid_v7()),
  single_file    TEXT CHECK(jsonschema('std.FileUpload', single_file)),
  multiple_files TEXT CHECK(jsonschema('std.FileUploads', multiple_files)),
  name           TEXT
) STRICT;
```

Que hace cada tipo:
- `std.FileUpload`: un archivo
- `std.FileUploads`: lista de archivos

## 3) Formas de subir archivos

TrailBase soporta 2 caminos:

1. JSON (base64 dentro del payload)
2. `multipart/form-data`

### 3.1 Upload via JSON base64

Ejemplo de `POST /api/records/v1/file_upload_table`:

```json
{
  "name": "Base64 File Upload Test",
  "single_file": {
    "name": "single_test",
    "filename": "test1.bin",
    "content_type": "application/octet-stream",
    "data": "AAECAwQF"
  },
  "multiple_files": [
    {
      "name": "multi_test_1",
      "filename": "test2.bin",
      "content_type": "application/octet-stream",
      "data": "KgUqBQ"
    },
    {
      "name": "multi_test_2",
      "filename": "test3.bin",
      "content_type": "application/octet-stream",
      "data": "_4BAIA=="
    }
  ]
}
```

Notas importantes:
- `data` acepta base64 URL-safe y base64 estandar.
- En JSON, el campo `name` es opcional.
- Si el schema es `std.FileUpload`, solo puede entrar 1 archivo en esa columna.

### 3.2 Upload via multipart/form-data

`single_file`/`multiple_files` deben coincidir con los nombres de columna.

```bash
curl -X POST http://localhost:4000/api/records/v1/file_upload_table \
  -F "name=Multipart Test" \
  -F "single_file=@./test.txt"
```

Reglas practicas:
- El name del campo multipart se usa para mapear a columna.
- Si mandas un archivo a una columna que no existe, TrailBase lo ignora.
- Si subes mas de un archivo a columna `std.FileUpload`, falla por colision.

## 4) Como leer metadata y descargar binario

### 4.1 Leer metadata (Record API normal)

```bash
curl http://localhost:4000/api/records/v1/file_upload_table/<record_id>
```

La respuesta trae metadata del archivo (por ejemplo filename/original_filename/content_type/mime_type, segun version).

### 4.2 Descargar contenido binario

TrailBase tiene endpoints dedicados:

```bash
# Columna de archivo unico
curl http://localhost:4000/api/records/v1/file_upload_table/<record_id>/file/single_file --output single.bin

# Columna lista de archivos (recomendado para cache por filename unico)
curl http://localhost:4000/api/records/v1/file_upload_table/<record_id>/files/multiple_files/<file_name> --output downloaded.bin
```

Diferencia clave:
- `/file/<column>`: para recuperar el archivo asociado en columna simple
- `/files/<column>/<file_name>`: para seleccionar uno puntual de una lista

## 5) Storage backend: local vs S3

Por defecto:
- Archivos se guardan localmente en `<traildepot>/uploads`

Opcional:
- Puedes configurar S3 con `s3_storage_config` en `config.textproto`

Campos configurables en proto:
- `endpoint`
- `region`
- `bucket_name`
- `access_key`
- `secret_access_key`

## 6) Type-safety y por que importa

Cuando defines `CHECK(jsonschema('std.FileUpload', col))`:
- SQLite valida el contrato al insertar/actualizar
- `trail schema --mode insert|select|update` refleja el tipo
- Puedes generar tipos cliente con mas seguridad

## 7) Errores comunes y como evitarlos

1. Tabla no `STRICT`
- Sintoma: problemas de type-safety y schema inconsistente.
- Solucion: usa `STRICT` en la tabla.

2. Usar `BLOB`/`TEXT` sin `jsonschema` para archivos
- Sintoma: no hay manejo especial de upload/download.
- Solucion: usa `TEXT CHECK(jsonschema('std.FileUpload'...))`.

3. Multipart con nombre de campo incorrecto
- Sintoma: archivo no aparece en metadata.
- Solucion: usa exactamente el nombre de columna (`single_file`, `multiple_files`, etc.).

4. Intentar leer binario desde `GET /api/records/v1/<api>/<id>`
- Sintoma: solo ves metadata.
- Solucion: usa endpoints `/file/...` o `/files/...`.

## 8) Checklist de implementacion

1. Crear tabla `STRICT` con columnas `std.FileUpload`/`std.FileUploads`.
2. Exponerla por Record API.
3. Probar create por JSON base64.
4. Probar create por multipart.
5. Verificar lectura de metadata por `read`.
6. Verificar descarga binaria por `/file` y `/files`.
7. Si aplica, mover storage a S3.

## 9) Referencias dentro de esta repo

- Docs de Record API y File Uploads: [docs/src/content/docs/documentation/apis_record.mdx](docs/src/content/docs/documentation/apis_record.mdx)
- Tipado y JSON schema en columnas: [docs/src/content/docs/documentation/models_and_relations.mdx](docs/src/content/docs/documentation/models_and_relations.mdx)
- Type-safety con `FileUpload`: [docs/src/content/docs/documentation/type_safety.mdx](docs/src/content/docs/documentation/type_safety.mdx)
- Definicion real de tabla de prueba: [client/testfixture/migrations/main/U1758448960__create_file_upload_table.sql](client/testfixture/migrations/main/U1758448960__create_file_upload_table.sql)
- Test TS con upload+download base64: [crates/assets/js/client/tests/integration/client_integration.test.ts](crates/assets/js/client/tests/integration/client_integration.test.ts)
- Test Rust multipart form: [crates/client/tests/integration_test.rs](crates/client/tests/integration_test.rs)
- Parsing y mapeo interno de archivos: [crates/core/src/records/params.rs](crates/core/src/records/params.rs)
- Esquema de tipos de archivo: [crates/schema/src/file.rs](crates/schema/src/file.rs)
- Configuracion S3 en proto: [crates/core/proto/config.proto](crates/core/proto/config.proto)
