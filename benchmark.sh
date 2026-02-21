#!/bin/bash
# RedRust Benchmark Script
# Compares RedRust performance against Redis (if available)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REDRUST_PORT=6379
REDIS_PORT=6380
REQUESTS=10000
PIPELINE=1

# Check which servers are available
echo -e "${BLUE}=== RedRust Benchmark ===${NC}"
echo

# Function to check if a port is open
check_port() {
    nc -z 127.0.0.1 $1 2>/dev/null && echo "open" || echo "closed"
}

# Check RedRust
if [ "$(check_port $REDRUST_PORT)" = "open" ]; then
    echo -e "${GREEN}✓ RedRust detected on port $REDRUST_PORT${NC}"
    REDRUST_AVAILABLE=true
else
    echo -e "${RED}✗ RedRust not running on port $REDRUST_PORT${NC}"
    echo "  Start it with: cargo run"
    REDRUST_AVAILABLE=false
fi

# Check Redis (optional)
if [ "$(check_port $REDIS_PORT)" = "open" ]; then
    echo -e "${GREEN}✓ Redis detected on port $REDIS_PORT${NC}"
    REDIS_AVAILABLE=true
else
    echo -e "${YELLOW}! Redis not running on port $REDIS_PORT (skipping comparison)${NC}"
    REDIS_AVAILABLE=false
fi

echo
if [ "$REDRUST_AVAILABLE" = "false" ]; then
    echo -e "${RED}Error: RedRust must be running to benchmark${NC}"
    exit 1
fi

# Test functions
run_redrust_cmd() {
    echo -e "$1\r\n" | nc 127.0.0.1 $REDRUST_PORT 2>/dev/null | head -1
}

# Clean up data first
echo -e "${BLUE}Cleaning up test data...${NC}"
echo -e "FLUSHDB is not implemented, manually cleaning..."
for i in $(seq 1 100); do
    echo -e "DEL bench:$i\r\n" | nc 2>/dev/null | head -1 > /dev/null || true
done

echo
echo -e "${BLUE}=== Running Benchmarks ($REQUESTS requests, pipeline=$PIPELINE) ===${NC}"
echo

# Benchmark 1: SET operations
echo -e "${YELLOW}Benchmark 1: SET${NC}"
echo -n "  Preparing SET commands..."
START_TIME=$(date +%s%N)
for i in $(seq 1 $REQUESTS); do
    run_redrust_cmd "SET bench:key:$i value_$i" > /dev/null
done
END_TIME=$(date +%s%N)
ELAPSED=$(( (END_TIME - START_TIME) / 1000000 ))  # Convert to ms
RPS=$(( REQUESTS * 1000 / (ELAPSED + 1) ))
echo -e "\r  ${GREEN}SET: $REQUESTS operations in ${ELAPSED}ms (${RPS} req/sec)${NC}"

# Benchmark 2: GET operations
echo
echo -e "${YELLOW}Benchmark 2: GET${NC}"
echo -n "  Running GET commands..."
START_TIME=$(date +%s%N)
for i in $(seq 1 $REQUESTS); do
    run_redrust_cmd "GET bench:key:$i" > /dev/null
done
END_TIME=$(date +%s%N)
ELAPSED=$(( (END_TIME - START_TIME) / 1000000 ))
RPS=$(( REQUESTS * 1000 / (ELAPSED + 1) ))
echo -e "\r  ${GREEN}GET: $REQUESTS operations in ${ELAPSED}ms (${RPS} req/sec)${NC}"

# Benchmark 3: LPUSH
echo
echo -e "${YELLOW}Benchmark 3: LPUSH (List operations)${NC}"
echo -n "  Running LPUSH commands..."
START_TIME=$(date +%s%N)
for i in $(seq 1 $REQUESTS); do
    run_redrust_cmd "LPUSH bench:list item_$i" > /dev/null
done
END_TIME=$(date +%s%N)
ELAPSED=$(( (END_TIME - START_TIME) / 1000000 ))
RPS=$(( REQUESTS * 1000 / (ELAPSED + 1) ))
echo -e "\r  ${GREEN}LPUSH: $REQUESTS operations in ${ELAPSED}ms (${RPS} req/sec)${NC}"

# Benchmark 4: LRANGE
echo
echo -e "${YELLOW}Benchmark 4: LRANGE (List read)${NC}"
echo -n "  Running LRANGE commands..."
START_TIME=$(date +%s%N)
for i in $(seq 1 1000); do
    run_redrust_cmd "LRANGE bench:list 0 99" > /dev/null
done
END_TIME=$(date +%s%N)
ELAPSED=$(( (END_TIME - START_TIME) / 1000000 ))
# Scale up RPS for this test
RPS=$(( 1000 * 1000 / (ELAPSED + 1) ))
echo -e "\r  ${GREEN}LRANGE: 1000 operations in ${ELAPSED}ms (${RPS} req/sec)${NC}"

# Benchmark 5: Mixed workload
echo
echo -e "${YELLOW}Benchmark 5: Mixed Workload (50% SET, 50% GET)${NC}"
echo -n "  Running mixed workload..."
START_TIME=$(date +%s%N)
for i in $(seq 1 $((REQUESTS / 2))); do
    run_redrust_cmd "SET bench:mixed:$i value_$i" > /dev/null
    run_redrust_cmd "GET bench:key:$i" > /dev/null
done
END_TIME=$(date +%s%N)
ELAPSED=$(( (END_TIME - START_TIME) / 1000000 ))
RPS=$(( REQUESTS * 1000 / (ELAPSED + 1) ))
echo -e "\r  ${GREEN}Mixed: $REQUESTS operations in ${ELAPSED}ms (${RPS} req/sec)${NC}"

# Cleanup
echo
echo -e "${BLUE}Cleaning up...${NC}"
for i in $(seq 1 100); do
    echo -e "DEL bench:key:$i\r\n" | nc 127.0.0.1 $REDRUST_PORT 2>/dev/null | head -1 > /dev/null || true
done
echo -e "DEL bench:list\r\n" | nc 127.0.0.1 $REDRUST_PORT 2>/dev/null | head -1 > /dev/null || true

echo
echo -e "${GREEN}Benchmark complete!${NC}"
echo
echo -e "${BLUE}Note:${NC} These are basic benchmarks. For production-grade"
echo "      benchmarking, consider using redis-benchmark or memtier_benchmark"
