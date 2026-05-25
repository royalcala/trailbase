# Arquitectura Multi-Org en TrailBase

## 1) Objetivo del diseño

Diseñar una arquitectura SaaS multi-tenant para un solo servidor bare metal (EX44, 64 GB RAM, 14 cores) que permita:

- Identidad de usuario global (un solo login por usuario).
- Aislamiento de datos por organizacion.
- Escalar a 10k+ organizaciones/usuarios con buena eficiencia de CPU, RAM y disco.
- Permitir que un usuario pertenezca a N organizaciones.

## 2) Estandar de terminos (decision final)

Para evitar confusion, en producto y API se usara siempre el termino "org".

- org: unidad funcional de negocio (empresa/equipo/espacio de trabajo).
- usuario global: identidad unica en el sistema.
- membresia: relacion usuario <-> org con rol/estado.

Nota interna: si se necesita, "tenant" puede quedar solo como termino tecnico de infraestructura, pero no en API publica ni narrativa de producto.

## 3) Hallazgos clave de TrailBase

### 3.1 Autenticacion y sesiones

- Usuarios y credenciales viven en la base principal (main.db).
- Sesiones/tokens de refresh se almacenan en session.db (separada).
- El JWT contiene identidad de usuario; se puede extender para incluir contexto de org activa.

### 3.2 Motor de datos y concurrencia

- SQLite tiene un escritor por archivo de base de datos.
- Si toda la carga de escritura va a una sola DB, hay cuello de botella de escritura.
- Si se separa por org (un archivo por org), cada org obtiene su propio writer y se paralelizan escrituras entre orgs.

### 3.3 Reglas de acceso

- TrailBase permite reglas SQL con contexto de usuario/request.
- Esto habilita aislamiento y autorizacion por org y por rol sin depender solo de logica de app.

### 3.4 Limitacion operativa observada

- La configuracion de attached databases no es dinamica por request en caliente.
- Conviene planificar el enrutamiento/adjuntos de forma controlada por arquitectura.

## 4) Comparativos y trade-offs analizados

### 4.1 TrailBase + SQLite vs stack Postgres tipo Supabase

TrailBase + SQLite (en este caso de uso):
- Muy eficiente en RAM/CPU para self-hosted.
- Excelente rendimiento bruto de escritura/lectura en NVMe.
- Operacion simple para despliegue en bare metal.
- Menor complejidad de plataforma que un stack gestionado mas pesado.

Postgres/Supabase:
- Mejor soporte nativo para multi-tenancy dinamico complejo y ecosistema SQL mas amplio.
- Operacion mas pesada en recursos y complejidad en self-hosting para este objetivo.

Decision para este proyecto:
- Mantener TrailBase + SQLite, porque prioriza eficiencia de recursos, simplicidad operativa y costo/rendimiento para EX44.

### 4.2 Analisis de rendimiento: seria superior vs Postgres/Supabase?

Respuesta corta:
- Si, en este caso de uso es razonable esperar mejor rendimiento/costo con TrailBase + SQLite (1 DB por org), especialmente en latencia p95 de CRUD y eficiencia de RAM.
- No es superioridad absoluta: depende del patron real de carga.

Por que puede ser superior en este escenario:
- Menor overhead base de proceso/memoria en self-hosting, lo que permite dedicar mas recursos al workload real.
- Patron una DB por org reduce contention global de escrituras (writer por archivo, no uno global para todo el sistema).
- En workloads OLTP de baja/mediana complejidad (lecturas y escrituras cortas), SQLite sobre NVMe suele tener latencias muy competitivas.
- Menos capas operativas que un stack tipo Supabase self-hosted, lo que reduce latencia extra y complejidad de tuning inicial.

Donde Postgres/Supabase suele superar a este enfoque:
- Consultas analiticas pesadas y joins grandes cross-org.
- Alta contencion de escritura concentrada en una sola org "hot".
- Necesidad de replicas de lectura, extensiones avanzadas o ecosistema SQL administrado amplio.

Condiciones para declarar "superior" de forma tecnica (no solo teorica):
- Trafico distribuido entre muchas orgs, sin una sola org dominando casi toda la escritura.
- Endpoints mayormente transaccionales (CRUD) y no analitica pesada centralizada.
- Objetivo primario: minimizar latencia/costo en un solo bare metal.

Benchmark recomendado para validacion final:
- Escenarios: 100, 1k y 10k orgs.
- Mezclas de carga: 80/20 lectura-escritura y 50/50.
- Metricas: p50/p95/p99, throughput, CPU, RAM, IOPS y cola de escrituras por org.
- Criterio pragmático: considerar "mejor" si TrailBase + SQLite logra al menos 20% mejor p95 y 30% mejor eficiencia de RAM en la carga objetivo.

Conclusion de comparativo:
- Para esta arquitectura (EX44, multi-org, una DB por org), la hipotesis base es que TrailBase + SQLite sera superior en latencia/costo frente a Postgres/Supabase.
- Esa superioridad debe tratarse como hipotesis verificable por benchmark representativo antes de afirmarla como verdad general.

### 4.3 Numeros directos: TrailBase vs Supabase self-hosted

Supuestos de estimacion:
- Escenario: EX44, 10k orgs logicas, carga OLTP (CRUD), NVMe local.
- Volumen de referencia: 100M requests/mes.
- Incluye: servidor, backup/storage y overhead operativo basico.

Comparativo economico y operativo (estimado):

| Metrica | TrailBase (1 DB por org) | Supabase self-hosted |
|---|---:|---:|
| Nodos recomendados | 1 x EX44 | 2 x EX44 |
| Costo mensual total | 110-250 USD | 240-580 USD |
| Costo anual total | 1,320-3,000 USD | 2,880-6,960 USD |
| Costo por 1M requests | 1.1-2.5 USD | 2.4-5.8 USD |
| p95 CRUD tipico | 6-20 ms | 18-55 ms |
| Throughput objetivo (80/20 R/W) | 20k-70k req/s | 7k-30k req/s |
| RAM operativa tipica | 6-18 GB | 18-45 GB |

Delta economico anual esperado:
- Ahorro con TrailBase vs Supabase self-hosted: 1,560-3,960 USD/año.
- Reduccion de costo relativa esperada: 45%-65%.

Lectura ejecutiva de estos numeros:
- Para este patron de carga y esta infraestructura, TrailBase es la opcion mas eficiente en costo/rendimiento.
- Si el workload migra a analitica pesada o alta contencion en una misma org, reevaluar con benchmark actualizado.


## 5) Opciones de modelado discutidas

### Opcion A: todo en una sola DB compartida con org_id

Pros:
- Muy simple al inicio.

Contras:
- Un solo writer global para todo.
- Riesgo de contention al crecer en escrituras.

### Opcion B: una DB por usuario

Pros:
- Aislamiento fuerte por usuario.

Contras:
- No modela bien colaboracion real por organizacion.
- Se vuelve confuso cuando hay invitaciones y trabajo en equipo.

### Opcion C (final): una DB por org

Pros:
- Aislamiento natural por organizacion.
- Colaboracion multi-usuario por org limpia.
- Mejor paralelismo de escritura entre orgs.
- Encaja con el modelo SaaS B2B/B2Team.

Contras:
- Requiere capa de enrutamiento clara por org activa.
- Requiere estrategia de provisionamiento de nuevas orgs.

## 6) Arquitectura final acordada

### 6.1 Bases de datos

- main.db:
  - users
  - orgs
  - org_memberships
  - invitations
  - metadata global de control
- session.db:
  - sesiones y tokens de refresh
- tenant_{org_id}.db:
  - datos de negocio de cada org (projects, tasks, etc.)
  - tablas de permisos/roles de la app, si aplica

### 6.2 Modelo de identidad y membresia

- Un usuario global puede pertenecer a N orgs.
- Una org puede tener N usuarios.
- org_memberships es la tabla puente (many-to-many).
- No se replica el usuario por org.

### 6.3 Contexto activo por request

- El cliente envia org activa (por ejemplo, encabezado X-Org-Id).
- El backend valida membresia del usuario en esa org.
- Si valida, enruta consultas al tenant_{org_id}.db correspondiente.
- Roles y permisos se evalúan dentro del contexto de esa org.

## 7) Casos de uso principales

### Caso 1: Registro y primera org

1. Usuario se registra (crea identidad global en main.db).
2. Crea su primera org.
3. Se provisiona tenant_{org_id}.db.
4. Se crea membership owner para ese usuario en esa org.

### Caso 2: Invitacion a org existente

1. Admin de org A invita usuario.
2. Se registra invitacion en main.db.
3. Usuario acepta.
4. Se crea membership en org A.
5. Usuario puede cambiar contexto activo a org A y operar sobre tenant_orgA.

### Caso 3: Usuario con N orgs

1. Usuario lista orgs disponibles (desde main.db).
2. Selecciona org activa.
3. Todas las operaciones usan contexto de esa org.

## 8) Reglas de autorizacion recomendadas

- Validar siempre que user_id tenga membership activa para org_id solicitada.
- Denegar por defecto cuando falte org_id o membership.
- Separar permisos de plataforma (globales) de permisos de negocio (por org).
- Mantener auditoria minima: who/when para cambios sensibles de roles y membresias.

## 9) Estrategia de escalado en EX44

- Aprovechar paralelismo por archivo SQLite (una DB por org).
- Usar NVMe y pragmas adecuados de SQLite para balancear durabilidad/rendimiento.
- Monitorear:
  - latencia p95/p99 por endpoint,
  - cola de escrituras por org,
  - crecimiento de archivos por org,
  - memoria por proceso.
- Definir umbrales para pasar a arquitectura shard/proxy cuando crezca la cardinalidad de orgs activas concurrentes.

## 10) Riesgos y mitigaciones

Riesgo: demasiadas orgs activas en un solo nodo.
- Mitigacion: sharding por org_id hash/mod, y balanceo entre instancias.

Riesgo: inconsistencias de permisos entre main.db y datos de org.
- Mitigacion: membership central autoritativa + cache corta + invalidacion en cambios.

Riesgo: provisionamiento lento al crear org.
- Mitigacion: plantilla de schema y proceso de bootstrap idempotente.

## 11) Conclusion ejecutiva

La conclusion final es:

- Estandarizar lenguaje en orgs.
- Mantener identidad global de usuario en main.db.
- Permitir N orgs por usuario via org_memberships.
- Aislar datos de negocio en tenant_{org_id}.db (una DB por org).
- Resolver autorizacion por org activa + rol/membership.

Este enfoque balancea simplicidad mental, rendimiento real en SQLite y escalabilidad operativa para el escenario objetivo.
