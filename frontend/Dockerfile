# Accounts UI for Dokploy: static build served by Nginx, which also proxies the
# API routes to the backend so the UI and API share one origin (host-only cookies).
FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:1.27-alpine
# API_ORIGIN: the backend as reachable from this container, e.g.
# http://knotree-api:8080 (Dokploy internal service name).
ENV API_ORIGIN=http://api:8080
COPY docker/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
