# Pharos Architecture

Project Pharos is a highly performant, read-optimized client-server ecosystem based on RFC 2378 (Phonebook Protocol). It is designed to empower Home Labbers and Enterprise Engineers by providing a "source of truth" for infrastructure that is as fast as it is reliable.

## Platform Architecture (Macro-View)
Helping you visualize how the various components—from the CLI to the Web Console—interact across the "Trust Boundary."

```mermaid
graph TD
    subgraph "Client Layer"
        Human[Human Actor]
        CLI_PH[ph CLI]
        CLI_MDB[mdb CLI]
    end

    subgraph "Web Layer (Astro v5)"
        Human -->|HTTPS| WebConsole[Pharos Web Console]
        WebConsole -->|JWT Auth| SessionManager[Session Manager]
        WebConsole -->|TCP/Ph Protocol| ClientLib[pharos.ts Client Lib]
    end

    subgraph "Backend Layer (Rust)"
        CLI_PH -->|TCP/Ph| ProtoListener[TCP Ph Listener]
        CLI_MDB -->|TCP/Ph| ProtoListener
        ClientLib -->|TCP/Ph| ProtoListener
        
        ProtoListener --> Middleware[Middleware Chain]
        Middleware --> CommandHandler[Command Handler]
        CommandHandler --> StorageTrait[Storage Trait]
        
        StorageTrait --> MemStore[Memory Storage]
        StorageTrait --> FileStore[File Storage]
        StorageTrait --> LDAPStore[LDAP Storage]
        
        Warp[Warp HTTP Server] -->|Pull| Metrics[Prometheus Metrics]
    end

    subgraph "Execution Layer (Podman)"
        CI[GitHub Actions] -->|Containerfile.test| Sandbox[Isolated Sandbox]
    end
```

## Application Architecture (Micro-View)
Detailing the internal async flow and the Vertical Slice Architecture (VSA) that keeps features decoupled and maintainable.

```mermaid
graph LR
    subgraph "Request Lifecycle"
        Parse[protocol.rs: Parser] --> Mid[middleware.rs: Chain]
        Mid --> Logic[lib.rs: Command Handler]
        Logic --> Store[storage.rs: Storage Trait]
    end

    subgraph "Storage Implementation"
        Store --> Mem[Memory]
        Store --> File[File]
        Store --> LDAP[LDAP]
    end

    subgraph "Security Middlewares"
        Mid --- Log[Logging]
        Mid --- Auth[SecurityTier]
        Mid --- RO[ReadOnly]
    end
```

## Storage Tiering Logic
Pharos adapts to your environment, ensuring that your data is exactly as persistent as you need it to be.

```mermaid
graph LR
    subgraph "Tier 1: Development"
        Mem[MemoryStorage]
        Mem -.->|Volatile| Data((In-Memory))
    end

    subgraph "Tier 2: Home Lab"
        File[FileStorage]
        File -.->|Persistent| JSON[(Local JSON File)]
    end

    subgraph "Tier 3: Enterprise"
        LDAP[LdapStorage]
        LDAP -.->|Centralized| Directory[(LDAP Server)]
    end
```

## Core Protocol: RFC 2378 (Modified)

Pharos implements the Phonebook Protocol with extensions for modern infrastructure management.

### Message Flow
1. **QUERY:** Client sends a search string.
2. **DISCRIMINATE:** Server identifies if the target is a `person` or `machine`.
3. **AUTH (if Write):** Server issues an SSH challenge. Client signs and returns.
4. **RESPONSE:** Server returns records in Ph format.

```mermaid
sequenceDiagram
    participant Client
    participant Server
    participant Auth
    participant Storage

    Client->>Server: QUERY "name=John"
    Server->>Storage: Search(people, "John")
    Storage-->>Server: [Records]
    Server-->>Client: 200: [Ph Records]

    Note over Client,Server: Write Operation
    Client->>Server: ADD "name=Jane"
    Server->>Auth: Challenge(SSH-Key)
    Auth-->>Client: Challenge
    Client-->>Auth: Signed-Response
    Auth-->>Server: Verified
    Server->>Storage: Commit(Jane)
    Server-->>Client: 200: Success
```

## Core Components

### 1. Pharos Server (`pharos-server`)
The backend engine handling connection lifecycle, protocol parsing, and storage abstraction.
- **Protocol:** RFC 2378 (Ph) with `auth` extension.
- **Authentication:** SSH-key based challenge-response for Write operations.
- **Metrics:** Integrated Prometheus scrape point (`:9090/metrics`) and health monitoring.

### 2. CLI Clients
- **`ph`:** Optimized for human contact management.
- **`mdb`:** Optimized for machine/infrastructure asset management.
- Both support automatic authentication via local SSH private keys.

### 3. Storage Tiering
- **Development:** Zero-configuration in-memory storage.
- **Home Lab:** File-level, restart-survivable JSON storage (optimized for LXC).
- **Enterprise:** LDAP-backed storage utilizing standard schemas (`inetOrgPerson`, `ipHost`).
