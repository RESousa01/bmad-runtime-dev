FROM node:24.18.0-alpine@sha256:a0b9bf06e4e6193cf7a0f58816cc935ff8c2a908f81e6f1a95432d679c54fbfd

WORKDIR /workspace

RUN corepack enable && corepack prepare pnpm@11.15.1 --activate

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json ./
COPY apps/desktop-ui/package.json apps/desktop-ui/package.json
COPY packages/ui/package.json packages/ui/package.json
COPY packages/contracts/package.json packages/contracts/package.json
COPY packages/bmad-foundation/package.json packages/bmad-foundation/package.json
COPY packages/bmad-fixtures/package.json packages/bmad-fixtures/package.json

COPY apps/desktop-ui apps/desktop-ui
COPY packages/ui packages/ui
COPY packages/contracts packages/contracts
COPY packages/bmad-foundation packages/bmad-foundation
COPY packages/bmad-fixtures packages/bmad-fixtures

RUN pnpm install --frozen-lockfile
RUN pnpm --filter @forge/desktop-ui build

EXPOSE 1420

CMD ["pnpm", "--filter", "@forge/desktop-ui", "exec", "vite", "preview", "--host", "0.0.0.0", "--port", "1420"]
