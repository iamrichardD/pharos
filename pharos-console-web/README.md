# Pharos Web Console

**The Sovereign Cockpit for your Lab.**

Pharos Web Console (`pharos-console-web`) is the primary Human/AI interface for the Pharos ecosystem. It transforms the high-performance RFC 2378 engine into an actionable, resource-first command center for both humans and AI agents.

## Key Features
- **Sovereign Cockpit**: A mobile-first, responsive interface for managing your entire lab.
- **WebMCP JSON-RPC Gateway**: A secure bridge for AI agents to interact with your lab resources via the Model Context Protocol.
- **Resource-First Design**: High-density inventory management with metadata "glance" blocks optimized for both humans and LLMs.
- **Security & Identity**: Implements "First-to-Claim" identity bonding and secure SSH-key authorization.

## Tech Stack
- **Framework**: Astro (SSR)
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **Testing**: Vitest & Playwright

## Commands

| Command | Action |
| :--- | :--- |
| `npm install` | Installs dependencies |
| `npm run dev` | Starts local dev server |
| `npm run build` | Builds the production SSR application |
| `npm run preview` | Previews the production build locally |
| `npm run test` | Runs unit tests (Vitest) |
| `npm run test:e2e` | Runs end-to-end tests (Playwright) |

## Configuration

The Web Console can be configured via environment variables:

| Variable | Description | Default |
| :--- | :--- | :--- |
| `PHAROS_HOST` | The IP/hostname of the Pharos Server. | `pharos-server` |
| `PHAROS_PORT` | The port of the Pharos Server. | `2378` |
| `PHAROS_PRIVATE_KEY` | Path to the private SSH key for Auth. | `/etc/pharos/keys/id_ed25519` |
| `PHAROS_PUBLIC_KEY` | Path to the public SSH key for Auth. | `/etc/pharos/keys/id_ed25519.pub` |
| `PHAROS_SANDBOX` | Enables Sandbox Simulator features. | `false` |

## Architecture
This component follows **Vertical Slice Architecture (VSA)**. Each feature (e.g., `auth`, `mdb`, `pulse`) is self-contained within `src/features/`, housing its own components, logic, and tests.
