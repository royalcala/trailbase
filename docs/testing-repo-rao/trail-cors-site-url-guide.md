# Guia rapida: CORS y site_url en TrailBase

Esta guia aclara que va en config.textproto y que va en flags de arranque para evitar errores de CORS.

## Resumen corto

- CORS no se configura en config.textproto.
- CORS se configura al iniciar el servidor con flags CLI.
- site_url si se configura en config.textproto (o con --public-url), pero sirve para links canonicos, emails y OAuth redirects.
- Si tu frontend corre en otro puerto/origen, ese origen se agrega en CORS, no en site_url.

## 1) Donde va cada cosa

### CORS

Se controla con flags de servidor:

- --dev
- --cors-allowed-origins

### URL publica del backend

Se define de dos formas:

- config.textproto -> server.site_url
- o por CLI con --public-url (tiene prioridad en runtime)

## 2) Caso desarrollo (frontend separado, por ejemplo Vite en :5173)

Si TrailBase esta en localhost:4000 y frontend en localhost:5173:

Comando recomendado:

    trail run --data-dir ./traildepot --dev --cors-allowed-origins http://localhost:5173

Notas:

- --dev vuelve CORS y cookies mas permisivos para desarrollo cross-origin.
- Si cambias de puerto (3000, 5174, etc), actualiza el origin exacto.
- Origin siempre debe incluir protocolo y puerto.

## 3) Caso produccion

Ejemplo con frontend en dominio distinto:

    trail run --data-dir ./traildepot --cors-allowed-origins https://app.midominio.com --public-url https://api.midominio.com

Recomendaciones:

- No uses --dev en produccion.
- Define origins explicitos, evita comodin *.
- Usa HTTPS en frontend y backend.

## 4) Que poner en config.textproto

Ejemplo minimo del bloque server:

    server {
      application_name: "Mi App"
      site_url: "https://api.midominio.com"
    }

Regla:

- site_url debe apuntar a la URL publica del backend TrailBase.
- Incluye puerto solo si el backend publico usa uno no estandar (por ejemplo :8443).
- No pongas aqui la URL del frontend, salvo que frontend y backend sean exactamente el mismo origen servido por TrailBase.

## 5) Checklist de diagnostico CORS

- El navegador muestra bloqueo en preflight OPTIONS o falta Access-Control-Allow-Origin.
- El origin del frontend coincide exactamente con --cors-allowed-origins.
- No hay mismatch de protocolo (http vs https).
- No hay mismatch de puerto.
- En dev cross-origin, arrancaste con --dev.
- Si usas cookies/sesion, revisa que tu flujo este pensado para cross-origin.

## 6) Ejemplos rapidos

Frontend local en :3000:

    trail run --data-dir ./traildepot --dev --cors-allowed-origins http://localhost:3000

Frontend local en :5173:

    trail run --data-dir ./traildepot --dev --cors-allowed-origins http://localhost:5173

Dos origins permitidos:

    trail run --data-dir ./traildepot --cors-allowed-origins https://app.midominio.com --cors-allowed-origins https://admin.midominio.com

## 7) Referencias en codigo

- crates/core/src/server/mod.rs (build_cors)
- crates/cli/src/args.rs (flags --dev y --cors-allowed-origins)
- crates/core/proto/config.proto (server.site_url)
- crates/core/src/app_state.rs (prioridad de --public-url sobre site_url)
