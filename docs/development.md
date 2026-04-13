# Development

## Local loop

1. `make setup`
2. `make dev`
3. Open `http://localhost:3467/dashboard`

## Startup phases

The daemon:

1. Loads config and generates durable local keys.
2. Connects storage.
3. Restores specs, secrets, and runtime state.
4. Boots the platform router, auth routes, and dashboard.
5. Starts transports and optional soul personalization.

## Dashboard development

Use `cd dashboard && npm run dev` for hot reload. The Vite proxy forwards `/tdata`, `/observe`, `/api`, `/paw`, and `/auth` to the Rust server so cookie auth works the same way in development and production.
