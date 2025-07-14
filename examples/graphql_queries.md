# Formicaio GraphQL API Examples

The Formicaio GraphQL API provides a flexible way to query node metrics and information. You can access the GraphQL playground at `/graphiql` when the server is running.

## Basic Queries

### Get All Nodes
```graphql
query {
  nodes {
    nodeId
    status
    port
    metricsPort
    rewards
    memUsed
    cpuUsage
    connectedPeers
    records
    netSize
  }
}
```

### Get a Specific Node
```graphql
query {
  node(nodeId: "your-node-id") {
    nodeId
    status
    peerId
    binVersion
    rewards
    memUsed
    cpuUsage
    connectedPeers
    records
    netSize
  }
}
```

### Get Node Metrics
```graphql
query {
  nodeMetrics(nodeId: "your-node-id") {
    ant_networking_process_memory_used_mb {
      key
      value
      timestamp
    }
    ant_networking_process_cpu_usage_percentage {
      key
      value
      timestamp
    }
    ant_node_current_reward_wallet_balance {
      key
      value
      timestamp
    }
  }
}
```

### Get Node Metrics with Time Filter
```graphql
query {
  nodeMetrics(
    nodeId: "your-node-id"
    filter: { since: 1640995200000 }
  ) {
    ant_networking_process_memory_used_mb {
      key
      value
      timestamp
    }
    ant_networking_process_cpu_usage_percentage {
      key
      value
      timestamp
    }
  }
}
```

### Get Global Stats
```graphql
query {
  stats {
    totalBalance
    totalNodes
    activeNodes
    inactiveNodes
    connectedPeers
    shunnedCount
    estimatedNetSize
    storedRecords
    relevantRecords
  }
}
```

### Get Individual Stats
```graphql
query {
  activeNodesCount
  totalNodesCount
  totalBalance
  networkSize
  storedRecords
}
```

### Filter Nodes by Status
```graphql
query {
  nodes(filter: { status: "Active" }) {
    nodeId
    status
    rewards
    memUsed
    cpuUsage
  }
}
```

## Advanced Queries

### Get Multiple Nodes with Metrics
```graphql
query {
  nodes {
    nodeId
    status
    rewards
    memUsed
    cpuUsage
  }
  stats {
    totalBalance
    activeNodes
    totalNodes
  }
}
```

### Get Node with All Details
```graphql
query {
  node(nodeId: "your-node-id") {
    nodeId
    pid
    created
    statusChanged
    status
    isStatusLocked
    isStatusUnknown
    peerId
    statusInfo
    binVersion
    port
    metricsPort
    nodeIp
    balance
    rewardsAddr
    homeNetwork
    upnp
    nodeLogs
    rewards
    records
    relevantRecords
    memUsed
    cpuUsage
    connectedPeers
    connectedRelayClients
    kbucketsPeers
    shunnedCount
    netSize
    ips
  }
}
```

## Available Metrics Keys

The following metric keys are available in the `nodeMetrics` query:

- `ant_node_current_reward_wallet_balance` - Current reward wallet balance
- `ant_networking_process_memory_used_mb` - Memory usage in MB
- `ant_networking_process_cpu_usage_percentage` - CPU usage percentage
- `ant_networking_records_stored` - Number of stored records
- `ant_networking_relevant_records` - Number of relevant records
- `ant_networking_connected_peers` - Number of connected peers
- `ant_networking_connected_relay_clients` - Number of connected relay clients
- `ant_networking_peers_in_routing_table` - Number of peers in routing table
- `ant_networking_shunned_count_total` - Number of shunned peers
- `ant_networking_estimated_network_size` - Estimated network size

## Using the GraphQL Playground

1. Start the Formicaio server
2. Navigate to `http://localhost:52100/graphiql`
3. Use the interactive playground to explore the schema and test queries
4. The schema documentation is available in the right sidebar

## Benefits of GraphQL API

- **Flexible Queries**: Request only the data you need
- **Single Request**: Get multiple pieces of data in one request
- **Real-time Schema**: Always up-to-date with the latest schema
- **Type Safety**: Strongly typed queries and responses
- **Introspection**: Self-documenting API with built-in documentation 