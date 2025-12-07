#!/bin/bash
docker compose up -d
echo "Waiting for cluster to be ready..."
sleep 5
for port in {7010..7015}; do
  docker exec redis-cluster redis-cli -p $port CONFIG SET protected-mode no
done
echo "Cluster ready and protected-mode disabled."
