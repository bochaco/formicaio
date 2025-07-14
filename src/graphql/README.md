# GraphQL API for Formicaio

This module provides a GraphQL API for querying node metrics and information in Formicaio.

## Overview

The GraphQL API exposes the same data as the REST API but with the following benefits:

- **Flexible Queries**: Request only the fields you need
- **Single Request**: Get multiple pieces of data in one request
- **Type Safety**: Strongly typed queries and responses
- **Introspection**: Self-documenting API with built-in documentation

## Endpoints

- **GraphQL**: `/graphql` - Main GraphQL endpoint
- **GraphiQL**: `/graphiql` - Interactive GraphQL playground

## Schema Structure

### Types

#### NodeMetricType
Represents a single metric data point:
- `key`: Metric name/key
- `value`: Metric value as string
- `timestamp`: Unix timestamp in milliseconds

#### NodeInstanceInfoType
Represents a node instance with all its information:
- `nodeId`: Unique node identifier
- `status`: Current node status
- `rewards`: Current reward balance
- `memUsed`: Memory usage in MB
- `cpuUsage`: CPU usage percentage
- `connectedPeers`: Number of connected peers
- And many more fields...

#### StatsType
Represents global statistics:
- `totalBalance`: Total balance across all nodes
- `totalNodes`: Total number of nodes
- `activeNodes`: Number of active nodes
- `estimatedNetSize`: Estimated network size
- And more...

### Queries

#### nodes
Get all nodes with optional filtering:
```graphql
nodes(filter: NodeFilter): [NodeInstanceInfoType!]!
```

#### node
Get a specific node by ID:
```graphql
node(nodeId: String!): NodeInstanceInfoType
```

#### nodeMetrics
Get metrics for a specific node:
```graphql
nodeMetrics(nodeId: String!, filter: MetricsFilter): [NodeMetricType!]!
```

#### stats
Get global statistics:
```graphql
stats: StatsType!
```

#### Individual Stats
Get individual stat values:
```graphql
activeNodesCount: Int!
totalNodesCount: Int!
totalBalance: String!
networkSize: Int!
storedRecords: Int!
```

## Implementation Details

### Files

- `mod.rs` - Module exports
- `schema.rs` - GraphQL schema definitions and type conversions
- `resolvers.rs` - Query resolvers implementation

### Type Conversions

The GraphQL types are converted from the internal Rust types:

- `NodeMetric` → `NodeMetricType`
- `NodeInstanceInfo` → `NodeInstanceInfoType`
- `Stats` → `StatsType`

### Integration

The GraphQL API is integrated into the main application in `main.rs`:

1. Creates a GraphQL schema with the server state
2. Adds GraphQL routes to the Axum router
3. Provides both `/graphql` and `/graphiql` endpoints

### Features

The GraphQL API is only available when the `ssr` feature is enabled, as it requires server-side functionality.

## Usage Examples

See `examples/graphql_queries.md` for comprehensive query examples.

## Benefits Over REST API

1. **Over-fetching Prevention**: Only request the fields you need
2. **Under-fetching Prevention**: Get multiple resources in one request
3. **Versioning**: Schema evolution without breaking changes
4. **Documentation**: Self-documenting with introspection
5. **Type Safety**: Compile-time query validation
6. **Real-time Schema**: Always up-to-date with code changes

## Future Enhancements

- Add mutations for node operations (start, stop, etc.)
- Add subscriptions for real-time updates
- Add more sophisticated filtering options
- Add pagination support
- Add field-level permissions 