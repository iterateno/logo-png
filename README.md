# Iterate Logo in PNG

This serves the iterate logo live from the `logo-api` in png-form. It's basically a hack until
`logo-api` gets png support.

It also has a websocket which sends the png bytes every time the logo changes. It works by polling
`logo-api`.

## Setup db for local development

```
createuser -S -R -d logo-png
psql -c "ALTER USER \"logo-png\" ENCRYPTED PASSWORD 'logo-png'"
createdb -O logo-png logo-png
```
