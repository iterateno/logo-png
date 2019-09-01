# UI to show history. Written in elm

## Running in development

Use elm live

```bash
# Install
npm install --global elm elm-live
# Use
elm-live src/Main.elm --start-page=history.html -- --output=history.js
```

http://localhost:3000/history

(or http://localhost:8000, but then the api call does not work. TODO: Fix the api call so it uses localhost:3000 in development)
