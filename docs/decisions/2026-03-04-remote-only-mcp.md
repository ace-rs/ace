# Decision: Remote-Only MCP (2026-03-04)

ACE supports remote MCP servers exclusively. Schools declare `[[mcp]]` entries with URLs pointing
to hosted MCP endpoints. The backend handles OAuth discovery, token acquisition, storage, and
refresh — ACE only registers the endpoint.

Docker-based stdio MCP (container images with injected tokens) is not supported. This was a
deliberate simplification based on the state of the MCP ecosystem.

## Why Remote-Only

As of early 2026, remote MCP with OAuth 2.1 is the dominant model. 80+ official vendor-hosted
servers exist. Every major developer tool category has remote MCP coverage:

| Category           | Services with remote MCP endpoints                    |
|--------------------|-------------------------------------------------------|
| Code hosting       | GitHub, GitLab, Buildkite                             |
| Project management | Jira/Confluence (Atlassian), Linear, Notion, Asana    |
| Observability      | Sentry, Datadog, PagerDuty, Cloudflare                |
| Cloud              | AWS (60+ servers), GCP (preview)                      |
| Databases          | Supabase, Neon, Prisma (managed Postgres)             |
| Payments           | Stripe, PayPal, Square                                |
| Design             | Figma, Canva, Webflow                                 |
| Deployment         | Vercel, Netlify                                       |

Raw database access (direct Postgres/MySQL/MongoDB) has no vendor-hosted remote endpoint, but
managed providers (Supabase, Neon, Prisma, AlloyDB) cover this via their own MCP servers with
OAuth. Internal services can be exposed through self-hosted MCP gateways (Cloudflare Workers,
etc.) that implement OAuth.

The Docker stdio model — where ACE injects tokens via env vars into containers — has no remaining
use case that cannot be served by hosting a remote MCP endpoint with OAuth instead.
